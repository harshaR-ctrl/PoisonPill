//! Deontic extraction and the wasm-bindgen entry point.
//!
//! Detection is entirely driven by `rules::RuleRegistry`; this module only
//! walks sentences, turns rule matches into wire findings, and attaches
//! opt-out/scoring evidence.

use wasm_bindgen::prelude::*;

use crate::exceptions::classify;
use crate::model::{
    AnalyzeResponse, ClauseFinding, DocumentReport, InversionShape, SCHEMA_VERSION,
};
use crate::optout::{associate, inspect as inspect_opt_out, Association};
use crate::rules::{RuleMatch, RuleRegistry};
use crate::scoring::poison_index;
use crate::tokenizer::{sentences, Sentence};

/// Hard limit on analysed input. Anything larger is truncated on a UTF-8
/// boundary and reported via `DocumentReport::truncated`. This bounds worst-case
/// work for pathological pages while leaving any realistic contract untouched.
/// 512 KiB is roughly 80,000 words.
pub const MAX_INPUT_BYTES: usize = 512 * 1024;

#[wasm_bindgen]
pub struct LegalEngine {
    registry: RuleRegistry,
}

#[wasm_bindgen]
impl LegalEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            registry: RuleRegistry::builtin(),
        }
    }

    /// Analyse a contract and return a `DocumentReport` as a JS object.
    #[wasm_bindgen(js_name = analyzeDocument)]
    pub fn analyze_document(&self, raw_text: &str) -> JsValue {
        let report = analyze_with(&self.registry, raw_text);
        serde_wasm_bindgen::to_value(&report).unwrap_or(JsValue::NULL)
    }

    /// Canonical JSON form used by parity tests and the worker.
    #[wasm_bindgen(js_name = analyzeJson)]
    pub fn analyze_json(&self, raw_text: &str) -> String {
        let outcome = AnalyzeResponse::ok(analyze_with(&self.registry, raw_text));
        serde_json::to_string(&outcome).unwrap_or_else(|error| {
            serde_json::json!({
                "status": "error",
                "error": { "code": "internal_serialization", "message": error.to_string() }
            })
            .to_string()
        })
    }
}

impl Default for LegalEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl LegalEngine {
    /// Native-only seam for the runtime rule-format guarantee test.
    pub fn with_registry(registry: RuleRegistry) -> Self {
        Self { registry }
    }

    pub fn registry(&self) -> &RuleRegistry {
        &self.registry
    }
}

/// Analyse with the shipped registry.
pub fn analyze(raw_text: &str) -> Vec<ClauseFinding> {
    analyze_with(&RuleRegistry::builtin(), raw_text).findings
}

/// Analyse with an arbitrary registry. This is the function the guarantee test
/// uses to prove that a synthetic category can be registered as data.
pub fn analyze_with(registry: &RuleRegistry, raw_text: &str) -> DocumentReport {
    let (input, truncated) = truncate_input(raw_text, MAX_INPUT_BYTES);
    let mut findings = Vec::new();

    let clauses = sentences(input);
    let blocks = block_ids(input, &clauses);
    // Opt-out facts are only needed once a clause produces a finding; compute
    // them lazily, at most once per clause, so a document with no findings
    // pays nothing.
    let mut opt_out_facts = None;

    for (index, sentence) in clauses.iter().enumerate() {
        let clause = sentence.text;
        let mut emitted_categories: Vec<&'static str> = Vec::new();

        for rule_match in registry.matches(clause) {
            let inversion = classify(clause, rule_match.inversion_sensitive, &rule_match.spans);
            if inversion == InversionShape::Inverted {
                continue;
            }

            // One finding per category per clause, selected by registry order.
            if emitted_categories.contains(&rule_match.category_id) {
                continue;
            }
            emitted_categories.push(rule_match.category_id);

            let facts = opt_out_facts.get_or_insert_with(|| {
                clauses
                    .iter()
                    .map(|sentence| inspect_opt_out(sentence.text))
                    .collect::<Vec<_>>()
            });
            let association = associate(facts, &blocks, index);
            let opt_out = association.opt_out();
            findings.push(ClauseFinding {
                category: rule_match.category_id.to_string(),
                label: rule_match.label.to_string(),
                rule_id: rule_match.rule_id.to_string(),
                raw_text: clause.to_string(),
                modality: rule_match.modality,
                severity: rule_match.weight,
                start_offset: sentence.start,
                end_offset: sentence.end,
                opt_out_available: opt_out.available,
                opt_out_target: opt_out.target,
                inversion,
                reason: reason_for(&rule_match, inversion, &association),
            });
        }
    }

    DocumentReport {
        schema_version: SCHEMA_VERSION.to_string(),
        poison_index: poison_index(&findings),
        findings,
        truncated,
    }
}

