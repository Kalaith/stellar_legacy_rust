# Procedural ship blueprint — refinement loop log

Self-paced `/loop`: refine the underway ship schematic (`src/ui/ship_schematic.rs`)
for clarity, hierarchy, usability, and a modular, engineering-schematic feel.
Rules: commit at the end of each iteration; every 5th iteration add a new system.
Each entry: what was preserved, the weakness found, the improvement made.

Verify with `.\scripts\capture_ui.ps1 -Scenes ship_underway`, plus
`cargo fmt/clippy/test`.

---

## Iteration 1 — modular grid + corridor bus
- **Preserved**: hull silhouette per class, three-signal highlighting (condition
  colour / tier size+pips / manned pip), status strip, legend, salvage panel.
- **Weakness**: compartments sat at six hard-coded band positions (couldn't add/
  remove/rearrange rooms); arbitrary interior "greeble" hatch lines added noise;
  boxes floated without a connective structure.
- **Improvement**: subsystem rooms now flow into a modular two-deck grid whose
  column count derives from the compartment count (`cols = n.div_ceil(2)`), so
  rooms reflow without breaking. Added a twin-line **corridor** (bow→stern) with a
  **branch connector** from every compartment, so the layout reads as a wired
  schematic. Removed the greeble hatches.
- **Open weaknesses** (next): dorsal weapon label crowds the centre-top subsystem
  caption; flat typography (no size/weight hierarchy); no standardized per-room
  icons or short-codes; label leaders + branch stubs both present (could unify).

## Iteration 2 — standardized tags + typographic hierarchy
- **Preserved**: modular grid, corridor + branches, highlighting, status strip.
- **Weakness**: rooms carried only long external captions (flat type, slow to
  scan); the dorsal weapon caption collided with the centre-top subsystem label.
- **Improvement**: each compartment now bears a standardized 3-letter tag inside
  the box (AGR/EDU/ENG/LSH/MED/SEC, plus CMD/DRV/WPN), drawn in the condition
  tone so the code doubles as a health read. The weapon caption moved to the side
  of its dorsal box, clearing the collision. Type now splits cleanly: in-box tag
  (14px, primary) → external name (12px) → deck caption (10px, dim). New test
  asserts every subsystem has a real triliteral code.
- **Open weaknesses** (next): still no pictographic icons (tags are the interim);
  external full-name captions are now partly redundant with the tags; connection
  routing is straight stubs (no orthogonal "bus" routing); single hull class
  shown in captures (verify corvette/ark/ring adapt).
