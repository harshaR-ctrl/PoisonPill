//! Declarative rule format and evaluator.
//!
//! A `RuleSpec` is data: ids, label, modality, weight, predicate lists,
//! proximity window and inversion sensitivity. The control flow below is
//! category-agnostic; adding a category means adding lexicons and one more
//! `RuleSpec` constant, not touching the evaluator.

use crate::lexicon;
use crate::model::Modality;

/// A predicate over one clause. Predicates are deliberately data-shaped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Predicate {
    /// True when any listed phrase occurs. Evidence is the earliest phrase.
    AnyPhrase(&'static [&'static str]),
    /// True when every listed phrase occurs.
    AllPhrases(&'static [&'static str]),
    /// True when a left phrase and a right phrase occur within `window` words
    /// of each other. A window of 0 means "anywhere in the clause".
    Near {
        left: &'static [&'static str],
        right: &'static [&'static str],
    },
}

/// One authored rule. All matcher behaviour is driven by these fields.
#[derive(Debug, Clone, Copy)]
pub struct RuleSpec {
    /// Stable concrete rule id, e.g. `forced_arbitration.class_action_waiver`.
    pub rule_id: &'static str,
    /// Stable category id shared by several rules.
    pub category_id: &'static str,
    /// Human label for the category.
    pub label: &'static str,
    pub modality: Modality,
    /// Base weight `w_k`, in `[1.0, 5.0]`.
    pub weight: f32,
    /// All of these must match.
    pub required: &'static [Predicate],
    /// Any of these add evidence but never gate the rule.
    pub optional: &'static [Predicate],
    /// Any of these suppress the rule.
    pub prohibited: &'static [Predicate],
    /// Maximum words between the first and last required match; 0 disables.
    pub proximity_window: usize,
    /// Opt the rule into exception-marker polarity evaluation.
    pub inversion_sensitive: bool,
}

/// A successful rule evaluation. `evidence` is stable data, not text generated
/// by the matcher, so reasons are deterministic and explainable.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleMatch {
    pub rule_id: &'static str,
    pub category_id: &'static str,
    pub label: &'static str,
    pub modality: Modality,
    pub weight: f32,
    pub evidence: Vec<&'static str>,
    pub spans: Vec<(usize, usize)>,
    pub inversion_sensitive: bool,
}

/// Ordered rule set. The order is part of determinism: when two rules share a
/// category, the first match wins.
#[derive(Debug, Clone, Default)]
pub struct RuleRegistry {
    rules: Vec<RuleSpec>,
}

impl RuleRegistry {
    pub fn new(rules: Vec<RuleSpec>) -> Self {
        Self { rules }
    }

    /// The shipped registry. Currently exactly one authored category.
    pub fn builtin() -> Self {
        Self::new(BUILTIN_RULES.to_vec())
    }

    pub fn rules(&self) -> &[RuleSpec] {
        &self.rules
    }

    /// Runtime registration is the extension seam the guarantee test exercises.
    pub fn add(&mut self, rule: RuleSpec) {
        self.rules.push(rule);
    }

    pub fn matches(&self, clause: &str) -> Vec<RuleMatch> {
        self.rules
            .iter()
            .filter_map(|rule| rule.evaluate(clause))
            .collect()
    }
}

impl RuleSpec {
    pub fn evaluate(&self, clause: &str) -> Option<RuleMatch> {
        let window = self.proximity_window;
        let mut evidence = Vec::new();
        let mut hit_spans = Vec::new();
        let mut all_spans = Vec::new();

        for predicate in self.required {
            let hit = predicate.evaluate(clause, window)?;
            push_unique(&mut evidence, hit.evidence);
            hit_spans.push((hit.start, hit.end));
            all_spans.extend(predicate.all_spans(clause, window));
        }

        if window > 0 && hit_spans.len() > 1 {
            let start = hit_spans.iter().map(|(start, _)| *start).min()?;
            let end = hit_spans.iter().map(|(_, end)| *end).max()?;
            if word_span(clause, start, end) > window {
                return None;
            }
        }

        for predicate in self.prohibited {
            if predicate.matches(clause, window) {
                return None;
            }
        }

        for predicate in self.optional {
            if let Some(hit) = predicate.evaluate(clause, window) {
                push_unique(&mut evidence, hit.evidence);
                all_spans.extend(predicate.all_spans(clause, window));
            }
        }

        all_spans.sort_unstable();
        all_spans.dedup();

        Some(RuleMatch {
            rule_id: self.rule_id,
            category_id: self.category_id,
            label: self.label,
            modality: self.modality,
            weight: self.weight,
            evidence,
            spans: all_spans,
            inversion_sensitive: self.inversion_sensitive,
        })
    }
}