/// Cut to at most `cap` bytes without splitting a UTF-8 character. The returned
/// prefix always satisfies `raw_text.is_char_boundary(prefix.len())`.
fn truncate_input(text: &str, cap: usize) -> (&str, bool) {
    if text.len() <= cap {
        return (text, false);
    }
    let mut end = cap;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    (&text[..end], true)
}

/// Block index for each clause. A block is a run of text not interrupted by a
/// blank line, so association never crosses a paragraph or section break.
fn block_ids(input: &str, clauses: &[Sentence<'_>]) -> Vec<usize> {
    let mut ids = Vec::with_capacity(clauses.len());
    let mut block = 0usize;
    let mut previous_end = 0usize;
    for clause in clauses {
        if previous_end > 0 && is_block_break(&input[previous_end..clause.start]) {
            block += 1;
        }
        ids.push(block);
        previous_end = clause.end;
    }
    ids
}

/// True when the gap between two clauses contains a blank line.
fn is_block_break(gap: &str) -> bool {
    let mut newlines = 0usize;
    for character in gap.chars() {
        match character {
            '\n' => {
                newlines += 1;
                if newlines >= 2 {
                    return true;
                }
            }
            ' ' | '\t' | '\r' => {}
            _ => newlines = 0,
        }
    }
    false
}

/// One sentence, built only from matched rule data.
fn reason_for(
    rule_match: &RuleMatch,
    inversion: InversionShape,
    association: &Association,
) -> String {
    let base = base_reason(rule_match, inversion);
    match association {
        Association::Neighbour(_) => {
            format!("{base}; opt-out stated in a neighbouring clause of the same block")
        }
        _ => base,
    }
}

fn base_reason(rule_match: &RuleMatch, inversion: InversionShape) -> String {
    let quoted = rule_match
        .evidence
        .iter()
        .map(|evidence| format!("\"{evidence}\""))
        .collect::<Vec<_>>()
        .join(", ");
    match inversion {
        InversionShape::ComplianceThenCarveOut => format!(
            "matched {quoted} in the clause; a hostile carve-out overrides the compliant base"
        ),
        InversionShape::Canceled => {
            format!("matched {quoted} in the clause; two carve-outs cancel each other")
        }
        _ => format!("matched {quoted} in the clause"),
    }
}

/// `P_I` from the scoring module. Kept here as a re-export for callers that
/// already import from the engine entry point.
pub use crate::scoring::poison_index as score;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Modality;
    use crate::rules::{Predicate, RuleRegistry, RuleSpec};

    #[test]
    fn finds_arbitration_clause_and_opt_out() {
        let doc = "You waive any right to a jury trial or class action, \
                   unless you opt-out within 30 days by emailing arb@example.com.";
        let findings = analyze(doc);
        assert!(findings.iter().any(|f| f.opt_out_available));
        assert!(findings
            .iter()
            .any(|f| f.opt_out_target.as_deref() == Some("arb@example.com")));
    }

    #[test]
    fn bare_address_without_a_window_is_not_an_opt_out() {
        let doc = "You waive any right to a class action; questions go to legal@example.com.";
        let findings = analyze(doc);
        assert!(!findings.is_empty());
        assert!(findings
            .iter()
            .all(|finding| !finding.opt_out_available && finding.opt_out_target.is_none()));
    }

    #[test]
    fn neighbouring_opt_out_is_associated_and_explained() {
        let doc = "Any dispute shall be resolved by binding arbitration. \
                   You may opt out within 45 days by emailing optout@example.com.";
        let finding = analyze(doc).into_iter().next().expect("finding");
        assert!(finding.opt_out_available);
        assert_eq!(
            finding.opt_out_target.as_deref(),
            Some("optout@example.com")
        );
        assert!(finding
            .reason
            .ends_with("neighbouring clause of the same block"));
    }

    #[test]
    fn blank_line_stops_association() {
        let doc = "Any dispute shall be resolved by binding arbitration.\n\n\
                   You may opt out within 45 days by emailing optout@example.com.";
        let finding = analyze(doc).into_iter().next().expect("finding");
        assert!(!finding.opt_out_available);
        assert_eq!(finding.opt_out_target, None);
    }

    #[test]
    fn single_newline_keeps_the_block() {
        let doc = "Any dispute shall be resolved by binding arbitration.\n\
                   You may opt out within 45 days by emailing optout@example.com.";
        let finding = analyze(doc).into_iter().next().expect("finding");
        assert!(finding.opt_out_available);
    }

    #[test]
    fn unrelated_neighbouring_opt_out_is_ignored() {
        let doc = "Any dispute shall be resolved by binding arbitration. \
                   You may opt out of marketing emails at any time by emailing privacy@example.com.";
        let finding = analyze(doc).into_iter().next().expect("finding");
        assert!(!finding.opt_out_available);
        assert_eq!(finding.opt_out_target, None);
        assert!(!finding.reason.contains("neighbouring"));
    }

    #[test]
    fn finding_offsets_slice_the_input() {
        let doc = "Intro. You waive any right to a class action.";
        let findings = analyze(doc);
        let finding = findings.first().expect("expected a finding");
        assert_eq!(
            &doc[finding.start_offset..finding.end_offset],
            finding.raw_text
        );
    }

    #[test]
    fn reason_uses_matched_data() {
        let doc = "You waive any right to a class action.";
        let finding = analyze(doc).into_iter().next().expect("finding");
        assert!(finding.reason.contains("\"waive\""));
        assert!(finding.reason.contains("\"class action\""));
    }

    #[test]
    fn poisoned_category_is_deduped_per_clause() {
        let doc = "You waive your right to a jury trial and to bring a class action in court.";
        let findings = analyze(doc);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].category, "forced_arbitration");
    }

    #[test]
    fn poison_index_is_bounded() {
        let finding = ClauseFinding {
            category: "forced_arbitration".into(),
            label: "Forced Arbitration & Class Waiver".into(),
            rule_id: "forced_arbitration.class_action_waiver".into(),
            raw_text: "waive class action at our sole discretion".into(),
            modality: Modality::Waiver,
            severity: 5.0,
            start_offset: 0,
            end_offset: 0,
            opt_out_available: false,
            opt_out_target: None,
            inversion: InversionShape::None,
            reason: "test".into(),
        };
        let many = vec![finding; 40];
        assert_eq!(poison_index(&many), 100.0);
    }

    #[test]
    fn oversize_document_is_truncated_and_keeps_offset_invariants() {
        let clause = "You waive any right to a class action. ";
        let text = clause.repeat(MAX_INPUT_BYTES / clause.len() + 10);
        let report = analyze_with(&RuleRegistry::builtin(), &text);

        assert!(report.truncated);
        assert!(!report.findings.is_empty());
        for finding in &report.findings {
            assert!(finding.end_offset <= MAX_INPUT_BYTES);
            assert_eq!(
                &text[finding.start_offset..finding.end_offset],
                finding.raw_text
            );
        }
    }

    #[test]
    fn input_at_the_cap_is_not_truncated() {
        let text = "a".repeat(MAX_INPUT_BYTES);
        let (prefix, truncated) = truncate_input(&text, MAX_INPUT_BYTES);
        assert!(!truncated);
        assert_eq!(prefix.len(), MAX_INPUT_BYTES);
    }

    #[test]
    fn truncation_never_splits_a_character() {
        let mut text = "a".repeat(MAX_INPUT_BYTES - 1);
        text.push('é');
        assert!(text.len() > MAX_INPUT_BYTES);

        let (prefix, truncated) = truncate_input(&text, MAX_INPUT_BYTES);
        assert!(truncated);
        assert_eq!(prefix.len(), MAX_INPUT_BYTES - 1);
        assert!(text.is_char_boundary(prefix.len()));
        assert!(prefix.chars().all(|character| character == 'a'));
    }

    #[test]
    fn runtime_registry_can_add_a_category() {
        const SYNTHETIC: RuleSpec = RuleSpec {
            rule_id: "canary.chirp",
            category_id: "canary",
            label: "Canary Category",
            modality: Modality::Obligation,
            weight: 2.0,
            required: &[
                Predicate::AnyPhrase(&["canary clause"]),
                Predicate::AnyPhrase(&["must chirp"]),
            ],
            optional: &[],
            prohibited: &[],
            proximity_window: 20,
            inversion_sensitive: false,
        };

        let mut registry = RuleRegistry::builtin();
        registry.add(SYNTHETIC);
        let report = analyze_with(&registry, "The canary clause must chirp twice.");
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].category, "canary");
        assert_eq!(report.findings[0].rule_id, "canary.chirp");
    }
}
