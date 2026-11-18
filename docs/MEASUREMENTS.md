# Measurements

Recorded at the Wasm milestone (`v0.1.0-base`). Numbers are informative, not
assertions. The only enforced limits are the 2 MiB size gate
(`scripts/check-size.sh`) and the 200 ms native budget
(`engine/tests/budgets.rs`), both run by `make verify`.

## Environment

| Item | Value |
| :--- | :--- |
| CPU | AMD Ryzen 7 7435HS |
| OS | Linux 7.3.3-200.fc44.x86_64 (x86_64) |
| Rust | rustc 1.97.1 (2026-07-14) |
| wasm-pack | 0.15.1 |
| Node | v22.23.1 |
| Profile | release, `opt-level = "z"`, LTO, `panic = "abort"`, `strip` |
| `wasm-opt` | disabled (bundled version rejects the current bulk-memory module) |

## Artifact size

| Artifact | Bytes | Budget |
| :--- | ---: | :--- |
| `extension/public/engine.wasm` | 113,158 (110.5 KiB) | 2,097,152 (2 MiB) |
| `extension/public/engine.js` (glue) | 11,354 (11.1 KiB) | — |
| sha256 (`engine.wasm`) | `e5162b546eefc3778766ac802a5e1686f90cb433baac332acd7a9e6c123f9b80` | — |

The artifact is ~5% of the size budget, so several categories can be authored
before size becomes a concern.

## Latency: 10,070 words

Fixture: the benign maintenance sentence from `engine/tests/budgets.rs`, repeated
530 times. This document produces no findings, so the number measures scanning
and sentence assembly, not report size. `analyzeJson` also serialises the empty
report.

| Path | Min | Median | Mean | Max | Notes |
| :--- | ---: | ---: | ---: | ---: | :--- |
| Native release (`analyze`) | — | 41.99 ms | — | — | One timed run after warm-up, including `Vec<ClauseFinding>` assembly. |
| Wasm (`analyzeJson`) | 10.01 ms | 10.20 ms | 10.37 ms | 11.25 ms | 20 timed runs after two warm-ups, Node 22. |

The hard budget is 200 ms, so the native path has ~4.8× headroom and the shipped
Wasm path ~20×. The native number being slower than Wasm is expected: the native
test builds debug-friendly `String`/`Vec` evidence through the same generic
code, while the Wasm release build has smaller data to serialise (empty report)
and different allocator behaviour. Neither number is used for cross-runtime
comparison; parity is asserted on bytes, not timing.

## Corpus

| Check | Result |
| :--- | :--- |
| Regression cases | 32 (8 positive, 16 hard negative, 8 ordinary negative) |
| Native/Wasm parity | 32/32 byte-identical canonical JSON |
| Findings offset invariant | Holds for every golden (`raw_text == input[start..end]`) |
| Determinism | Repeated `analyze_json` is byte-identical for every case |
| Truncation through Wasm | Oversize input marked `truncated`, offsets stay valid, no panic on a split character |

## Held-out run

Fourteen held-out documents, written before this milestone, analysed once through
the committed Wasm artifact. Full reports and input digests are in
`fixtures/heldout/REPORT.md`.

| Metric | Result |
| :--- | ---: |
| Documents | 14 |
| Documents with at least one finding | 10 |
| Total findings | 15 |
| `forced_arbitration.mandatory_arbitration` | 7 |
| `forced_arbitration.class_action_waiver` | 5 |
| `forced_arbitration.jury_trial_waiver` | 3 |
| `forced_arbitration.court_access_waiver` | 0 |

Notable misses, recorded without tuning the corpus afterwards:

- `06-marketplace-terms.txt` uses "shall be settled by arbitration", which is not
  in `MANDATORY_ARBITRATION`; the clause is predatory but does not fire.
- Four documents (`01`, `04`, `09`, `14`) state an opt-out window in a sentence
  separate from the arbitration finding. Detection is clause-scoped, so all 15
  held-out findings report `opt_out_available: false` even where the document
  offers an opt-out. This is a cross-clause association gap, not a failure to
  find the address.
