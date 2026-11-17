//! Placing library parts on placement slots by clicking them in the view. A click on a hardware
//! part's row in the Hardware Library arms placement for it (a part whose data card has no
//! connection point is not armed: the panel asks for one first). While it is armed, the slots of
//! the part's library group that are in use and hold nothing glow in the view; with the pointer
//! over one the part shows there as a ghost, where `hardware.place` with `preview` says it goes,
//! and a click places it (`hardware.place`, one undo step). The mode stays on, so the same part
//! goes on several slots (four corner drives); a slot that holds a part is a small grey dot with
//! a check. Esc, a right click in the view, or a click on the part's row again ends it.
//!
//! After a placement a small card beside it flips the part over (`joint.edit` `flip`), turns it
//! half a turn about the slot's normal (`joint.edit` `angle`, the joint's own turn after the
//! mate, so the slot's turn that its limits hold stays as it is) or takes it away
//! (`hardware.unplace`). Another placement, or the end of the mode, puts the card away.
//!
//! The ghost is the part's display mesh, made once per file in a scratch session (as a parts
//! dialog's picture is) and kept in the part's own coordinates, those of its STEP file, which the
//! preview's placement takes into the world; a part of more than [`MAX_TRIANGLES`] triangles
//! shows its box instead.

use std::collections::HashMap;
use std::rc::Rc;

use egui::{Pos2, Stroke, vec2};
use serde_json::{Value, json};
use solidcore_record::Vec3;
use solidcraft_engine::Session;
use solidcraft_engine::doc::{Mat, apply_point, mat_inverse};
use solidcraft_engine::geom::prelude::*;
use solidcraft_theme::Tokens;
use solidcraft_ui_text::tr;

use crate::SolveApp;
use crate::hardware_library::HardwareLibrary;
use crate::hardware_slot::Slot;
use crate::ui_kit;
use crate::viewport::Proj;

/// A free slot's dot and its glow (points), and how much larger both grow under the pointer.
const RADIUS: f32 = 6.0;
const GLOW: f32 = 13.0;
const HOVER_GROWTH: f32 = 1.35;
/// How near a free slot's dot the pointer must be (points) to be over it.
const REACH: f32 = 13.0;
/// A slot that holds a part: its dot.
const TAKEN_RADIUS: f32 = 4.5;
/// A ghost of more triangles than this is drawn as its box.
const MAX_TRIANGLES: usize = 60_000;

/// The part being placed.
pub(crate) struct Placing {
    /// The part's file, its name and its library group.
    pub(crate) path: String,
    pub(crate) name: String,
    pub(crate) group: String,
    /// The design it was armed in: its session and document.
    session: u64,
    document: String,
    /// Where the part goes on each slot (`hardware.place` with `preview`), by slot id, for the
    /// design as `previews_for` says (session, revision); `None` where it can't go.
    previews: HashMap<String, Option<Mat>>,
    previews_for: (u64, u64),
    /// The last placement, whose card is up.
    last: Option<Placed>,
}

/// A placement this mode made: its slot, the slot's name, the joint holding the part and where
/// the slot was on screen.
struct Placed {
    slot: String,
    label: String,
    joint: u64,
    at: Pos2,
}

/// A part's ghost: its display triangles in its own coordinates and the box around them (no
/// triangles: too many, the box is drawn instead).
pub(crate) struct Ghost {
    triangles: Vec<[Vec3; 3]>,
    low: Vec3,
    high: Vec3,
}

/// The ghosts made so far, by file; `None` for a file that would not come in.
pub(crate) type Ghosts = HashMap<String, Option<Rc<Ghost>>>;

/// Whether a part is being placed.
pub(crate) fn active(app: &SolveApp) -> bool {
    app.hardware_library.as_ref().is_some_and(|d| d.placing.is_some())
}

/// Whether the part at `path` is the one being placed.
pub(crate) fn armed_for(d: &HardwareLibrary, path: &str) -> bool {
    d.placing.as_ref().is_some_and(|p| p.path == path)
}

