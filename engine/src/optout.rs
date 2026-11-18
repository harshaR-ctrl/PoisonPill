//! Opt-out detection.
//!
//! A clause offers an opt-out only when it states both an action the user can
//! take and a time window in which to take it. A bare address is deliberately
//! not enough: detection here feeds `opt_out_available`/`opt_out_target` on the
//! wire, and downstream mail generation is out of scope. The engine only
//! extracts a validated address so a future UI can display it.
//!
//! All scanning is linear and hand-rolled: no regex, no backtracking, no
//! allocation proportional to anything but the clause.

use crate::lexicon;
use crate::rules::find_phrase;

/// Result of inspecting one clause for opt-out language.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OptOut {
    /// The clause has both an action and a window.
    pub available: bool,
    /// Lowercased address, present only when `available` is true and a valid
    /// address was found.
    pub target: Option<String>,
}

/// Inspect a single clause. A window without an action, or an address without a
/// window, never qualifies.
pub fn detect(text: &str) -> OptOut {
    if !has_opt_out_action(text) || !has_opt_out_window(text) {
        return OptOut::default();
    }
    OptOut {
        available: true,
        target: extract_email(text),
    }
}

/// Maximum distance, in sentences, between a finding and a neighbouring clause
/// whose opt-out may be associated with it. Both must also share a block (text
/// not separated by a blank line).
pub const ASSOCIATION_WINDOW: usize = 3;

/// Opt-out facts for one clause, computed once per document so association is
/// linear in the number of clauses.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClauseOptOut {
    pub opt_out: OptOut,
    /// The clause opts out of something other than dispute resolution.
    pub foreign_subject: bool,
    /// The clause names dispute resolution (arbitration, "this provision", ...).
    pub dispute_subject: bool,
}

/// Inspect one clause for association purposes.
pub fn inspect(text: &str) -> ClauseOptOut {
    let opt_out = detect(text);
    if !opt_out.available {
        return ClauseOptOut::default();
    }
    ClauseOptOut {
        opt_out,
        foreign_subject: any_phrase(text, lexicon::OPT_OUT_FOREIGN_SUBJECTS),
        dispute_subject: any_phrase(text, lexicon::OPT_OUT_DISPUTE_SUBJECTS),
    }
}

/// Where a finding's opt-out came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Association {
    /// The finding's own clause states action and window.
    InClause(OptOut),
    /// A neighbouring clause in the same block does, unambiguously.
    Neighbour(OptOut),
    /// No opt-out, or the neighbourhood is ambiguous.
    None,
}

impl Association {
    pub fn opt_out(&self) -> OptOut {
        match self {
            Association::InClause(found) | Association::Neighbour(found) => found.clone(),
            Association::None => OptOut::default(),
        }
    }
}

/// Resolve the opt-out for clause `index`.
///
/// The rule is bounded and explainable:
/// 1. the clause's own opt-out always wins;
/// 2. otherwise consider clauses in the same block within
///    [`ASSOCIATION_WINDOW`] sentences that state action *and* window and do
///    not name a foreign subject (marketing, cookies, personal data, ...);
/// 3. if any candidate names dispute resolution, keep only those;
/// 4. if the remaining candidates name more than one distinct address, the
///    association is ambiguous and nothing is asserted.
///
/// `facts` and `blocks` are parallel per-clause arrays.
pub fn associate(facts: &[ClauseOptOut], blocks: &[usize], index: usize) -> Association {
    let Some(own) = facts.get(index) else {
        return Association::None;
    };
    if own.opt_out.available {
        return Association::InClause(own.opt_out.clone());
    }
    let Some(&block) = blocks.get(index) else {
        return Association::None;
    };

    let low = index.saturating_sub(ASSOCIATION_WINDOW);
    let high = (index + ASSOCIATION_WINDOW).min(facts.len().saturating_sub(1));
    let candidates: Vec<&ClauseOptOut> = (low..=high)
        .filter(|&other| other != index && blocks.get(other) == Some(&block))
        .filter_map(|other| facts.get(other))
        .filter(|fact| fact.opt_out.available && !fact.foreign_subject)
        .collect();
    if candidates.is_empty() {
        return Association::None;
    }

    let specific: Vec<&ClauseOptOut> = candidates
        .iter()
        .copied()
        .filter(|fact| fact.dispute_subject)
        .collect();
    let chosen = if specific.is_empty() {
        candidates
    } else {
        specific
    };

    let mut target: Option<&str> = None;
    for fact in &chosen {
        if let Some(address) = fact.opt_out.target.as_deref() {
            match target {
                Some(existing) if existing != address => return Association::None,
                _ => target = Some(address),
            }
        }
    }

    Association::Neighbour(OptOut {
        available: true,
        target: target.map(str::to_string),
    })
}

