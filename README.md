# Stellar Legacy

A generational starship strategy game in Rust + Macroquad. You are the standing
council of a generation ship — captains age out, heirs inherit, and every promise
the ship makes will be kept (or broken) by someone else's grandchildren.

- **Design:** `gdd.md` (authoritative — pillars, systems, formulas, milestones)
- **Handoff / roadmap:** `PLAN.md` (what's built, what's next, conventions)
- Port of the web original `game_apps/stellar_legacy/` (React/PHP); all game rules
  now live in a deterministic Rust simulation, saves are local toolkit slots.

## Layout

```
src/
├── main.rs              # entry + STELLAR_LEGACY capture harness (scenes: menu/gameplay/event)
├── game.rs              # Game struct: state machine, UiAction dispatch, tick driver
├── state.rs, state/     # GameState (Menu/Gameplay), SimState (all serializable campaign state)
├── data.rs, data/       # serde types for assets/*.json, embedded via include_str!
├── simulation.rs, simulation/  # stateless services: tick, succession, events, contract, market
├── chronicle.rs         # cross-playthrough contract log (persists outside save slots)
├── save.rs              # save slots + migration hook
└── ui.rs, ui/           # terminal-styled screens; pure view layer returning UiAction
assets/                  # all content/balance data (events, legacies, contracts, components, names)
```

## Run / test / verify

```powershell
cargo run
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt -- --check
.\scripts\capture_ui.ps1 -Scenes menu,gameplay,event   # headless UI screenshots
.\publish.ps1                                           # build Windows + WebGL, deploy
```