/// A slot the part being placed can go on: of its group, in use, where it can be placed, holding
/// nothing.
pub(crate) fn free(slot: &Slot, group: &str) -> bool {
    slot.group == group && slot.active && slot.frame.is_some() && slot.placed.is_none()
}

fn taken(slot: &Slot, group: &str) -> bool {
    slot.group == group && slot.frame.is_some() && slot.placed.is_some()
}

/// Arm placement for a library part, as `hardware.library.list` lists it. Refused (saying why)
/// for a part whose data card has no connection point, and while a sketch, an edit in place, a
/// connection point or a slot is open; an open dialog or tool is closed first.
pub(crate) fn arm(app: &mut SolveApp, d: &mut HardwareLibrary, part: &Value) -> Result<(), String> {
    if !part["card"]["connection"].is_object() {
        return Err(tr("hardware_place.no_connection").to_owned());
    }
    if crate::hardware_connection::active(app) {
        return Err(tr("hardware_connection.busy").to_owned());
    }
    if crate::hardware_slot::active(app) {
        return Err(tr("hardware_slot.busy").to_owned());
    }
    if app.session.active_sketch.is_some() || app.edit_in_place.active.is_some() {
        return Err(tr("hardware_connection.finish_first").to_owned());
    }
    if app.dialog.is_some() {
        crate::dialogs::cancel(app);
    }
    app.tool = None;
    let text = |key: &str| part[key].as_str().unwrap_or_default().to_owned();
    let name = part["card"]["part_no"].as_str().filter(|n| !n.trim().is_empty()).map_or_else(|| text("name"), str::to_owned);
    d.placing = Some(Placing {
        path: text("path"),
        name,
        group: text("group"),
        session: app.session.uid,
        document: app.session.document_id.clone(),
        previews: HashMap::new(),
        previews_for: (0, 0),
        last: None,
    });
    Ok(())
}

/// End placement (Esc, a right click). Returns whether a part was being placed.
pub(crate) fn cancel(app: &mut SolveApp) -> bool {
    app.hardware_library.as_mut().and_then(|d| d.placing.take()).is_some()
}

/// The free slots' dots where they are on screen now, those under a panel or a card left out.
fn targets(app: &SolveApp, d: &HardwareLibrary, proj: &Proj) -> Vec<(String, Pos2)> {
    let Some(placing) = &d.placing else { return Vec::new() };
    d.slots
        .slots
        .iter()
        .filter(|s| free(s, &placing.group))
        .filter_map(|s| proj.to_screen(s.frame?.origin).map(|at| (s.id.clone(), at)))
        .filter(|(_, at)| !crate::shell::covered(app, *at))
        .collect()
}

/// Whether a free slot's dot owns the pointer at `p`: a press there neither selects, nor draws a
/// box, nor turns the view.
pub(crate) fn owns_pointer(app: &SolveApp, proj: &Proj, p: Pos2) -> bool {
    app.hardware_library.as_ref().is_some_and(|d| targets(app, d, proj).iter().any(|(_, at)| at.distance(p) <= REACH))
}

/// The ghost of the part at `path`, made the first time it is asked for.
fn ghost(ghosts: &mut Ghosts, path: &str) -> Option<Rc<Ghost>> {
    ghosts.entry(path.to_owned()).or_insert_with(|| make_ghost(path).map(Rc::new)).clone()
}

