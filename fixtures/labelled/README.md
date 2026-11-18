# Labelled clause set

A small, clause-level development set used to measure rule recall and guard
precision for individual Forced Arbitration rules. It is **not** the held-out
corpus: it was written to probe specific rules and the lexicon was extended while
looking at it, so its numbers are optimistic development numbers, not an
accuracy estimate. `fixtures/heldout/` remains the frozen one-shot record.

## Format

`clauses.tsv`: one clause per line, `label<TAB>text`. Lines starting with `#`
and blank lines are ignored.

| Label | Meaning |
| :--- | :--- |
| `mandatory_arbitration`, `court_access_waiver`, `class_action_bar`, ... | The clause is predatory and this rule is the one it should exercise. |
| `none` | A near miss: the clause must produce no finding. |

A positive counts as a **category hit** when any `forced_arbitration` finding is
produced, and as a **rule hit** when the labelled rule is the one reported (a
clause reports one finding per category, chosen by registry order).

## Running

```sh
cargo test --manifest-path engine/Cargo.toml --test labelled -- --nocapture
```

`negatives_never_fire` always runs and fails on any false positive.
`report_recall` is ignored by default and prints the per-rule table recorded in
`docs/MEASUREMENTS.md`.
