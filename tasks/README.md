# Implementation briefs — generational-voyage redesign

These briefs decompose `plan.md` (the design record) into self-contained workstreams.
Each brief can be executed **without reading `plan.md`** — it restates every decision that
binds it. If code you find contradicts a brief, **stop and report; do not guess.**

## Execution order (do not reorder)

| # | Brief | Depends on |
| --- | --- | --- |
| 1 | `W1-rescale.md` — 300–600 yr charters, decay retune, autoplay soak harness | — |
| 2 | `W3-month-clock.md` — month-resolution time, speed selector, dated events | W1 |
| 3 | `W2-phases.md` — real travel/operation/return phases, quantified objectives, early return | W3 |
| 4 | `W7-factions.md` — 6 founding factions, pick 3, faction loss + replacement | W2 |
| 5 | `W4-prep-launch.md` — PREP screen, provisioning (food/fuel), explicit LAUNCH | W7 |
| 6 | `W5-subsystems.md` — six ship subsystems, decay + knowledge-gated repair, reactive upgrades | W4 |
| 7 | `W6-events.md` — family/phase-tagged event catalog, seeded campaign skeleton | W5 |

Complete one brief fully (all acceptance criteria green) before starting the next.
Each brief ends with the same verification commands; run them all before considering
a brief done.

## Ground rules (repeated in every brief; they are hard constraints)

1. **Data-driven:** all content and tuning live in `assets/*.json`. Never hardcode
   missions, events, factions, or balance numbers in Rust.
2. **800-line hard limit** on every `.rs` file (soft limit 600). When a file grows,
   extract a cohesive sibling module (`foo.rs` + `foo/` directory — **never** create
   a `mod.rs`).
3. **UI is a pure view.** Panels read `&SimState` and return `UiAction` variants
   (`src/ui.rs:149`); all mutation happens in the dispatcher (`src/game/actions.rs`)
   or the stateless services in `src/simulation/`.
4. **Determinism:** all randomness flows through `sim.rng` (`SeededRng`). Never use
   `rand`, wall-clock time, or any unseeded entropy inside the simulation.
5. **Old saves are abandoned** (owner decision 2026-07-21). Do not write migration
   shims or `#[serde(default)]` fields *for compatibility reasons*; restructuring
   `SimState` freely is allowed. (`#[serde(default)]` is still fine where a field is
   genuinely optional in authored JSON.)
6. Delete unused code outright; never keep `_`-prefixed dead fields.
7. Match existing style — this repo's modules, naming, and comment density.

## Verification (identical for every brief)

```powershell
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt -- --check
cargo check --release --target wasm32-unknown-unknown
```

Only after the final brief (W6): `.\publish.ps1` and verify at
`http://127.0.0.1/stellar_legacy/`.
