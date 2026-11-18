# AGENTS.md

PoisonPill is a local-first Chrome Manifest V3 extension that flags predatory contract clauses using a Rust→WebAssembly deontic parser. It must never make a network request.

`PoisonPill Project Documentation (1).md` is the product spec: taxonomy, formulas, directory layout, and illustrative code. Read it before changing behaviour. Its snippets are partial examples, not the shipped implementation — this file describes the actual repo.

## Layout

| Path | Role |
| :--- | :--- |
| `engine/` | Rust crate compiled to `wasm-bindgen`. The only place detection logic lives. |
| `engine/src/lib.rs` | Module wiring and re-exports; the wasm binding module is scoped separately. |
| `engine/src/model.rs` | Wire contract v1: `ClauseFinding`, `DocumentReport`, typed errors. |
| `engine/src/tokenizer.rs` | Linear sentence splitter with offsets and abbreviation guards. |
| `engine/src/deontic.rs` | Engine entry point, sentence walk, input cap, finding assembly. |
| `engine/src/optout.rs` | Opt-out action/window/address detection. |
| `engine/src/exceptions.rs` | Boundary-marker inversion calculus (`InversionShape`). |
| `engine/src/lexicon.rs` | Phrase data (actors, verbs, assets, guards) the content engineer extends. |
| `engine/src/rules.rs` | Declarative `RuleSpec` data and the category-agnostic evaluator. |
| `engine/src/scoring.rs` | Document-level `P_I`, `μ`, `τ`. |
| `engine/src/bin/analyze.rs` | Native CLI; prints the canonical envelope and generates goldens. |
| `engine/tests/fixtures.rs` | Corpus/golden harness and offset invariants. |
| `engine/tests/rule_format.rs` | Runtime registration proof for the declarative rule format. |
| `engine/tests/budgets.rs` | Ignored 10k-word release latency budget. |
| `engine/tests/labelled.rs` | Labelled dev-set precision guard and per-rule recall report. |
| `fixtures/labelled/` | Clause-level dev set (`clauses.tsv`); not held out. |
| `fixtures/cases/` | Positive, hard-negative and ordinary-negative regression corpus. |
| `fixtures/golden/` | Exact canonical envelopes; never written by tests. |
| `fixtures/heldout/` | One-shot corpus and its recorded `REPORT.md`. |
| `extension/src/` | TypeScript: `content.ts`, `worker.ts`, `popup.ts`, `types.ts`. |
| `extension/public/` | Static assets plus the committed `engine.js`/`engine.wasm` and `playback.json`. |
| `manifest.json` | MV3 manifest, kept at the repo root. |
| `package.json`, `tsconfig.json` | TypeScript typecheck + esbuild shell build. |
| `scripts/` | Wasm parity, measurement, held-out, golden and size scripts. |
| `docs/` | `CONTRACT.md`, `RULES.md`, `DECISIONS.md`, `KNOWN_LIMITS.md`, `BACKLOG.md`, `HANDOFF.md`, `MEASUREMENTS.md`. |
| `Makefile` | Build and verification entry points. |

## Build

```sh
npm install       # once: TypeScript, @types/chrome, esbuild
make engine       # cargo + wasm-pack -> extension/public/{engine.js,engine.wasm}
make extension    # esbuild -> ./dist
make verify       # fmt, clippy, tests, size/latency budgets, native/Wasm parity, typecheck
make goldens      # explicit golden regeneration; review the diff
```

`dist/` is the unpacked extension (manifest, icons, compiled JS, public assets flattened to its root). Load it with `chrome://extensions` → *Load unpacked*. Never commit `dist/`, `engine/target/`, or `engine/pkg/`.

Prerequisites: `wasm-pack` (`cargo install wasm-pack`), the `wasm32-unknown-unknown` target, and Node 22+. The built `extension/public/engine.js` and `engine.wasm` are committed, so extension work needs only Node; Rust is needed only when the engine changes.

## Invariants