fn any_phrase(text: &str, phrases: &[&str]) -> bool {
    phrases
        .iter()
        .any(|phrase| find_phrase(text, phrase).is_some())
}

/// True when the clause names an action that constitutes opting out.
pub fn has_opt_out_action(text: &str) -> bool {
    lexicon::OPT_OUT_ACTIONS
        .iter()
        .any(|phrase| find_phrase(text, phrase).is_some())
}

/// True when the clause states a deadline. Explicit phrases are data in the
/// lexicon; numeric windows are additionally detected by shape so
/// `within 45 days` and `30-day` both count.
pub fn has_opt_out_window(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    if lexicon::OPT_OUT_WINDOW_PHRASES
        .iter()
        .chain(lexicon::OPT_OUT_DAY_WORDS)
        .any(|phrase| find_phrase(&lower, phrase).is_some())
    {
        return true;
    }
    numeric_day_window(&lower)
}

/// Find the first valid address in the clause, accepting common obfuscation.
/// The returned address is lowercased and rebuilt from its parts, so the result
/// is stable even when the source used `[at]` or `(dot)`.
pub fn extract_email(text: &str) -> Option<String> {
    let normalized = normalize(text);
    let bytes = normalized.as_bytes();

    for (at, _) in normalized.match_indices('@') {
        let start = expand_local(bytes, at);
        let end = expand_domain(bytes, at + 1);
        if start >= at || end <= at + 1 {
            continue;
        }
        let local = &normalized[start..at];
        let domain = &normalized[at + 1..end];
        if is_valid_local(local) && is_valid_domain(domain) {
            return Some(format!("{local}@{domain}"));
        }
    }
    None
}

/// Replace `[at]`, `(at)`, ` at `, `[dot]`, `(dot)` and ` dot ` with their
/// literal characters and collapse the surrounding whitespace.
fn normalize(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let mut out = String::with_capacity(lower.len());
    let mut index = 0usize;

    while index < lower.len() {
        if let Some((marker, replacement)) = marker_at(&lower, index) {
            trim_trailing_space(&mut out);
            out.push(replacement);
            index += marker.len();
            index = skip_space(lower.as_bytes(), index);
        } else {
            // `index` is always on a boundary: markers and copied chars are
            // whole UTF-8 sequences.
            let Some(character) = lower[index..].chars().next() else {
                break;
            };
            out.push(character);
            index += character.len_utf8();
        }
    }

    out
}

const MARKERS: &[(&str, char)] = &[
    ("(at)", '@'),
    ("[at]", '@'),
    (" at ", '@'),
    ("(dot)", '.'),
    ("[dot]", '.'),
    (" dot ", '.'),
];

fn marker_at(text: &str, index: usize) -> Option<(&'static str, char)> {
    MARKERS
        .iter()
        .copied()
        .find(|(marker, _)| text[index..].starts_with(marker))
}

fn trim_trailing_space(out: &mut String) {
    while matches!(out.chars().next_back(), Some(' ' | '\t' | '\n' | '\r')) {
        out.pop();
    }
}

fn skip_space(bytes: &[u8], mut index: usize) -> usize {
    while matches!(bytes.get(index), Some(b' ' | b'\t' | b'\n' | b'\r')) {
        index += 1;
    }
    index
}

/// Walk left from `@`, stopping on the first character that cannot appear in a
/// local part. `@` itself stops the walk, which keeps `us@privacy@example.com`
/// scannable from its second `@`.
fn expand_local(bytes: &[u8], at: usize) -> usize {
    let mut start = at;
    while start > 0 && is_local_byte(bytes[start - 1]) {
        start -= 1;
    }
    start
}

fn expand_domain(bytes: &[u8], from: usize) -> usize {
    let mut end = from;
    while let Some(byte) = bytes.get(end) {
        if !is_domain_byte(*byte) {
            break;
        }
        end += 1;
    }
    // A sentence-final period is punctuation, not a label separator.
    while end > from && matches!(bytes.get(end - 1), Some(b'.' | b'-')) {
        end -= 1;
    }
    end
}

fn is_local_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'/'
                | b'='
                | b'?'
                | b'^'
                | b'_'
                | b'`'
                | b'{'
                | b'|'
                | b'}'
                | b'~'
                | b'-'
                | b'.'
        )
}