impl Predicate {
    pub fn matches(&self, clause: &str, window: usize) -> bool {
        self.evaluate(clause, window).is_some()
    }

    pub fn evaluate(&self, clause: &str, window: usize) -> Option<PredicateHit> {
        match self {
            Predicate::AnyPhrase(phrases) => earliest_phrase(clause, phrases),
            Predicate::AllPhrases(phrases) => all_phrases(clause, phrases),
            Predicate::Near { left, right } => nearest_pair(clause, left, right, window),
        }
    }

    /// Every occurrence that could count as matched data. Inversion uses this
    /// so a hostile carve-out after the marker is visible even when the same
    /// phrase also appeared in the compliant base.
    pub fn all_spans(&self, clause: &str, window: usize) -> Vec<(usize, usize)> {
        match self {
            Predicate::AnyPhrase(phrases) | Predicate::AllPhrases(phrases) => {
                all_occurrences(clause, phrases)
                    .into_iter()
                    .map(|(start, _, len)| (start, start + len))
                    .collect()
            }
            Predicate::Near { .. } => self
                .evaluate(clause, window)
                .map(|hit| vec![(hit.start, hit.end)])
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PredicateHit {
    pub start: usize,
    pub end: usize,
    pub evidence: Vec<&'static str>,
}

fn earliest_phrase(clause: &str, phrases: &'static [&'static str]) -> Option<PredicateHit> {
    let mut best: Option<(&'static str, usize)> = None;
    for phrase in phrases {
        if let Some(start) = find_phrase(clause, phrase) {
            if best.is_none_or(|(_, best_start)| start < best_start) {
                best = Some((phrase, start));
            }
        }
    }
    best.map(|(phrase, start)| PredicateHit {
        start,
        end: start + phrase.len(),
        evidence: vec![phrase],
    })
}

fn all_phrases(clause: &str, phrases: &'static [&'static str]) -> Option<PredicateHit> {
    let mut start = usize::MAX;
    let mut end = 0usize;
    let mut evidence = Vec::with_capacity(phrases.len());
    for phrase in phrases {
        let phrase_start = find_phrase(clause, phrase)?;
        start = start.min(phrase_start);
        end = end.max(phrase_start + phrase.len());
        evidence.push(*phrase);
    }
    Some(PredicateHit {
        start,
        end,
        evidence,
    })
}

fn nearest_pair(
    clause: &str,
    left: &'static [&'static str],
    right: &'static [&'static str],
    window: usize,
) -> Option<PredicateHit> {
    let lefts = all_occurrences(clause, left);
    let rights = all_occurrences(clause, right);

    let mut best: Option<(usize, usize, &'static str, &'static str, usize)> = None;
    for (left_start, left_phrase, left_len) in &lefts {
        for (right_start, right_phrase, right_len) in &rights {
            let start = *left_start.min(right_start);
            let end = (*left_start + left_len).max(*right_start + right_len);
            let distance = word_span(clause, start, end);
            if window > 0 && distance > window {
                continue;
            }
            if best.is_none_or(|(_, _, _, _, best_distance)| distance < best_distance) {
                best = Some((start, end, left_phrase, right_phrase, distance));
            }
        }
    }

    best.map(|(start, end, left_phrase, right_phrase, _)| PredicateHit {
        start,
        end,
        evidence: vec![left_phrase, right_phrase],
    })
}

fn all_occurrences(
    clause: &str,
    phrases: &'static [&'static str],
) -> Vec<(usize, &'static str, usize)> {
    let mut out = Vec::new();
    for phrase in phrases {
        let mut from = 0usize;
        while let Some(relative) = find_phrase(&clause[from..], phrase) {
            let start = from + relative;
            out.push((start, *phrase, phrase.len()));
            from = start + phrase.len().max(1);
            if out.len() >= 64 {
                return out;
            }
        }
    }
    out
}