/// The part brought into a scratch design, its display triangles taken back through its top
/// occurrence's placement into its own coordinates.
fn make_ghost(path: &str) -> Option<Ghost> {
    let mut s = Session::default();
    let r = s.execute("file.insert_step", &json!({ "path": path })).ok()?;
    let occurrence = r["occurrence"].as_u64()?;
    let to_part = mat_inverse(&solidcraft_engine::doc::joints::occurrence_world(&s.doc, occurrence))?;
    let world = crate::open_meshing::session_world(&s);
    let mut triangles = Vec::new();
    let (mut low, mut high) =
        (Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY), Vec3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY));
    let mut too_many = false;
    for body in &world.bodies {
        let mesh = body.mesh();
        for t in &mesh.triangles {
            let Some(tri) = mesh.tri(t) else { continue };
            let tri = tri.map(|v| apply_point(&to_part, v));
            for v in tri {
                low = Vec3::new(low.x.min(v.x), low.y.min(v.y), low.z.min(v.z));
                high = Vec3::new(high.x.max(v.x), high.y.max(v.y), high.z.max(v.z));
            }
            if triangles.len() < MAX_TRIANGLES {
                triangles.push(tri);
            } else {
                too_many = true;
            }
        }
    }
    if !(low.is_finite() && high.is_finite()) {
        return None;
    }
    if too_many {
        triangles = Vec::new();
    }
    Some(Ghost { triangles, low, high })
}

/// A row-major 4x4 matrix as the commands give it, as a placement.
fn placement_of(v: &Value) -> Option<Mat> {
    let rows: Vec<Vec<f64>> = serde_json::from_value(v.clone()).ok()?;
    let at = |r: usize, c: usize| rows.get(r).and_then(|row| row.get(c)).copied().filter(|x| x.is_finite());
    let mut m = [[0.0; 4]; 4];
    for (c, column) in m.iter_mut().enumerate() {
        for (r, cell) in column.iter_mut().enumerate() {
            *cell = at(r, c)?;
        }
    }
    Some(m)
}

/// Where the part goes on `slot`, worked out once for the design as it is.
fn preview(app: &mut SolveApp, placing: &mut Placing, slot: &str) -> Option<Mat> {
    let key = (app.session.uid, app.session.revision);
    if placing.previews_for != key {
        placing.previews.clear();
        placing.previews_for = key;
    }
    if let Some(m) = placing.previews.get(slot) {
        return *m;
    }
    let m = app
        .session
        .execute("hardware.place", &json!({"slot": slot, "path": placing.path, "preview": true}))
        .ok()
        .and_then(|r| placement_of(&r["transform"]));
    placing.previews.insert(slot.to_owned(), m);
    m
}