fn is_domain_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-')
}

/// Prose pronouns that appear immediately before `at` in sentences like
/// "email us at example.com". Treating them as a local part would invent an
/// address, so they are rejected.
const PROSE_LOCAL_PARTS: &[&str] = &[
    "us", "me", "you", "them", "him", "her", "it", "this", "that",
];

fn is_valid_local(local: &str) -> bool {
    !local.is_empty()
        && local.len() <= 64
        && !local.starts_with('.')
        && !local.ends_with('.')
        && !local.contains("..")
        && !PROSE_LOCAL_PARTS.contains(&local)
        && local
            .bytes()
            .all(|byte| is_local_byte(byte) && byte != b'@')
}

fn is_valid_domain(domain: &str) -> bool {
    if domain.is_empty() || domain.len() > 253 {
        return false;
    }

    let labels: Vec<&str> = domain.split('.').collect();
    if labels.len() < 2 {
        return false;
    }
    if labels.iter().any(|label| {
        label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    }) {
        return false;
    }

    let Some(tld) = labels.last() else {
        return false;
    };
    tld.len() >= 2 && tld.bytes().all(|byte| byte.is_ascii_alphabetic())
}

/// A number (Arabic digits) adjacent to a day unit: `30 days`, `30-day`.
fn numeric_day_window(lower: &str) -> bool {
    let words: Vec<&str> = lower
        .split_whitespace()
        .map(|word| word.trim_matches(|c: char| !(c.is_ascii_alphanumeric() || c == '-')))
        .filter(|word| !word.is_empty())
        .collect();

    for (index, word) in words.iter().enumerate() {
        let mut parts = word.split('-').filter(|part| !part.is_empty()).peekable();
        while let Some(part) = parts.next() {
            if !part.bytes().all(|byte| byte.is_ascii_digit()) {
                continue;
            }
            if parts.peek().is_some_and(|next| next.starts_with("day")) {
                return true;
            }
            if words
                .get(index + 1)
                .is_some_and(|next| next.starts_with("day"))
            {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_and_window_produce_available() {
        let found = detect("You may opt out within 30 days of accepting these terms.");
        assert!(found.available);
        assert_eq!(found.target, None);
    }

    #[test]
    fn plain_address_is_lowercased() {
        let found = detect(
            "You may opt out within 30 days by emailing Privacy@Example.COM and stating your name.",
        );
        assert!(found.available);
        assert_eq!(found.target.as_deref(), Some("privacy@example.com"));
    }

    #[test]
    fn bare_address_without_a_window_never_qualifies() {
        let found = detect("Questions can be sent to privacy@example.com.");
        assert!(!found.available);
        assert_eq!(found.target, None);
    }

    #[test]
    fn window_without_an_action_never_qualifies() {
        let found = detect("You have 30 days to respond to this notice.");
        assert!(!found.available);
        assert_eq!(found.target, None);
    }

    #[test]
    fn any_time_window_qualifies() {
        let found = detect("Opt out at any time by emailing privacy@example.com.");
        assert!(found.available);
        assert_eq!(found.target.as_deref(), Some("privacy@example.com"));
    }

    #[test]
    fn bracketed_obfuscation_is_rebuilt() {
        let found = detect(
            "To opt out within 30 days, write to privacy [at] example [dot] com and confirm.",
        );
        assert!(found.available);
        assert_eq!(found.target.as_deref(), Some("privacy@example.com"));
    }

    #[test]
    fn parenthesized_obfuscation_is_rebuilt() {
        let found = detect(
            "To opt out within 30 days, write to privacy (at) example (dot) com and confirm.",
        );
        assert_eq!(found.target.as_deref(), Some("privacy@example.com"));
    }

    #[test]
    fn spaced_obfuscation_is_rebuilt() {
        let found = detect("To opt out within 30 days, write to privacy at example dot com.");
        assert_eq!(found.target.as_deref(), Some("privacy@example.com"));
    }

    #[test]
    fn prose_pronoun_before_at_is_not_a_local_part() {
        let found = detect("You may opt out within 30 days by emailing us at example.com.");
        assert!(found.available);
        assert_eq!(found.target, None);
    }

    #[test]
    fn address_preceded_by_at_marker_still_parses() {
        let found =
            detect("To opt out within 30 days, write to us at privacy@example.com, or call us.");
        assert_eq!(found.target.as_deref(), Some("privacy@example.com"));
    }

    #[test]
    fn malformed_addresses_are_rejected() {
        for text in [
            "Opt out within 30 days by emailing privacy@example.",
            "Opt out within 30 days by emailing privacy@-example.com.",
            "Opt out within 30 days by emailing privacy@example..com.",
            "Opt out within 30 days by emailing privacy@exam_ple.com.",
            "Opt out within 30 days by emailing .privacy@example.com.",
        ] {
            let found = detect(text);
            assert!(found.available, "action and window present: {text}");
            assert_eq!(found.target, None, "should reject: {text}");
        }
    }

    #[test]
    fn trailing_sentence_period_is_not_a_label() {
        assert_eq!(
            extract_email("Write to privacy@example.com.").as_deref(),
            Some("privacy@example.com")
        );
    }

    #[test]
    fn unicode_never_panics_and_is_rejected() {
        let found = detect("Opt out within 30 days — naïve@exämple.com is not an address. 🚫");
        assert!(found.available);
        assert_eq!(found.target, None);

        assert_eq!(extract_email("emoji 🚫@example.com here"), None);
        assert_eq!(extract_email("ünïcode @ example.com"), None);
    }

    #[test]
    fn numeric_window_supports_ranges_and_hyphens() {
        assert!(has_opt_out_window("within 45 days"));
        assert!(has_opt_out_window("a 30-day window"));
        assert!(has_opt_out_window("no later than ninety (90) days"));
        assert!(!has_opt_out_window("within a reasonable while"));
    }

    fn facts(clauses: &[&str]) -> Vec<ClauseOptOut> {
        clauses.iter().map(|clause| inspect(clause)).collect()
    }

    #[test]
    fn own_clause_opt_out_wins() {
        let facts = facts(&["Disputes go to binding arbitration; opt out within 30 days."]);
        assert!(matches!(
            associate(&facts, &[0], 0),
            Association::InClause(_)
        ));
    }

    #[test]
    fn neighbour_in_same_block_is_associated() {
        let facts = facts(&[
            "Any dispute shall be resolved by binding arbitration.",
            "You may opt out within 45 days by emailing optout@example.com.",
        ]);
        let found = associate(&facts, &[0, 0], 0);
        assert_eq!(
            found,
            Association::Neighbour(OptOut {
                available: true,
                target: Some("optout@example.com".into()),
            })
        );
    }

    #[test]
    fn neighbour_in_another_block_is_not_associated() {
        let facts = facts(&[
            "Any dispute shall be resolved by binding arbitration.",
            "You may opt out within 45 days by emailing optout@example.com.",
        ]);
        assert_eq!(associate(&facts, &[0, 1], 0), Association::None);
    }

    #[test]
    fn neighbour_beyond_the_window_is_not_associated() {
        let mut clauses = vec!["Any dispute shall be resolved by binding arbitration."];
        clauses.extend(["Filler sentence."; ASSOCIATION_WINDOW]);
        clauses.push("You may opt out within 45 days by emailing optout@example.com.");
        let facts = facts(&clauses);
        let blocks = vec![0; facts.len()];
        assert_eq!(associate(&facts, &blocks, 0), Association::None);
    }

    #[test]
    fn foreign_subject_is_never_associated() {
        let facts = facts(&[
            "Any dispute shall be resolved by binding arbitration.",
            "You may opt out of marketing emails at any time by emailing privacy@example.com.",
        ]);
        assert_eq!(associate(&facts, &[0, 0], 0), Association::None);
    }

    #[test]
    fn competing_addresses_are_ambiguous() {
        let facts = facts(&[
            "You may opt out within 30 days by emailing a@example.com.",
            "Any dispute shall be resolved by binding arbitration.",
            "You may opt out within 30 days by emailing b@example.com.",
        ]);
        assert_eq!(associate(&facts, &[0, 0, 0], 1), Association::None);
    }

    #[test]
    fn dispute_subject_breaks_a_tie() {
        let facts = facts(&[
            "You may opt out within 30 days by emailing a@example.com.",
            "Any dispute shall be resolved by binding arbitration.",
            "To opt out of arbitration, email b@example.com within 30 days.",
        ]);
        assert_eq!(
            associate(&facts, &[0, 0, 0], 1).opt_out().target.as_deref(),
            Some("b@example.com")
        );
    }

    #[test]
    fn written_windows_are_lexicon_driven() {
        assert!(has_opt_out_window("no later than sixty (60) days"));
        assert!(has_opt_out_window("within thirty (30) days"));
        assert!(has_opt_out_window("at any time"));
    }
}
