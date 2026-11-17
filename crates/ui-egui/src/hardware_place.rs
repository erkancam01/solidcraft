//! Placing library parts on placement slots by clicking them in the view. A click on a hardware
//! part's row in the Hardware Library arms placement for it (a part whose data card has no
//! connection point is not armed: the panel asks for one first). While it is armed, the slots of
//! the part's library group that hold nothing glow in the view, those whose condition does not
//! hold now dimmer (a part placed there comes in hidden, for the condition's rules to show); with
//! the pointer over one the part shows there as a ghost, where `hardware.place` with `preview`
//! says it goes, and a click places it (`hardware.place`, one undo step). The mode stays on, so
//! the same part goes on several slots (four corner drives); a slot that holds a part is a small
//! grey dot with a check. Esc, a right click in the view, or a click on the part's row again ends
//! it.
//!
//! After a placement a small card beside the part, following it as the view turns (and away while
//! the part is off screen), flips it over (`joint.edit` `flip`), turns it half a turn about the
//! slot's normal (`joint.edit` `angle`, the joint's own turn after the mate, so the slot's turn
//! that its limits hold stays as it is) or takes it away (`hardware.unplace`). Another placement,
//! or the end of the mode, puts the card away.
//!
//! The ghost is the part's display mesh, made once per file in a scratch session (as a parts
//! dialog's picture is) and kept in the part's own coordinates, those of its STEP file, which the
//! preview's placement takes into the world: shaded in the accent, its creases and outline drawn
//! over it, stronger the smaller it is on screen. A part of more than [`MAX_TRIANGLES`] triangles
//! shows its box instead.

use std::collections::HashMap;
use std::rc::Rc;

use egui::{Pos2, Stroke, pos2, vec2};
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
/// How much a free slot out of its condition glows of what one in use does.
const INACTIVE: f32 = 0.45;
/// A ghost of more triangles than this is drawn as its box.
const MAX_TRIANGLES: usize = 60_000;
/// A ghost this many points across on screen or fewer is far away (its edges at their strongest);
/// one of [`NEAR`] or more is up close (its edges subtle).
const FAR: f32 = 40.0;
const NEAR: f32 = 320.0;
/// Two triangles meet at a crease, an edge of the ghost drawn always, when their normals are
/// further apart than this (its cosine: 30°).
const CREASE: f64 = 0.866;
/// The ghost's box: its corners (bit 0 x, bit 1 y, bit 2 z at the high end) the edges join.
const BOX_EDGES: [(usize, usize); 12] = [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)];

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

/// A placement this mode made: its slot, the slot's name and the joint holding the part.
struct Placed {
    slot: String,
    label: String,
    joint: u64,
}

/// A part's ghost: its display triangles in its own coordinates, their edges and the box around
/// them (no triangles: too many, the box is drawn instead).
pub(crate) struct Ghost {
    triangles: Vec<[Vec3; 3]>,
    edges: Vec<GhostEdge>,
    low: Vec3,
    high: Vec3,
}

