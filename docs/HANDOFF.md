# Handoff

You are picking up PoisonPill at `v0.1.0-base`. This document is the short path
from "the repo is here" to "the next change is merged." Everything else is
reference material linked below.

## What exists

A working mechanism, not a product. The Rust→Wasm engine detects one category
(Forced Arbitration) deterministically and reports a canonical JSON envelope. A
reference MV3 shell runs it in a worker and shows the last report in a plain
popup. There is no highlighting, history, mail generation, or second category.

`make verify` is green at the tag: fmt, clippy, unit/integration tests, 32/32
native/Wasm byte parity, a 2 MiB size gate, a 200 ms 10k-word budget, and the
TypeScript typecheck. Numbers and environment are in
[`MEASUREMENTS.md`](MEASUREMENTS.md).

## Read in this order

| Document | Why |
| :--- | :--- |
| [`../README.md`](../README.md) | Build commands and what the base deliberately omits. |
| [`../AGENTS.md`](../AGENTS.md) | Repo map, invariants, gotchas. Non-negotiable. |
| [`DECISIONS.md`](DECISIONS.md) | Why the contract, rule format, opt-out scope and shell look like this. |
| [`KNOWN_LIMITS.md`](KNOWN_LIMITS.md) | The gaps you are inheriting, with held-out evidence. |
| [`BACKLOG.md`](BACKLOG.md) | The full ordered list of remaining work. |
| [`CONTRACT.md`](CONTRACT.md), [`RULES.md`](RULES.md) | Touch these before changing the wire format or adding a rule. |

## Start here: the first three tasks

> **Status (post-`v0.1.0-base`):** all three are implemented — see decisions 16
> and 17 in [`DECISIONS.md`](DECISIONS.md) and the post-base section of
> [`MEASUREMENTS.md`](MEASUREMENTS.md). Held-out `01`, `04`, `09`, `14` report
> the opt-out and `06` fires. The next pick is the backlog's item 1 (a second
> category). The text below is kept as the record of what was asked.

Do them in this order unless you have a reason not to. Each is bounded and each
has an acceptance test you can write before the implementation.

### 1. Cross-clause opt-out association

**Why first.** It is the largest user-visible recall gap and it does not require
a new category or a new lexicon. Four held-out documents (`01`, `04`, `09`, `14`)
offer an opt-out in a sentence separate from the arbitration finding, so all 15
held-out findings say `opt_out_available: false`.

**Shape of the change.** `engine/src/optout.rs` currently requires action and
window in one sentence. Introduce a bounded association, for example a window and
address in the same paragraph/section with no competing address. Keep it
explainable: the finding's `reason` must still be derivable from matched data.

**Acceptance.** Held-out `01`, `04`, `09`, `14` report `opt_out_available: true`
with the right target; a new hard negative where an unrelated privacy address
sits near an arbitration clause does **not**. Update `KNOWN_LIMITS.md` and
`MEASUREMENTS.md` when it lands.

### 2. Broaden mandatory-arbitration phrasing

**Why.** `06-marketplace-terms.txt` says "shall be settled by arbitration" and
produces no finding. That is a plain lexicon gap, not a mechanism failure.

**Shape of the change.** Add phrases to `MANDATORY_ARBITRATION` in
`engine/src/lexicon.rs`. Likely candidates: "settled by arbitration", "referred
to arbitration", "subject to arbitration". Do not touch the matcher.

**Acceptance.** Held-out `06` fires `mandatory_arbitration`; benign settlement
language ("the parties settled ...") and "arbitration is available" style
sentences stay hard negatives. The 2:1 hard-negative ratio test stays green.

### 3. Prove or fix `court_access_waiver` and `class_action_bar`

**Why.** Both are fixture-positive but produced zero held-out hits. Before adding
more rules, find out whether the rules are wrong or the corpus simply never
phrases those clauses.

**Shape of the change.** Build a small labelled set of real court-access and
class-bar clauses, run them through the native CLI, and either add lexicon
phrases or narrow the rule's claim in `RULES.md`.

**Acceptance.** A written number in `MEASUREMENTS.md` (hits vs cases) and a
decision recorded in `DECISIONS.md`. Deleting a rule is a valid outcome if it
cannot be made precise.

## After that

Pick from [`BACKLOG.md`](BACKLOG.md). The intended order is: author a second
category (needs its own 2:1 hard-negative corpus), then a precision/recall
harness on labelled text, then the product layer (annotation, opt-out drafting,
history). Do not start the product layer before the contract stops moving.

## Rules of the road

- **Detection lives in Rust.** A classification heuristic in TypeScript is a
  design regression. The content script extracts text and stores a report.
- **Detection changes need artifacts.** After changing Rust: `make engine`, then
  `make verify`, then commit `extension/public/engine.js` and `engine.wasm`
  together with the source. `make parity` compares the shipped bytes to native.
- **Goldens are written only by `make goldens`.** Tests never write them. Read
  the diff before committing it.
- **Hard negatives at 2:1.** Enforced by a test; a new category without them is
  not ready.
- **Schema changes are one commit.** `engine/src/model.rs` and
  `extension/src/types.ts` move together, and `schema_version` bumps if an
  existing consumer breaks.
- **The held-out corpus is frozen.** Do not tune against it. If you need a new
  held-out set, create a second one and run it once; leave the first as the
  honest record of this milestone.
- **No network.** No `fetch` beyond the local `playback.json`, no remote fonts or
  styles, `host_permissions` stays empty, permissions stay `storage` only.
- **The input cap is a promise.** 512 KiB, cut on a UTF-8 boundary, `truncated`
  set. Offsets are UTF-8 byte offsets into the original string and
  `raw_text == input[start..end]` must hold.

## Verify before you push

```sh
make verify
git status --short          # expect only the files you meant to change
```

If parity fails, the committed Wasm is stale — rerun `make engine`. If a golden
diff appears that you did not intend, stop and read it; it is a behaviour change.