- `02-saas-dpa.txt`, `08-insurance-policy.txt` and `10-housing-agreement.txt`
  correctly produce no findings.

These are recall gaps in the authored lexicon, not mechanism failures. They are
listed in `docs/KNOWN_LIMITS.md` and `docs/BACKLOG.md` rather than fixed by
rewriting the held-out set.

## Post-base: opt-out association and lexicon breadth

Recorded after decisions 16 and 17. Different machine from the table above, so
latency numbers are only comparable within this section.

| Item | Value |
| :--- | :--- |
| OS | Windows 11 (x86_64) |
| Rust | rustc 1.100.0, `stable-x86_64-pc-windows-gnu` |
| Node | v24.12.0 |

### Latency (native release, `engine/tests/budgets.rs`, 3 runs each)

| Build | Result |
| :--- | ---: |
| `v0.1.0-base` source on this machine | 106–112 ms |
| Broadened lexicon, old `find_phrase` | 342–347 ms (**over** the 200 ms budget) |
| Broadened lexicon, first-byte-skip `find_phrase` | 18–21 ms |

The phrase count roughly doubled, and the naive scan checked every byte offset of
every sentence for every phrase. The rewrite skips to bytes that can begin the
phrase and drops the char-boundary checks for ASCII needles, where they are
always true. Every golden is byte-identical before and after the rewrite.

### Labelled dev set (`fixtures/labelled/clauses.tsv`)

32 positives, 21 near misses. Not a held-out set: the lexicon was extended while
looking at it.

| Rule | Cases | Hits at `v0.1.0-base` | Hits now |
| :--- | ---: | ---: | ---: |
| `mandatory_arbitration` | 12 | 2 | 12 |
| `court_access_waiver` | 10 | 3 | 10 |
| `class_action_bar` | 10 | 3 | 10 |
| Near misses firing (false positives) | 21 | 2 | 0 |

The two base false positives were "Our support team cannot answer questions
about class action settlements" (`class_action_bar`) and severability prose about
"the class action waiver … found unenforceable" (`class_action_waiver`).

### Regression corpus

| Check | Result |
| :--- | :--- |
| Regression cases | 47 (13 positive, 26 hard negative, 8 ordinary negative) |
| Existing goldens changed | 1: `positive/05-class-action-bar` `reason` now also cites the subject it matched (`"arbitrator"`) |
| Native/Wasm parity | 47/47 byte-identical canonical JSON |
| Wasm artifact | 119,689 bytes (116.9 KiB), sha256 `ea95f6b3139ad1b94fa9687d3dca31ead19222f985ff2c119612fd6a25dce697`; built with wasm-pack 0.15.1, `engine.js` glue unchanged |
| Wasm latency (`make measure-wasm`, Node 24) | median 9.95 ms, mean 10.09 ms, max 14.51 ms over 20 runs (10,070 words) |

### Held-out acceptance check (native, not a new estimate)

The handoff named held-out documents in the acceptance criteria and its text
quotes them, so this is an acceptance check, not a fresh held-out measurement.
`fixtures/heldout/REPORT.md` is untouched and remains the honest base record.

| Document | Base | Now |
| :--- | :--- | :--- |
| `01-online-course-terms.txt` | 2 findings, no opt-out | 2 findings, opt-out `legal@acme-learning.example` |
| `04-loan-agreement.txt` | 2 findings, no opt-out | 2 findings, opt-out (postal address, no email target) |
| `06-marketplace-terms.txt` | 0 findings | 1 `mandatory_arbitration` |
| `09-exchange-agreement.txt` | 2 findings, no opt-out | 2 findings, opt-out `arbitration@example.exchange` |
| `14-help-center.txt` | 2 findings, no opt-out | 2 findings, opt-out `optout@example.com` |
| `02`, `08`, `10`, `12` | 0 findings | 0 findings |
| `court_access_waiver` / `class_action_bar` hits | 0 / 0 | 0 / 0 |

Every other document's findings are unchanged. Documents with an associated
opt-out drop to `P_I = 1.0` because `τ = 0.1` applies.
