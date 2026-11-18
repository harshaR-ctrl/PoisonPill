//! Labelled clause set: per-rule recall and a hard precision guard.
//!
//! `fixtures/labelled/clauses.tsv` is a development set, not a held-out one.
//! `negatives_never_fire` is always on. `report_recall` is ignored by default
//! and prints the table recorded in `docs/MEASUREMENTS.md`.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use poisonpill_engine::analyze;

const CATEGORY: &str = "forced_arbitration";

fn labelled() -> Vec<(String, String)> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/labelled/clauses.tsv");
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    text.lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .map(|line| {
            let (label, clause) = line
                .split_once('\t')
                .unwrap_or_else(|| panic!("malformed labelled line: {line}"));
            (label.trim().to_string(), clause.trim().to_string())
        })
        .collect()
}

#[test]
fn labelled_set_is_well_formed() {
    let rows = labelled();
    let negatives = rows.iter().filter(|(label, _)| label == "none").count();
    let positives = rows.len() - negatives;
    assert!(positives > 0, "the labelled set needs positives");
    assert!(
        negatives * 2 >= positives,
        "keep at least one near miss per two positives ({negatives} vs {positives})"
    );
}

#[test]
fn negatives_never_fire() {
    let false_positives: Vec<String> = labelled()
        .into_iter()
        .filter(|(label, _)| label == "none")
        .filter_map(|(_, clause)| {
            let findings = analyze(&clause);
            (!findings.is_empty()).then(|| format!("{} <- {clause}", findings[0].rule_id))
        })
        .collect();
    assert!(
        false_positives.is_empty(),
        "labelled near misses fired:\n{}",
        false_positives.join("\n")
    );
}

#[test]
#[ignore = "measurement: run with --ignored --nocapture"]
fn report_recall() {
    // rule -> (cases, category hits, rule hits)
    let mut table: BTreeMap<String, (usize, usize, usize)> = BTreeMap::new();
    let mut misses = Vec::new();

    for (label, clause) in labelled() {
        if label == "none" {
            continue;
        }
        let findings = analyze(&clause);
        let category_hit = findings.iter().any(|finding| finding.category == CATEGORY);
        let rule_id = format!("{CATEGORY}.{label}");
        let rule_hit = findings.iter().any(|finding| finding.rule_id == rule_id);

        let row = table.entry(label.clone()).or_default();
        row.0 += 1;
        row.1 += usize::from(category_hit);
        row.2 += usize::from(rule_hit);
        if !category_hit {
            misses.push(format!("{label}: {clause}"));
        }
    }

    println!("| Rule | Cases | Category hits | Rule hits |");
    println!("| :--- | ---: | ---: | ---: |");
    for (label, (cases, category, rule)) in &table {
        println!("| `{label}` | {cases} | {category} | {rule} |");
    }
    if !misses.is_empty() {
        println!("\nmisses:");
        for miss in misses {
            println!("- {miss}");
        }
    }
}
