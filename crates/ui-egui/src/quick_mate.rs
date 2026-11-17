//! Quick mate: with no command running, a reference picked on each of two parts (a planar face, a
//! round face or edge, an edge, a point) offers the assembly constraints they fit in a small card
//! above the selection, as an assembly program's context toolbar does. A click adds the rule at
//! once (`assembly.constraint.create`, one undo step); Distance and Angle first take their value.
//! After a rule is added the card offers Flip, which turns the moving part over through
//! `assembly.constraint.edit`.
//!
//! The rules come from the same table as the Constrain dialog (`Constraint::fitting`); the card
//! shows the everyday ones (mate, concentric, parallel, perpendicular, tangent, distance, angle),
//! the dialog keeps the rest. The moving part is the one not held in place: a reference on the
//! design's own bodies (which never move) or on a grounded part is always the held side.

use serde_json::{Value, json};
use solidcraft_engine::Sel;
use solidcraft_engine::doc::joints::{Constraint, RefShape};

use crate::SolveApp;
use crate::dialogs_assembly::picked_snap;
use crate::ui_kit;

/// The rules the card offers, in the order shown.
const OFFERED: [Constraint; 7] = [
    Constraint::Mate,
    Constraint::Concentric,
    Constraint::Parallel,
    Constraint::Perpendicular,
    Constraint::Tangent,
    Constraint::Distance,
    Constraint::Angle,
];

/// The card's state, kept while the same two references stay selected.
#[derive(Clone, Debug, Default)]
pub struct QuickMate {
    /// The selection the state is for.
    key: Vec<Sel>,
    /// A Distance or Angle rule waiting for its value, and the value typed.
    asking: Option<Constraint>,
    value: String,
    /// The rule added from this selection: the constraint's id and its rule (Flip acts on it).
    added: Option<(u64, Constraint)>,
    /// Why the last click was refused.
    error: Option<String>,
}

/// The two references, as the rules see them.
struct Pair {
    shapes: [RefShape; 2],
    parts: [u64; 2],
    params: [Value; 2],
    /// Where the second reference is (the card opens beside it).
    at: solidcore_record::Vec3,
}

/// Nothing else is going on: no command, dialog, sketch, drag or menu, and a design is shown.
fn idle(app: &SolveApp) -> bool {
    app.dialog.is_none()
        && app.tool.is_none()
        && app.session.active_sketch.is_none()
        && !app.home.open
        && !crate::render_workspace::active(app)
        && !crate::occurrence_drag::active(app)
        && !crate::joint_drag::active(app)
        && !crate::occurrence_rotate::active(app)
        && app.viewport.context_menu.is_none()
        && !crate::toolbar::unfinished(app)
        && !crate::select_other::is_open(app)
        // A connection point or a slot picked, or a part placed on slots: the picks are theirs.
        && !crate::hardware_connection::any_mode(app)
}

/// Two references on two different parts, at least one of which is not the design itself.
fn pair(app: &SolveApp) -> Option<Pair> {
    let [a, b] = app.session.selection.as_slice() else { return None };
    let (a, b) = (picked_snap(app, a)?, picked_snap(app, b)?);
    let parts = [crate::dialogs_constrain::part_of(&a), crate::dialogs_constrain::part_of(&b)];
    if parts[0] == parts[1] {
        return None;
    }
    let at = b.at;
    Some(Pair { shapes: [crate::dialogs_constrain::shape(&a), crate::dialogs_constrain::shape(&b)], parts, params: [a.param, b.param], at })
}

/// The rules the card offers for `p`.
fn offered(p: &Pair) -> Vec<Constraint> {
    let fit = Constraint::fitting(p.shapes[0], p.shapes[1]);
    OFFERED.into_iter().filter(|c| fit.contains(c)).collect()
}

/// Whether a part may move: not the design itself and not grounded.
fn movable(app: &SolveApp, part: u64) -> bool {
    part != 0 && !app.session.doc.occurrences.iter().any(|o| o.id == part && o.grounded)
}

/// `assembly.constraint.create`'s parameters for `rule`: the references ordered so that the part
/// the engine moves (b, or a when the rule swaps them) is one that may move.
fn create_params(app: &SolveApp, rule: Constraint, p: &Pair, value: &str) -> Value {
    let order = |first: usize| {
        let (i, j) = (first, 1 - first);
        let swap = rule.fit(p.shapes[i], p.shapes[j]).is_some_and(|f| f.swap);
        let moving = if swap { p.parts[i] } else { p.parts[j] };
        (i, j, movable(app, moving))
    };
    // The second pick moves, as picked, unless only the first one can.
    let (i, j, _) = [order(0), order(1)].into_iter().find(|(_, _, ok)| *ok).unwrap_or_else(|| order(0));
    let mut params = json!({"type": rule.name(), "a": p.params[i], "b": p.params[j]});
    match rule {
        Constraint::Distance => params["offset"] = json!(value.trim()),
        Constraint::Angle => params["angle"] = json!(value.trim()),
        _ => {}
    }
    params
}

/// Add `rule` between the two references.
fn add(app: &mut SolveApp, rule: Constraint, p: &Pair) {
    let value = app.quick_mate.value.clone();
    let params = create_params(app, rule, p, &value);
    match app.run("assembly.constraint.create", params) {
        Ok(reply) => {
            app.quick_mate.added = reply.get("joint").and_then(Value::as_u64).map(|id| (id, rule));
            app.quick_mate.asking = None;
            app.quick_mate.error = None;
        }
        Err(e) => app.quick_mate.error = Some(e),
    }
}