/// An edge of a ghost's triangles: its ends, the triangles either side (`right` `None` on an open
/// boundary) and whether they meet at a crease. Creases and boundaries are always drawn; any
/// other edge where one side faces the view and the other doesn't, the outline.
struct GhostEdge {
    a: Vec3,
    b: Vec3,
    left: usize,
    right: Option<usize>,
    crease: bool,
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

/// A slot the part being placed can go on: of its group, where it can be placed, holding nothing.
/// One whose condition does not hold now is free too (the part comes in hidden); one whose
/// condition can't be worked out is not.
pub(crate) fn free(slot: &Slot, group: &str) -> bool {
    slot.group == group && slot.error.is_none() && slot.frame.is_some() && slot.placed.is_none()
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

/// The free slots' dots where they are on screen now, and whether each slot's condition holds
/// now; those under a panel or a card left out.
fn targets(app: &SolveApp, d: &HardwareLibrary, proj: &Proj) -> Vec<(String, Pos2, bool)> {
    let Some(placing) = &d.placing else { return Vec::new() };
    d.slots
        .slots
        .iter()
        .filter(|s| free(s, &placing.group))
        .filter_map(|s| proj.to_screen(s.frame?.origin).map(|at| (s.id.clone(), at, s.active)))
        .filter(|(_, at, _)| !crate::shell::covered(app, *at))
        .collect()
}

/// Whether a free slot's dot owns the pointer at `p`: a press there neither selects, nor draws a
/// box, nor turns the view.
pub(crate) fn owns_pointer(app: &SolveApp, proj: &Proj, p: Pos2) -> bool {
    app.hardware_library.as_ref().is_some_and(|d| targets(app, d, proj).iter().any(|(_, at, _)| at.distance(p) <= REACH))
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
    let edges = ghost_edges(&triangles);
    Some(Ghost { triangles, edges, low, high })
}

/// Each edge of `triangles` once (its ends to a tenth of a micrometre), with the triangles either
/// side and whether they meet at a crease (an open boundary, or a triangle with no normal, counts
/// as one).
fn ghost_edges(triangles: &[[Vec3; 3]]) -> Vec<GhostEdge> {
    let key = |v: Vec3| [v.x, v.y, v.z].map(|c| (c * 1e4).round() as i64);
    let mut seen: HashMap<([i64; 3], [i64; 3]), usize> = HashMap::new();
    let mut edges: Vec<GhostEdge> = Vec::new();
    for (i, tri) in triangles.iter().enumerate() {
        for (a, b) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])] {
            let (ka, kb) = (key(a), key(b));
            let k = if ka <= kb { (ka, kb) } else { (kb, ka) };
            match seen.get(&k).copied() {
                Some(e) => {
                    if let Some(edge) = edges.get_mut(e) {
                        edge.right = edge.right.or(Some(i));
                    }
                }
                None => {
                    seen.insert(k, edges.len());
                    edges.push(GhostEdge { a, b, left: i, right: None, crease: true });
                }
            }
        }
    }
    let normal = |i: usize| triangles.get(i).and_then(|t| (t[1] - t[0]).cross(t[2] - t[0]).normalized());
    for e in &mut edges {
        e.crease = match (normal(e.left), e.right.and_then(normal)) {
            (Some(l), Some(r)) => l.dot(r) < CREASE,
            _ => true,
        };
    }
    edges
}