fn word_span(clause: &str, start: usize, end: usize) -> usize {
    if start >= end || end > clause.len() {
        return 0;
    }
    clause[start..end].split_whitespace().count()
}

/// ASCII-case-insensitive substring search. Returns a byte offset on a char
/// boundary. Deliberately no regex and no backtracking.
pub fn find_phrase(haystack: &str, phrase: &str) -> Option<usize> {
    let needle = phrase.as_bytes();
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }

    let hay = haystack.as_bytes();
    let last = hay.len() - needle.len();
    let lower = needle[0].to_ascii_lowercase();
    let upper = needle[0].to_ascii_uppercase();
    // An all-ASCII needle can only match ASCII bytes, and an ASCII byte (and
    // the byte after one) is always a char boundary in valid UTF-8, so the
    // boundary checks are needed only for non-ASCII needles.
    let ascii = needle.is_ascii();

    let mut start = 0usize;
    while start <= last {
        // Skip straight to the next byte that can begin a match.
        let offset = hay[start..=last]
            .iter()
            .position(|&byte| byte == lower || byte == upper)?;
        start += offset;
        let end = start + needle.len();
        if hay[start..end].eq_ignore_ascii_case(needle)
            && (ascii || (haystack.is_char_boundary(start) && haystack.is_char_boundary(end)))
        {
            return Some(start);
        }
        start += 1;
    }
    None
}

fn push_unique(target: &mut Vec<&'static str>, values: Vec<&'static str>) {
    for value in values {
        if !target.contains(&value) {
            target.push(value);
        }
    }
}

// ---------------------------------------------------------------------------
// Authored data: Forced Arbitration & Class Waiver
// ---------------------------------------------------------------------------

const ARBITRATION_GUARDS: &[Predicate] = &[
    Predicate::AnyPhrase(lexicon::COMPLIANCE_GUARDS),
    Predicate::AnyPhrase(lexicon::PROVIDER_WAIVERS),
    Predicate::AnyPhrase(lexicon::SEVERABILITY_REFERENCES),
];

const MANDATORY_ARBITRATION_GUARDS: &[Predicate] = &[
    Predicate::AnyPhrase(lexicon::COMPLIANCE_GUARDS),
    Predicate::AnyPhrase(lexicon::PROVIDER_WAIVERS),
    Predicate::AnyPhrase(lexicon::SEVERABILITY_REFERENCES),
    Predicate::AnyPhrase(lexicon::VOLUNTARY_ARBITRATION),
];

/// The one reference category. Every field is data.
pub const ARBITRATION_RULES: &[RuleSpec] = &[
    RuleSpec {
        rule_id: "forced_arbitration.class_action_waiver",
        category_id: "forced_arbitration",
        label: "Forced Arbitration & Class Waiver",
        modality: Modality::Waiver,
        weight: 5.0,
        required: &[
            Predicate::AnyPhrase(lexicon::WAIVER_VERBS),
            Predicate::AnyPhrase(lexicon::CLASS_ACTIONS),
        ],
        optional: &[Predicate::AnyPhrase(lexicon::ARBITRATION_AMPLIFIERS)],
        prohibited: ARBITRATION_GUARDS,
        proximity_window: 45,
        inversion_sensitive: false,
    },
    RuleSpec {
        rule_id: "forced_arbitration.jury_trial_waiver",
        category_id: "forced_arbitration",
        label: "Forced Arbitration & Class Waiver",
        modality: Modality::Waiver,
        weight: 5.0,
        required: &[
            Predicate::AnyPhrase(lexicon::WAIVER_VERBS),
            Predicate::AnyPhrase(lexicon::JURY_TERMS),
        ],
        optional: &[],
        prohibited: ARBITRATION_GUARDS,
        proximity_window: 45,
        inversion_sensitive: false,
    },
    RuleSpec {
        rule_id: "forced_arbitration.court_access_waiver",
        category_id: "forced_arbitration",
        label: "Forced Arbitration & Class Waiver",
        modality: Modality::Waiver,
        weight: 5.0,
        required: &[
            Predicate::AnyPhrase(lexicon::WAIVER_VERBS),
            Predicate::AnyPhrase(lexicon::COURT_TERMS),
        ],
        optional: &[],
        prohibited: ARBITRATION_GUARDS,
        proximity_window: 45,
        inversion_sensitive: false,
    },
    RuleSpec {
        rule_id: "forced_arbitration.mandatory_arbitration",
        category_id: "forced_arbitration",
        label: "Forced Arbitration & Class Waiver",
        modality: Modality::Waiver,
        weight: 5.0,
        required: &[
            Predicate::AnyPhrase(lexicon::MANDATORY_ARBITRATION),
            Predicate::AnyPhrase(lexicon::DISPUTE_NEXUS),
        ],
        optional: &[],
        prohibited: MANDATORY_ARBITRATION_GUARDS,
        proximity_window: 60,
        inversion_sensitive: false,
    },
    RuleSpec {
        rule_id: "forced_arbitration.class_action_bar",
        category_id: "forced_arbitration",
        label: "Forced Arbitration & Class Waiver",
        modality: Modality::Prohibition,
        weight: 5.0,
        required: &[
            Predicate::AnyPhrase(lexicon::CLASS_ACTIONS),
            Predicate::AnyPhrase(lexicon::CLASS_ACTION_BARS),
            Predicate::AnyPhrase(lexicon::CLASS_BAR_SUBJECTS),
        ],
        optional: &[],
        prohibited: ARBITRATION_GUARDS,
        proximity_window: 45,
        inversion_sensitive: false,
    },
];

