# Wire Contract v1

This document is the boundary between the Rust engine and everything written in
TypeScript. The Rust source of truth is `engine/src/model.rs`; the TypeScript
mirror is `extension/src/types.ts`. Change both in the same commit and bump
`schema_version` when an existing consumer would break.

All JSON uses **snake_case**. The canonical representation is the Rust
`serde_json` string returned by `LegalEngine.analyze_json`. The worker parses
that string; it does not reshape it.

## `ClauseFinding`

One finding per category per clause. Findings are sorted by `start_offset`, then
by the deterministic order of rules in the registry.

| Field | Type | Meaning |
| :--- | :--- | :--- |
| `category` | string | Stable machine id, e.g. `forced_arbitration`. |
| `label` | string | Human label, e.g. `Forced Arbitration & Class Waiver`. |
| `rule_id` | string | The concrete rule that fired, e.g. `forced_arbitration.class_action_waiver`. |
| `raw_text` | string | Exact clause slice, trimmed of surrounding whitespace. |
| `modality` | `obligation` \| `permission` \| `prohibition` \| `waiver` | Deontic edge carried by the clause. |
| `severity` | number | Base category weight `w_k` in `[1.0, 5.0]`. **Not** the poison-index contribution. |
| `start_offset` | integer | Inclusive byte offset of `raw_text` in the exact input string. |
| `end_offset` | integer | Exclusive byte offset of `raw_text` in the exact input string. |
| `opt_out_available` | boolean | The clause, or one unambiguous neighbouring clause in the same block, states an opt-out action *and* a window. See *Opt-out semantics*. |
| `opt_out_target` | string? | Verified email address, or omitted. Only present when `opt_out_available` is true. |
| `inversion` | see below | Effect of exception markers on this clause. |
| `reason` | string | One sentence explaining which matched data made the rule fire. |

### Offsets

Offsets are **UTF-8 byte offsets**, always on character boundaries. They index the
string passed to the engine. When the engine truncates oversize input, findings
only cover the prefix and their offsets remain valid for the original string.

The hard cap is **512 KiB** (524,288 bytes, roughly 80,000 words). Input beyond
the cap is not analysed: the engine cuts on a character boundary, sets
`truncated`, and returns findings for the prefix only. A caller should surface
that state rather than retrying with the same text.

`raw_text == input[start_offset..end_offset]` is an invariant, asserted by tests.

### Inversion values

- `none` — no marker, or the rule did not opt into inversion sensitivity.
- `canceled` — an even number of markers; polarity stands and the rule may fire.
- `inverted` — an odd number of markers flipped a positive predicate. The rule
  is suppressed and therefore not normally present on the wire; the shape exists
  for diagnostic tests.
- `compliance_then_carve_out` — a negated/compliant base followed by a hostile
  carve-out. This is a distinct hostile shape and always fires.

Markers inside quoted text are ignored.

### Opt-out semantics

`opt_out_available` means the clause contains both an opt-out action (`opt out`,
`reject this arbitration`, `written notice`, …) and a window (`within 30 days`,
`no later than sixty (60) days`, `at any time`, …). A bare email address with no
stated window never qualifies and `opt_out_target` is absent.

`opt_out_target` is the first valid address in the clause, lowercased and rebuilt
from its parts. Common obfuscation (`[at]`, `(at)`, ` at `, `[dot]`, `(dot)`,
` dot `) is accepted. Malformed addresses and prose such as `email us at
example.com` do not produce a target. The action and the window must appear in
the same sentence; that sentence is the *opt-out clause*.

A finding's opt-out is resolved in this order:

1. If the finding's own clause is an opt-out clause, it is used.
2. Otherwise, opt-out clauses within 3 sentences of the finding **and** in the
   same block (no blank line between them) are candidates, excluding any that
   name a foreign subject (`marketing`, `cookie`, `personal data`, …).
3. If any candidate names dispute resolution (`arbitration`, `this provision`,
   …), only those candidates count.
4. If the remaining candidates name more than one distinct address, the
   association is ambiguous: `opt_out_available` is `false`.

A neighbouring association appends `; opt-out stated in a neighbouring clause of
the same block` to `reason`, so the claim is always explainable from the report.
The field shapes did not change, so `schema_version` stays `"1.0"`.

The engine detects the address; generating mail is out of scope for the engine.

## `DocumentReport`

| Field | Type | Meaning |
| :--- | :--- | :--- |
| `schema_version` | string | `"1.0"` for this contract. |
| `findings` | `ClauseFinding[]` | Sorted, as above. |
| `poison_index` | number | Document-level `P_I`, clamped to `[0, 100]`, rounded to two decimals. |
| `truncated` | boolean | Input exceeded the 512 KiB cap; only a prefix was analysed. |

`P_I = min(100, Σ w_k · μ(C_k) · τ(C_k))`, computed once at document level.
`μ` is `2.0` when the clause grants unilateral discretion, else `1.0`. `τ` is
`0.1` when an opt-out window is present, else `1.0`.

## Canonical response envelope

`analyze_json` never throws for user text. It returns one of:

```json
{"status":"ok","report":{"schema_version":"1.0","findings":[],"poison_index":0.0,"truncated":false}}
```

```json
{"status":"error","error":{"code":"invalid_input","message":"..."}}
```

`code` is one of `internal_serialization` or `invalid_input`. The worker checks
`status` and forwards a typed `EngineError`; it never guesses from a string.

## Worker messages

The worker lives in `extension/src/worker.ts` and speaks these shapes.

### Requests

```ts
type WorkerRequest =
  | { type: 'ANALYZE_TEXT'; text: string }
  | { type: 'PLAYBACK_FIXTURE' };
```

`PLAYBACK_FIXTURE` returns a bundled report so UI work needs no Rust toolchain.

### Responses

```ts
type WorkerResponse =
  | { type: 'ANALYSIS_RESULT'; report: DocumentReport }
  | { type: 'ANALYSIS_ERROR'; error: EngineError };
```

The worker unwraps the canonical envelope into `ANALYSIS_RESULT`; it does not
expose the envelope to the content script.

### Storage record

The content script stores exactly one record per scan under
`chrome.storage.local["last_scan"]`:

```ts
interface ScanRecord {
  url: string;
  scanned_at: string; // ISO timestamp owned by the UI, never part of the report
  report: DocumentReport;
}
```

## Determinism

Identical input produces byte-identical `analyze_json` output across native and
Wasm builds. There are no timestamps, hash-map iteration orders, or
locale-dependent operations in the report. `scanned_at` lives only in
`ScanRecord`.