/// How far away a ghost `extent` points across on screen looks: 0 up close, 1 far away.
fn farness(extent: f32) -> f32 {
    if extent.is_finite() { 1.0 - ((extent - FAR) / (NEAR - FAR)).clamp(0.0, 1.0) } else { 0.0 }
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
/// in the accent, and its creases and outline over them (or its box), all the stronger the
/// smaller it is on screen, so it reads from far away and stays light up close.
fn paint_ghost(painter: &egui::Painter, proj: &Proj, ghost: &Ghost, m: &Mat) {
    let t = Tokens::get();
    let (l, h) = (ghost.low, ghost.high);
    let corners: [Option<Pos2>; 8] = std::array::from_fn(|i| {
        let p = Vec3::new(if i & 1 == 0 { l.x } else { h.x }, if i & 2 == 0 { l.y } else { h.y }, if i & 4 == 0 { l.z } else { h.z });
        proj.to_screen(apply_point(m, p))
    });
    let on_screen: Vec<Pos2> = corners.iter().flatten().copied().collect();
    let far = farness(egui::Rect::from_points(&on_screen).size().max_elem());
    let edge = Stroke::new(1.0 + 0.5 * far, t.accent.gamma_multiply(0.45 + 0.55 * far));
    if ghost.triangles.is_empty() {
        for (a, b) in BOX_EDGES {
            if let (Some(Some(a)), Some(Some(b))) = (corners.get(a), corners.get(b)) {
                painter.line_segment([*a, *b], edge);
            }
        }
        return;
    }
    let view = proj.ray(painter.clip_rect().center()).1;
    let mut mesh = egui::Mesh::default();
    // Which way each triangle faces the view: 1 towards it, -1 away, 0 not drawn.
    let mut sides = vec![0_i8; ghost.triangles.len()];
    for (k, tri) in ghost.triangles.iter().enumerate() {
        let w = tri.map(|v| apply_point(m, v));
        let Some(n) = (w[1] - w[0]).cross(w[2] - w[0]).normalized() else { continue };
        let (Some(a), Some(b), Some(c)) = (proj.to_screen(w[0]), proj.to_screen(w[1]), proj.to_screen(w[2])) else { continue };
        let Ok(i) = u32::try_from(mesh.vertices.len()) else { break };
        let towards = n.dot(view);
        if let Some(side) = sides.get_mut(k) {
            *side = if towards > 0.0 { 1 } else { -1 };
        }
        let facing = towards.abs() as f32;
        let colour = t.accent.gamma_multiply((0.16 + 0.34 * facing) * (1.0 + 0.5 * far));
        for p in [a, b, c] {
            mesh.colored_vertex(p, colour);
        }
        mesh.add_triangle(i, i + 1, i + 2);
    }
    painter.add(egui::Shape::mesh(mesh));
    let side = |i: usize| sides.get(i).copied().unwrap_or(0);
    for e in &ghost.edges {
        let outline = match e.right.map(side) {
            Some(r) => side(e.left) * r < 0,
            None => false,
        };
        if !(e.crease || outline) {
            continue;
        }
        if let (Some(a), Some(b)) = (proj.to_screen(apply_point(m, e.a)), proj.to_screen(apply_point(m, e.b))) {
            painter.line_segment([a, b], edge);
        }
    }
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
    for (id, at, active) in &targets {
        let area = egui::Rect::from_center_size(*at, vec2(2.0 * REACH, 2.0 * REACH));
        let r = ui.interact(area, egui::Id::unique(("sc_hardware_place", id.as_str())), egui::Sense::click_and_drag());
        let hovered = r.hovered() || r.is_pointer_button_down_on();
        if hovered {
            over = Some((id.clone(), *at));
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if r.clicked() {
            clicked = Some(id.clone());
        }
        let grow = if hovered { HOVER_GROWTH } else { 1.0 };
        // Out of its condition now: dimmer, and a part placed there comes in hidden.
        let dim = if *active { 1.0 } else { INACTIVE };
        painter.circle_filled(*at, GLOW * grow, t.accent.gamma_multiply(dim * if hovered { 0.38 } else { 0.2 }));
        painter.circle_filled(*at, GLOW * grow * 0.6, t.accent.gamma_multiply(dim * if hovered { 0.5 } else { 0.3 }));
        painter.circle(*at, RADIUS * grow, t.accent.gamma_multiply(if *active { 1.0 } else { 0.6 }), Stroke::new(1.5, t.panel));
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
            let tag = if slot.active { slot.label.clone() } else { format!("{} — {}", slot.label, tr("hardware_slot.inactive")) };
            crate::hardware_slot::name_tag(painter, *at + vec2(GLOW - 8.0, 0.0), &tag);
        }
    }
    if let Some(id) = clicked {
        let label = d.slots.slots.iter().find(|s| s.id == id).map_or_else(|| id.clone(), |s| s.label.clone());
        match app.run("hardware.place", json!({"slot": id, "path": placing.path})) {
            Ok(r) => {
                placing.last = r["joint"].as_u64().map(|joint| Placed { slot: id.clone(), label: label.clone(), joint });
                let text = if r["active"].as_bool() == Some(false) {
                    tr("hardware_place.hidden").to_owned()
                } else {
                    solidcraft_ui_text::format("hardware_place.placed", &[("part", placing.name.as_str()), ("slot", label.as_str())])
                };
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
    let (slot, id, label) = (last.slot.clone(), last.joint, last.label.clone());
    let title = format!("{label}: {}", placing.name);
    let card = egui::Id::unique(("sc_hardware_placed", slot.as_str(), id));
    // Beside the part where the view shows it now; away while that is off screen.
    let Some(corner) = card_at(app, ctx, &slot, card) else { return };
    let window = ui_kit::Window::new(card, "joint", &title).anchor(egui::Align2::LEFT_TOP, corner - ctx.content_rect().min);
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

/// Where the last placement's card goes now (its left-top corner): beside the part's connection
/// point, its slot's origin as the view shows it, inside the view's free area; `None` while that
/// point is off screen.
fn card_at(app: &SolveApp, ctx: &egui::Context, slot: &str, card: egui::Id) -> Option<Pos2> {
    let rect = app.viewport.rect?;
    let origin = app.hardware_library.as_ref()?.slots.slots.iter().find(|s| s.id == slot)?.frame?.origin;
    let at = crate::viewport::projection(app, rect).to_screen(origin)?;
    if !crate::shell::content_rect(app, rect).contains(at) {
        return None;
    }
    let t = Tokens::get();
    let last = ctx.memory(|m| m.area_rect(card));
    let size = last.map_or(vec2(t.form_card_width * 1.5, t.form_header_height + 2.0 * t.space_24), |r| r.size());
    let free = crate::hardware_connection::free_area(app, rect, last).shrink(t.space_8);
    Some(card_spot(at, size, free))
}

/// A card's left-top corner for a part at `at` on screen: above it and to its right, on the other
/// side where that leaves `free`, and inside `free` (its left-top corner when it does not fit).
fn card_spot(at: Pos2, size: egui::Vec2, free: egui::Rect) -> Pos2 {
    let right = at.x + GLOW;
    let x = if right + size.x <= free.right() { right } else { at.x - GLOW - size.x };
    let above = at.y - GLOW - size.y;
    let y = if above >= free.top() { above } else { at.y + GLOW };
    pos2(ui_kit::within(x, free.left(), free.right() - size.x), ui_kit::within(y, free.top(), free.bottom() - size.y))
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
        assert!(free(&slot("Köşe aktarma", false, None), "Köşe aktarma"), "out of its condition: the part comes in hidden");
        let mut broken = slot("Köşe aktarma", false, None);
        broken.error = Some("when: no parameter `NOPE`".into());
        assert!(!free(&broken, "Köşe aktarma"), "a condition that can't be worked out");
        assert!(!free(&slot("Köşe aktarma", true, Some((5, 6))), "Köşe aktarma"), "taken");
        assert!(taken(&slot("Köşe aktarma", true, Some((5, 6))), "Köşe aktarma"));
    }

    #[test]
    fn the_card_sits_beside_the_part_inside_the_free_area() {
        let free = egui::Rect::from_min_max(pos2(0.0, 100.0), pos2(1000.0, 800.0));
        let size = vec2(200.0, 80.0);
        assert_eq!(card_spot(pos2(500.0, 400.0), size, free), pos2(500.0 + GLOW, 400.0 - GLOW - 80.0), "above and to the right");
        assert_eq!(card_spot(pos2(900.0, 400.0), size, free), pos2(900.0 - GLOW - 200.0, 400.0 - GLOW - 80.0), "to the left at the right edge");
        assert_eq!(card_spot(pos2(500.0, 120.0), size, free), pos2(500.0 + GLOW, 120.0 + GLOW), "below at the top");
        let small = egui::Rect::from_min_size(pos2(10.0, 20.0), vec2(50.0, 30.0));
        assert_eq!(card_spot(pos2(30.0, 30.0), size, small), pos2(10.0, 20.0), "its corner where it does not fit");
        let spot = card_spot(pos2(f32::NAN, 30.0), size, free);
        assert!(spot.y.is_finite(), "{spot:?}");
    }

    #[test]
    fn a_ghosts_edges_are_shared_and_creases_found() {
        // Two faces of a box meeting at an edge (a crease), and two triangles of one face (no
        // crease between them).
        let (o, x, y, z) = (Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z);
        let triangles = [[o, x, x + y], [o, x + y, y], [x, o, z]];
        let edges = ghost_edges(&triangles);
        assert_eq!(edges.len(), 7, "nine sides, two of them shared");
        let between = |a: Vec3, b: Vec3| edges.iter().find(|e| (e.a == a && e.b == b) || (e.a == b && e.b == a)).unwrap();
        let diagonal = between(o, x + y);
        assert!(diagonal.right.is_some() && !diagonal.crease, "one face's two triangles");
        let fold = between(o, x);
        assert!(fold.right.is_some() && fold.crease, "a right angle between the faces");
        assert!(between(x, x + y).right.is_none() && between(x, x + y).crease, "an open boundary");
        assert!(farness(10.0) == 1.0 && farness(1000.0) == 0.0 && farness(f32::NAN) == 0.0);
    }
}