/// Every shipped rule. Add a category by appending its rules here.
pub const BUILTIN_RULES: &[RuleSpec] = ARBITRATION_RULES;

#[cfg(test)]
mod tests {
    use super::*;

    fn first_match(registry: &RuleRegistry, text: &str) -> Option<RuleMatch> {
        registry.matches(text).into_iter().next()
    }

    #[test]
    fn class_action_waiver_fires_with_evidence() {
        let registry = RuleRegistry::builtin();
        let found = first_match(
            &registry,
            "You waive any right to bring claims as a plaintiff or class member in a class action.",
        )
        .expect("expected a class waiver");
        assert_eq!(found.rule_id, "forced_arbitration.class_action_waiver");
        assert!(found.evidence.contains(&"waive"));
        assert!(found
            .evidence
            .iter()
            .any(|evidence| evidence.contains("class")));
    }

    #[test]
    fn mandatory_arbitration_fires() {
        let registry = RuleRegistry::builtin();
        let found = first_match(
            &registry,
            "Any dispute arising out of these Terms shall be resolved exclusively by binding arbitration.",
        )
        .expect("expected mandatory arbitration");
        assert_eq!(found.rule_id, "forced_arbitration.mandatory_arbitration");
    }

    #[test]
    fn provider_waiver_is_prohibited() {
        let registry = RuleRegistry::builtin();
        assert!(
            first_match(&registry, "We waive any right to a jury trial.").is_none(),
            "a provider-side waiver is not a user harm"
        );
    }

    #[test]
    fn compliance_boilerplate_is_prohibited() {
        let registry = RuleRegistry::builtin();
        assert!(first_match(
            &registry,
            "Nothing in this Agreement waives your right to a jury trial."
        )
        .is_none());
    }

    #[test]
    fn proximity_window_rejects_far_apart_terms() {
        let registry = RuleRegistry::builtin();
        let filler = "word ".repeat(80);
        let text = format!("You waive your rights. {filler} A class action is described later.");
        assert!(registry.matches(&text).is_empty());
    }

    #[test]
    fn phrase_search_is_ascii_case_insensitive() {
        assert_eq!(find_phrase("You WAIVE all claims", "waive"), Some(4));
    }

    #[test]
    fn phrase_search_handles_multibyte_text() {
        let text = "Ünïcode — you waive";
        assert_eq!(find_phrase(text, "WAIVE"), text.find("waive"));
        assert_eq!(find_phrase("naïve waiver", "ïve"), Some(2));
        assert_eq!(find_phrase("🚫🚫", "x"), None);
        assert_eq!(find_phrase("ab", "abc"), None);
        assert_eq!(find_phrase("aaab", "aab"), Some(1));
    }

    #[test]
    fn rule_ids_are_unique() {
        let mut ids: Vec<_> = BUILTIN_RULES.iter().map(|rule| rule.rule_id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), BUILTIN_RULES.len());
    }
}
