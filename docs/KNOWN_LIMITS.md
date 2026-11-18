# Known Limits

Honest boundaries of `v0.1.0-base`. These are properties of the shipped design,
not bugs discovered late. Where the held-out run shows a consequence, it is
quoted from [`fixtures/heldout/REPORT.md`](../fixtures/heldout/REPORT.md).
Backlog items that would change these limits are in
[`BACKLOG.md`](BACKLOG.md).

## Detection

### The lexicon decides recall

Matching is ASCII-case-insensitive substring search over the phrase lists in
`engine/src/lexicon.rs`. If a predatory clause is phrased with words that are not
in those lists, it does not fire. There is no stemming, no lemmatisation, and no
semantic model.

Visible consequence at `v0.1.0-base`: `06-marketplace-terms.txt` said a dispute
"shall be settled by arbitration" and produced no finding. The phrase has since
been added with voluntary-arbitration guards (decision 17), but the general
point stands: any phrasing outside the lists is invisible.

Because the match is substring-based, a phrase must be specific enough to
explain itself. Broad phrases such as `court` are only safe when paired with a
waiver verb and a proximity window, and even then they can drift.

### Only English, only ASCII case folding

No Unicode case folding, no accented or translated terms. In
`13-unicode-translation.txt` the French sentence "Vous waivez tout droit à un
procès avec jury" contributes nothing; the document is found only because its
English sentence contains "shall be resolved by binding arbitration".

### Patterns are shallow

"Predatory" is approximated by shape: a waiver verb near a protected interest, a
mandatory-arbitration phrase near a dispute word, or a class-action term near a
bar. The engine does not understand who benefits, whether the term is
enforceable, or whether local law voids it. Hard negatives reduce obvious
false positives (provider waivers, compliance boilerplate, descriptive mentions,
quoted text, "does not waive") but cannot cover every paraphrase.

### One finding per category per clause

A category contributes at most one finding per sentence; the first matching rule
in registry order wins. In `01-online-course-terms.txt` the sentence "You and the
company each waive the right to a jury trial and the right to participate in a
class action" reports `class_action_waiver` only. The jury waiver in the same
sentence is real but not represented. Separate sentences in
`11-subscription-bullets.txt` report both.

### `court_access_waiver` and `class_action_bar` recall is dev-set only

On the labelled dev set (`fixtures/labelled/`) both rules now hit every
realistic phrasing (10/10 each) with no near miss firing; before the lexicon
work they hit 3/10 each. Those are development numbers: the lexicon was extended
while looking at that set. The frozen held-out corpus still records zero hits for
both, partly because the clauses there that mention court are either descriptive
or already claimed by an earlier rule in the same category (one finding per
category per clause). Treat their real-world recall as plausible, not measured.

### Precision and recall are unmeasured

The regression corpus is curated, not sampled. It proves invariants and guards
against regressions; it is not a precision/recall estimate. No labelled
population of real contracts has been scored, so no accuracy percentage is
quoted anywhere in these docs.

### One category

Only `forced_arbitration` is authored. Unilateral change, liability caps,
surveillance/data cascades, and termination asymmetry have empty registry slots
and no detection. Absence of a finding says nothing about those categories.

## Opt-out

### Block-bounded association

An opt-out clause still needs an action and a window in one sentence. Since
decision 16 a finding may borrow the opt-out of a clause up to three sentences
away in the same block (no blank line between them), unless that clause names a
foreign subject (marketing, cookies, personal data, …) or the neighbourhood
offers two different addresses. Held-out `01`, `04`, `09` and `14` now report
the opt-out. Still missed: an opt-out in a separate section after a heading and a
blank line, one more than three sentences away, and one whose subject is only
implied by a heading. The foreign/dispute subject lists are lexicon data with the
same substring limits as every other list.

### Action plus window, or nothing

A bare address, an address without a window, or a window without an action never
qualifies. The engine does not infer a window from context, a linked policy, or a
jurisdiction's default. It also does not verify deliverability or ownership of an
address.

### Detection only

The engine reports that an opt-out exists and where to send it. It never drafts
the notice, fills a template, or opens `mailto:`. Those are out of scope for the
engine and are not built in the reference shell.

## Input and runtime

### The engine cap is 512 KiB; the shell caps lower and silently

The engine truncates at 524,288 bytes on a character boundary and sets
`truncated`. The reference content script caps extracted text at 120,000 UTF-16
code units *before* handing it over, so the engine never sees an oversize input
and `truncated` stays `false` in normal browsing. Treat the shell's cutoff as
silent: a huge page is under-reported without a flag in the stored record.

### Extraction is shallow

The content script reads `document.body.innerText`. It does not read shadow DOM,
same-origin iframe bodies (the script runs with `all_frames: false`), canvas, or
PDF viewer surfaces. A contract rendered inside a PDF or a cross-origin embed is
not scanned. The page must also pass a cheap `looksLegal()` heuristic (legal-ish
URL, a checkbox, or a terms/privacy link) and be at least 200 characters; other
documents are skipped without a report.

### Pages that mutate are not re-scanned

Extraction happens once at `document_idle`. A single-page app that renders terms
later, or a "load more" expansion, is not re-read. There is also no re-scan
action in the popup.

### Latency numbers are not a browser guarantee

The 10k-word measurements in [`MEASUREMENTS.md`](MEASUREMENTS.md) come from Node
on one machine. The native release path and the Wasm path are not compared for
speed, only for byte parity. A low-end device will be slower, and the 200 ms gate
is a native test, not a Wasm profile in Chrome.

## Determinism and delivery

- The report is deterministic, but the *scan record* is not: `scanned_at` is a
  wall-clock timestamp owned by the UI. Two scans of the same page differ in that
  field and in `url`.
- `poison_index` is rounded once at document level. Recomputing a per-clause
  breakdown from `severity` will not exactly reconstruct it, because `μ` and `τ`
  are applied per finding before clamping.
- The committed Wasm bytes are the shipped engine. If they are rebuilt without
  regenerating goldens, `make parity` fails rather than silently shipping a
  different engine.
- `wasm-opt` is disabled, so the artifact is larger than it could be. This is a
  size limit, not a correctness one.

## Product surface

The reference shell deliberately shows a plain list of the last report. There is
no highlighting, badge, shield, jump-to-clause, history, comparison, theming, or
audit memory. Those are product features listed in [`BACKLOG.md`](BACKLOG.md);
their absence is not a defect of the contract.