/// The part's ghost at placement `m`: its triangles shaded by how squarely they face the view,
/// in the accent at a low opacity, or its box.
fn paint_ghost(painter: &egui::Painter, proj: &Proj, ghost: &Ghost, m: &Mat) {
    let t = Tokens::get();
    if ghost.triangles.is_empty() {
        let (l, h) = (ghost.low, ghost.high);
        let corner = |i: usize| {
            let p = Vec3::new(if i & 1 == 0 { l.x } else { h.x }, if i & 2 == 0 { l.y } else { h.y }, if i & 4 == 0 { l.z } else { h.z });
            proj.to_screen(apply_point(m, p))
        };
        for (a, b) in [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)] {
            if let (Some(a), Some(b)) = (corner(a), corner(b)) {
                painter.line_segment([a, b], Stroke::new(1.5, t.accent));
            }
        }
        return;
    }
    let view = proj.ray(painter.clip_rect().center()).1;
    let mut mesh = egui::Mesh::default();
    for tri in &ghost.triangles {
        let w = tri.map(|v| apply_point(m, v));
        let Some(n) = (w[1] - w[0]).cross(w[2] - w[0]).normalized() else { continue };
        let (Some(a), Some(b), Some(c)) = (proj.to_screen(w[0]), proj.to_screen(w[1]), proj.to_screen(w[2])) else { continue };
        let Ok(i) = u32::try_from(mesh.vertices.len()) else { break };
        let facing = n.dot(view).abs() as f32;
        let colour = t.accent.gamma_multiply(0.10 + 0.30 * facing);
        for p in [a, b, c] {
            mesh.colored_vertex(p, colour);
        }
        mesh.add_triangle(i, i + 1, i + 2);
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// A check mark in a slot's dot that holds a part.
fn paint_check(painter: &egui::Painter, at: Pos2, colour: egui::Color32) {
    let s = TAKEN_RADIUS * 0.6;
    painter.line_segment([at + vec2(-s, 0.0), at + vec2(-s * 0.3, s * 0.7)], Stroke::new(1.5, colour));
    painter.line_segment([at + vec2(-s * 0.3, s * 0.7), at + vec2(s, -s * 0.6)], Stroke::new(1.5, colour));
}

/// While a part is placed: the slots it can go on, glowing (larger and brighter under the
/// pointer, with the part's ghost there), those that hold a part, and a click that places it.
pub(crate) fn show(app: &mut SolveApp, ui: &mut egui::Ui, painter: &egui::Painter, proj: &Proj) {
    let Some(mut d) = app.hardware_library.take() else { return };
    if d.placing.is_some() {
        placing_view(app, &mut d, ui, painter, proj);
    }
    app.hardware_library = Some(d);
}

fn placing_view(app: &mut SolveApp, d: &mut HardwareLibrary, ui: &mut egui::Ui, painter: &egui::Painter, proj: &Proj) {
    let t = Tokens::get();
    let targets = targets(app, d, proj);
    let Some(placing) = d.placing.as_mut() else { return };
    for slot in d.slots.slots.iter().filter(|s| taken(s, &placing.group)) {
        if let Some(at) = slot.frame.and_then(|f| proj.to_screen(f.origin)) {
            painter.circle(at, TAKEN_RADIUS, t.text_dim, Stroke::new(1.0, t.panel));
            paint_check(painter, at, t.panel);
        }
    }
    let mut over = None;
    let mut clicked = None;
    for (id, at) in &targets {
        let area = egui::Rect::from_center_size(*at, vec2(2.0 * REACH, 2.0 * REACH));
        let r = ui.interact(area, egui::Id::unique(("sc_hardware_place", id.as_str())), egui::Sense::click_and_drag());
        let hovered = r.hovered() || r.is_pointer_button_down_on();
        if hovered {
            over = Some((id.clone(), *at));
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if r.clicked() {
            clicked = Some((id.clone(), *at));
        }
        let grow = if hovered { HOVER_GROWTH } else { 1.0 };
        painter.circle_filled(*at, GLOW * grow, t.accent.gamma_multiply(if hovered { 0.38 } else { 0.2 }));
        painter.circle_filled(*at, GLOW * grow * 0.6, t.accent.gamma_multiply(if hovered { 0.5 } else { 0.3 }));
        painter.circle(*at, RADIUS * grow, t.accent, Stroke::new(1.5, t.panel));
    }
    if let Some((id, at)) = &over {
        if let Some(ghost) = ghost(&mut d.ghosts, &placing.path)
            && let Some(m) = preview(app, placing, id)
        {
            paint_ghost(painter, proj, &ghost, &m);
        }
        if let Some(slot) = d.slots.slots.iter().find(|s| s.id == *id) {
            if let Some(frame) = &slot.frame {
                crate::hardware_connection::paint_triad(painter, proj, frame, crate::hardware_connection::triad_length(app));
            }
            crate::hardware_slot::name_tag(painter, *at + vec2(GLOW - 8.0, 0.0), &slot.label);
        }
    }
    if let Some((id, at)) = clicked {
        let label = d.slots.slots.iter().find(|s| s.id == id).map_or_else(|| id.clone(), |s| s.label.clone());
        match app.run("hardware.place", json!({"slot": id, "path": placing.path})) {
            Ok(r) => {
                placing.last = r["joint"].as_u64().map(|joint| Placed { slot: id.clone(), label: label.clone(), joint, at });
                let text = solidcraft_ui_text::format("hardware_place.placed", &[("part", placing.name.as_str()), ("slot", label.as_str())]);
                app.set_status(text, false);
            }
            Err(e) => {
                placing.last = None;
                app.set_status(e, true);
            }
        }
    }
}

/// Every frame: end placement when the design or what is open changed under it, and show the last
/// placement's card (Flip over, Turn 180°, Remove).
pub(crate) fn tick(app: &mut SolveApp, ctx: &egui::Context) {
    let Some(placing) = app.hardware_library.as_ref().and_then(|d| d.placing.as_ref()) else { return };
    let elsewhere = app.session.uid != placing.session || app.session.document_id != placing.document;
    let taken_over = app.dialog.is_some()
        || app.tool.is_some()
        || app.session.active_sketch.is_some()
        || crate::hardware_connection::active(app)
        || crate::hardware_slot::active(app);
    if elsewhere || taken_over {
        cancel(app);
        return;
    }
    let Some(last) = placing.last.as_ref() else { return };
    // Undone or deleted meanwhile: no card.
    let Some(joint) = app.session.doc.assembly.joints.iter().find(|j| j.id == last.joint) else {
        forget_last(app);
        return;
    };
    let (flip, turned) = (joint.flip, joint.angle.abs() > 1e-6);
    let (slot, id, label, at) = (last.slot.clone(), last.joint, last.label.clone(), last.at);
    let title = format!("{label}: {}", placing.name);
    let window = ui_kit::Window::new(egui::Id::unique(("sc_hardware_placed", slot.as_str(), id)), "joint", &title)
        .placed(egui::Align2::LEFT_BOTTOM, at + vec2(GLOW, -GLOW));
    let mut asked = None;
    let closed = window.show_body(ctx, |ui| {
        ui.horizontal_wrapped(|ui| {
            for (key, action) in [("hardware_place.flip", 0), ("hardware_place.turn", 1), ("hardware_place.remove", 2)] {
                if ui_kit::button(ui, tr(key), ui_kit::Button::Normal).clicked() {
                    asked = Some(action);
                }
            }
        });
    });
    let done = match asked {
        Some(0) => app.run("joint.edit", json!({"joint": id, "flip": !flip})).map(|_| false),
        Some(1) => app.run("joint.edit", json!({"joint": id, "angle": if turned { 0 } else { 180 }})).map(|_| false),
        Some(2) => app.run("hardware.unplace", json!({ "slot": slot })).map(|_| true),
        _ => Ok(closed),
    };
    match done {
        Ok(true) => forget_last(app),
        Ok(false) => {}
        Err(e) => app.set_status(e, true),
    }
}

fn forget_last(app: &mut SolveApp) {
    if let Some(placing) = app.hardware_library.as_mut().and_then(|d| d.placing.as_mut()) {
        placing.last = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reply_matrix_reads_back_as_a_placement() {
        let m = placement_of(&json!([[1, 0, 0, 10], [0, 0, -1, 20], [0, 1, 0, 30], [0, 0, 0, 1]])).unwrap();
        assert_eq!(m[3], [10.0, 20.0, 30.0, 1.0], "the translation is the last column");
        assert_eq!(m[1], [0.0, 0.0, 1.0, 0.0], "y goes to z");
        assert!(placement_of(&json!([[1, 0], [0, 1]])).is_none());
        assert!(placement_of(&json!("no")).is_none());
    }

    #[test]
    fn only_free_slots_of_the_parts_group_take_it() {
        let slot = |group: &str, active: bool, placed: Option<(u64, u64)>| Slot {
            id: "s".into(),
            label: "S".into(),
            group: group.into(),
            along: "0".into(),
            across: "middle".into(),
            angle: 0,
            flip: false,
            when: None,
            active,
            error: None,
            frame: Some(crate::hardware_connection::Frame { origin: Vec3::ZERO, z: Vec3::X, x: Vec3::Z }),
            placed,
        };
        assert!(free(&slot("Köşe aktarma", true, None), "Köşe aktarma"));
        assert!(!free(&slot("Menteşe grubu", true, None), "Köşe aktarma"), "another group");
        assert!(!free(&slot("Köşe aktarma", false, None), "Köşe aktarma"), "not in use");
        assert!(!free(&slot("Köşe aktarma", true, Some((5, 6))), "Köşe aktarma"), "taken");
        assert!(taken(&slot("Köşe aktarma", true, Some((5, 6))), "Köşe aktarma"));
    }
}