/// The default value a Distance or Angle rule asks with.
fn default_value(rule: Constraint) -> &'static str {
    if rule == Constraint::Angle { "90 deg" } else { "0 mm" }
}

/// What a click on the card asks for.
enum Click {
    Rule(Constraint),
    Apply,
    Flip,
}

/// Each frame, in the 3D view: show the card when two references on two parts are selected.
pub fn show(app: &mut SolveApp, ui: &egui::Ui, free: egui::Rect) {
    crate::shell::register_panel(app, "quick_mate", None);
    if !idle(app) {
        return;
    }
    let Some(p) = pair(app) else {
        app.quick_mate = QuickMate::default();
        return;
    };
    if app.quick_mate.key != app.session.selection {
        app.quick_mate = QuickMate { key: app.session.selection.clone(), ..Default::default() };
    }
    let rules = offered(&p);
    if rules.is_empty() && app.quick_mate.added.is_none() {
        return;
    }
    let Some(viewport) = app.viewport.rect else { return };
    let proj = crate::viewport::projection(app, viewport);
    // Beside the second reference (where the pointer just was), never over the toolbars: the card
    // hangs above its anchor, so the anchor stays a card's height below the view's top.
    let Some(at) = proj.to_screen(p.at).filter(|at| at.is_finite() && free.contains(*at)) else { return };
    let t = solidcraft_theme::Tokens::get();
    let tall = if app.quick_mate.asking.is_some() || app.quick_mate.error.is_some() { 6.0 } else { 3.0 };
    let anchor = egui::pos2(
        crate::ui_kit::within(at.x, free.left(), free.right()),
        crate::ui_kit::within(at.y - t.space_12, free.top() + tall * t.control_height, free.bottom()),
    );
    let state = app.quick_mate.clone();
    let ctx = ui.ctx().clone();
    let card = ui_kit::Container::selection_card(&ctx, egui::Id::unique("quick_mate"), anchor, free, |ui| {
        let mut click = None;
        let mut value = state.value.clone();
        ui.vertical(|ui| {
            if let Some((_, rule)) = state.added {
                ui.horizontal(|ui| {
                    ui.label(format!("✓ {}", crate::dialogs_constrain::label(rule)));
                    let flip = ui_kit::button(ui, solidcraft_ui_text::tr("constrain.flip"), ui_kit::Button::Small);
                    crate::scenario::publish_handle("quick_mate:flip", flip.rect.center());
                    if flip.clicked() {
                        click = Some(Click::Flip);
                    }
                });
                if let Some(e) = &state.error {
                    ui_kit::status_line(ui, e, ui_kit::StatusTone::Error);
                }
                return;
            }
            ui.horizontal_wrapped(|ui| {
                for rule in &rules {
                    let options = if state.asking == Some(*rule) { ui_kit::Button::SmallSelected } else { ui_kit::Button::Small };
                    let r = ui_kit::button(ui, crate::dialogs_constrain::label(*rule), options);
                    crate::scenario::publish_handle(&format!("quick_mate:{}", rule.name()), r.rect.center());
                    if r.clicked() {
                        click = Some(Click::Rule(*rule));
                    }
                }
            });
            if let Some(rule) = state.asking {
                let label = solidcraft_ui_text::tr(if rule == Constraint::Angle { "constrain.angle" } else { "constrain.offset" });
                let entered = ui_kit::expr_row(ui, label, &mut value);
                let apply = ui_kit::button(ui, solidcraft_ui_text::tr("constrain.apply"), ui_kit::Button::Small);
                crate::scenario::publish_handle("quick_mate:apply", apply.rect.center());
                if entered || apply.clicked() {
                    click = Some(Click::Apply);
                }
            }
            if let Some(e) = &state.error {
                ui_kit::status_line(ui, e, ui_kit::StatusTone::Error);
            }
        });
        (click, value)
    });
    crate::scenario::publish_rect("quick_mate", card.response.rect);
    crate::shell::register_panel(app, "quick_mate", Some(card.response.rect));
    let (click, value) = card.inner;
    app.quick_mate.value = value;
    // A click changes the card (a rule added shows its Flip): draw the next frame at once, so the
    // card's new layout is there before the next click.
    if click.is_some() {
        ui.ctx().request_repaint();
    }
    match click {
        Some(Click::Rule(rule)) if matches!(rule, Constraint::Distance | Constraint::Angle) => {
            if app.quick_mate.asking != Some(rule) {
                app.quick_mate.value = default_value(rule).into();
            }
            app.quick_mate.asking = Some(rule);
        }
        Some(Click::Rule(rule)) => add(app, rule, &p),
        Some(Click::Apply) => {
            if let Some(rule) = app.quick_mate.asking {
                add(app, rule, &p);
            }
        }
        Some(Click::Flip) => {
            if let Some((id, _)) = app.quick_mate.added
                && let Err(e) = app.run("assembly.constraint.edit", json!({"constraint": id, "flip": true}))
            {
                app.quick_mate.error = Some(e);
            }
        }
        None => {}
    }
}

#[cfg(test)]
#[path = "quick_mate_tests.rs"]
mod tests;
