//! Full-screen "voyage terminated" takeover shown when the dynasty goes extinct
//! (GDD §7). A CRT halt screen: a summary readout of the run and a single
//! retire-voyage exit. Pure view — clicking pushes [`UiAction::RetireVoyage`].

use crate::ui::{
    stat_line, term, term_button, term_panel, GameplayCtx, UiAction, LOGICAL_HEIGHT, LOGICAL_WIDTH,
};
use macroquad::prelude::*;
use macroquad_toolkit::prelude::*;
use macroquad_toolkit::ui::{draw_ui_text_ex, RectExt};

pub fn draw(ctx: &GameplayCtx<'_>, mouse: Vec2, actions: &mut Vec<UiAction>) {
    let sim = ctx.sim;
    draw_rectangle(0.0, 0.0, LOGICAL_WIDTH, LOGICAL_HEIGHT, term::bg());

    let legacy = ctx
        .data
        .legacies
        .get(&sim.legacy.legacy_id)
        .map(|l| l.name.clone())
        .unwrap_or_default();

    // Halted-terminal banner.
    draw_text_glow(
        "VOYAGE TERMINATED",
        LOGICAL_WIDTH / 2.0 - 232.0,
        140.0,
        TextStyle::new(46.0, term::alert()),
        0.14,
        3.0,
    );
    draw_ui_text_ex(
        &format!("// {} — no heir remains //", legacy.to_uppercase()),
        LOGICAL_WIDTH / 2.0 - 150.0,
        175.0,
        TextStyle::new(16.0, term::dim()).params(),
    );

    let panel = Rect::new(LOGICAL_WIDTH / 2.0 - 300.0, 220.0, 600.0, 384.0);
    term_panel(panel, Some("FINAL LOG // DYNASTY REGISTRY SEALED"));
    let content = panel.inset(28.0);

    let contracts = ctx
        .chronicle
        .entries
        .iter()
        .filter(|e| e.legacy_id == sim.legacy.legacy_id)
        .count();
    let leader = sim
        .dynasty
        .leader()
        .map(|l| l.name.clone())
        .unwrap_or_else(|| "an empty chair".to_owned());

    let rows: [(&str, String); 6] = [
        ("YEARS ELAPSED", sim.year.to_string()),
        ("GENERATIONS", sim.dynasty.generation.to_string()),
        ("FINAL POPULATION", sim.population.count.to_string()),
        ("TRADITION EARNED", sim.legacy.tradition_points.to_string()),
        ("CONTRACTS LOGGED", contracts.to_string()),
        ("LAST COMMANDER", leader),
    ];
    let mut y = content.y + 30.0;
    for (label, value) in rows {
        stat_line(content.x, y, label, &value, term::accent());
        y += 30.0;
    }

    y += 14.0;
    draw_text_block(
        "The ship sails on, unmanned by any bloodline. Its Chronicle endures.",
        content.x,
        y,
        content.w,
        44.0,
        14.0,
        4.0,
        term::dim(),
    );

    // Blinking retire prompt.
    let caret = if blink(get_time() as f32, 2.5) {
        ">"
    } else {
        " "
    };
    let btn = Rect::new(content.x, content.bottom() - 46.0, content.w, 42.0);
    if term_button(btn, &format!("{caret} RETIRE VOYAGE"), true, mouse) {
        actions.push(UiAction::RetireVoyage);
    }
}
