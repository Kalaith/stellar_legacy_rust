# TODO — Stellar Legacy

Standing direction for content passes lives in `content_depth.md`; this file holds only
the discrete open items left behind by finished workstreams.

## Release gate

- Fix the stale onboarding copy: the welcome screen still says SPACE / ENTER advances
  time, but the current voyage uses real-time auto-advance with Pause / 1x / 2x / 3x.
- Fix the stale help copy for the same control mismatch.
- Replace the static active-charter systems panel (`ORIGIN: Home Berth`, `WAYPOINT:
  deep transit`, `DESTINATION: per charter`) with the active charter's actual origin,
  operation site, destination, objective subsystem, and next milestone.
- Synchronise `gdd.md` and `content_depth.md` with the current real-time loop, event
  count, contract count, Heritage behaviour, and interaction flow before publishing.
- Complete a fresh renown-0 charter by hand, then run `publish.ps1` from this project
  directory once the release changes are stable.

## Audio pass

- Add a restrained underway ambience and a small set of high-value cues: UI click,
  council decision alert, event resolution, phase transition, homecoming, and gameover.

## Ship schematic (underway view)

- Sharpen the barge vs ark silhouettes — both read as full-bulge hulls today.
- Give the deck captions real information; they are pure flavour.
- Thicken the compartment icon strokes so they survive capture-scale rendering.
- Add a legend entry explaining the compartment pictograms.

## Event content

- Prefer a small number of 2–4 event consequence chains over another large batch of
  isolated two-choice events. Target a few strong chains per objective family and
  additional legacy/faction chains for replay value.

## Meta-progression

- Decide whether Heritage is intentionally an automatic renown-tier head start or a
  player choice. If it remains automatic, update the GDD and store-facing copy; if the
  promised modifier selection is desired, implement the choice before calling it done.

## Verification

- Human playthrough of a renown-0 charter to feel the early economy — the first
  upgrade should read as a choice, not a formality.