- **Zero network egress.** No `fetch`, `XMLHttpRequest`, WebSocket, telemetry, CDN fonts, or remote stylesheets. The single `fetch` in `worker.ts` reads `chrome.runtime.getURL('playback.json')`, a local extension resource. `manifest.json` grants `storage` only — `host_permissions` stays empty.
- **Detection lives in Rust.** Content scripts extract text and display the stored report; they never classify clauses. Adding a heuristic in TypeScript is a design regression.
- **The Wasm contract is snake_case JSON.** The Rust source of truth is `engine/src/model.rs`; `extension/src/types.ts` mirrors it exactly. `ClauseFinding` carries `category`, `label`, `rule_id`, `raw_text`, `modality`, `severity`, `start_offset`, `end_offset`, `opt_out_available`, `opt_out_target`, `inversion`, `reason`. Offsets are UTF-8 byte offsets into the exact input. `opt_out_target` is omitted when absent. See `docs/CONTRACT.md`.
- **Analysis runs off the main thread.** All parsing goes through `worker.js`; the content script only posts text and consumes the report.
- **`wasm-unsafe-eval` stays in the CSP.** Removing it breaks Wasm instantiation; widen nothing else.
- **The content script writes nothing to the page.** No badges, highlights, shields, jump-to-clause or theming. The popup renders the stored report in its own document only. Those other features are product work layered on the contract, not part of this base.
- **Input work is bounded.** The engine analyses at most 512 KiB, cuts on a UTF-8 boundary, and sets `truncated`.
- **Goldens never auto-update.** Tests compare; only `make goldens` writes. Held-out documents are run at the Wasm milestone and are never tuned. The one re-run was a serialization-only fix (`-0.0` → `0.0`); the recorded diff is the artifact digest and those values, with no finding changes.

## Conventions

- Rust: edition 2021, `#![deny(unsafe_code)]` unless a comment justifies it, no panics on user text (`unwrap` only on serialisation of owned structs).
- Severity weights come from `RuleSpec.weight` in `rules.rs` and follow the spec table (arbitration `5.0` … surveillance `3.0`). Scores are normalised into `P_I ∈ [0, 100]` at the document level, never per clause. Only Forced Arbitration is authored; the other four categories are empty slots. See `docs/RULES.md`.
- An opt-out clause needs an action and a window in the same sentence. A finding uses its own opt-out clause, else one unambiguous neighbour within 3 sentences in the same block (`optout::associate`; see `docs/CONTRACT.md`). `optout.rs` owns address extraction and association; TypeScript only displays the result and never generates mail.
- Keep `engine` free of Chrome APIs so it stays testable with plain `cargo test`.
- TypeScript: strict mode, no `any` at the worker boundary, explicit `MessageEvent` types.

## Gotchas

- `.gitattributes` pins LF. Offsets are byte offsets, so a CRLF checkout (Git for Windows with `core.autocrlf=true`) would shift every golden; do not remove it.
- On Windows without Visual Studio Build Tools, the MSVC Rust toolchain cannot link. Use `stable-x86_64-pc-windows-gnu` (`rustup toolchain install stable-x86_64-pc-windows-gnu -t wasm32-unknown-unknown`, then `RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-gnu`). Building `wasm-bindgen-cli` from source there also needs a MinGW assembler; installing the prebuilt `wasm-pack`/`wasm-bindgen` release binary is simpler.

- `manifest.json` references the flattened `dist/` root, so asset paths (`icons/…`, `popup.html`) are relative to `dist`, not `extension/`.
- `engine.js`/`engine.wasm` are committed despite living in `public/`. After changing Rust, run `make engine` and commit both files; `make parity` tests the committed bytes.
- Chrome loads `worker.js` as a classic worker, and content/popup scripts are classic scripts. `tsconfig.json` is typecheck-only (`noEmit`); esbuild emits IIFE bundles. Never emit ES modules or CommonJS `exports` references for these entry points.
- The executable entry points are `extension/src/content.ts`, `worker.ts`, and `popup.ts`; `types.ts` is type-only and is not bundled.
- The spec's `$P_I$` weighting uses per-category `w_k`; the taxonomy table is authoritative when numbers disagree.
- SHA-256 tree-diff/audit memory is not implemented in this base. If added later, store digests and offsets under `chrome.storage.local`, not full contract text.
