//! A hardware part's connection point, defined once in the app: the Hardware Library's
//! Connection point… brings the part into the design for a moment (after a checkpoint), frames
//! it, and a small card asks for two picks on it: the seat face (the flat face that lies on the
//! sash's hardware groove) and a reference edge in that face. Save writes the frame to the part's
//! data card (`hardware.card.set`) in the part's own coordinates, those of its STEP file, then
//! the design goes back to the checkpoint: the part is gone again and no undo step is left.
//! Cancel, Esc and closing the card go back to it too.
//!
//! The frame: `z` is the seat face's normal out of the material, the origin the reference edge's
//! end nearest the click on it (or, with From the middle of the edge, its middle), `x` along the
//! edge away from that end and `y = z × x`. With Centre across the face (on by default) the origin
//! then moves along `y` to the middle of the seat face's width (the least and the most of its
//! exact boundary along `y`), its place along `x` kept: a part placed on a slot whose `across` is
//! `middle` then sits in the middle of the groove. A placement on a slot is later
//! `slot frame × inverse(connection frame)`.
//!
//! The part's own coordinates are those of the import's top occurrence (the component the file
//! came in as): a STEP assembly's bodies sit in sub-occurrences placed inside it, so a pick on
//! any of them is taken back through the top occurrence's placement in the world.
//!
//! The picks are the view's own selection (no dialog is open): a click on a face is the seat
//! face, one on an edge the reference edge. While the card is up a click picks faces and edges
//! (not bodies, components or corners); the pick filter is put back afterwards.

use egui::{Stroke, vec2};
use serde_json::json;
use solidcore_record::Vec3;
use solidcraft_engine::Sel;
use solidcraft_engine::Session;
use solidcraft_engine::doc::{Mat, apply_point, apply_vector, mat_inverse};
use solidcraft_engine::geom::prelude::*;
use solidcraft_theme::Tokens;
use solidcraft_ui_text::tr;

use crate::SolveApp;
use crate::ui_kit;
use crate::viewport::Proj;

/// An edge shorter than this (mm) has no direction.
const MIN_EDGE: f64 = 1e-6;
/// How far off the seat face's plane an end of the reference edge may be (mm), plus
/// [`IN_FACE_SHARE`] of the edge's length.
const IN_FACE: f64 = 0.01;
const IN_FACE_SHARE: f64 = 1e-5;
/// The most the reference edge may lean out of the seat face (the sine of the angle).
const MAX_LEAN: f64 = 1e-3;
/// How far a point of a straight edge may be from the line through its ends (mm), plus
/// [`STRAIGHT_SHARE`] of the edge's length.
const STRAIGHT: f64 = 1e-4;
const STRAIGHT_SHARE: f64 = 1e-6;
/// The pick filter's kinds a click must reach while the card is up, and the one it must not.
const PICKED: [&str; 2] = ["faces", "edges"];
const NOT_PICKED: &str = "vertices";

/// A connection frame: the origin, the unit `z` and the unit `x` square to it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Frame {
    pub origin: Vec3,
    pub z: Vec3,
    pub x: Vec3,
}

/// The seat face as picked: a point on its plane, its normal out of the material and points of its
/// exact boundary, in the part's coordinates.
#[derive(Clone, Debug)]
struct Seat {
    sel: Sel,
    body: String,
    point: Vec3,
    normal: Vec3,
    boundary: Vec<Vec3>,
}

/// The reference edge as picked: its end nearest the click and its other end, in the part's
/// coordinates.
#[derive(Clone, Debug)]
struct Reference {
    sel: Sel,
    body: String,
    near: Vec3,
    far: Vec3,
}

/// The card's state while a connection point is being picked.
pub struct Pick {
    /// The part's file, and the name it came in as.
    path: String,
    name: String,
    /// The checkpoint taken before the part came in.
    checkpoint: u64,
    /// The design it came into (its session and document) and the import's top occurrence.
    session: u64,
    document: String,
    occurrence: u64,
    face: Option<Seat>,
    edge: Option<Reference>,
    /// The origin at the edge's middle instead of its nearer end.
    from_middle: bool,
    /// The origin moved across the seat face to the middle of its width.
    centre_across: bool,
    /// Why the last pick was not taken, or Save was refused.
    refused: Option<String>,
    /// What a click picked before: bodies, components, and the pick filter's kinds turned off.
    picking: Picking,
    /// The view before the part was framed.
    cam: solidcraft_viewport::Camera,
}

impl Pick {
    /// The frame the picks make, in the part's coordinates.
    fn frame(&self) -> Option<Frame> {
        let (seat, edge) = (self.face.as_ref()?, self.edge.as_ref()?);
        if seat.body != edge.body {
            return None;
        }
        let frame = frame_of(seat.point, seat.normal, edge.near, edge.far, self.from_middle)?;
        Some(if self.centre_across { centred_across(frame, &seat.boundary) } else { frame })
    }

    /// What is wrong with the picks as they stand, or with the last one.
    fn problem(&self) -> Option<String> {
        if self.face.is_some() && self.edge.is_some() && self.frame().is_none() {
            return Some(tr("hardware_connection.not_in_face").to_owned());
        }
        self.refused.clone()
    }
}

/// The frame a seat face (a point on its plane and its normal out of the material) and a
/// reference edge in it (its end nearest the pick, then its other end) make: `z` along the
/// normal, `x` along the edge from `near`, the origin at `near` or, `from_middle`, halfway along
/// the edge. `None` when the edge does not lie in the face's plane.
pub(crate) fn frame_of(point: Vec3, normal: Vec3, near: Vec3, far: Vec3, from_middle: bool) -> Option<Frame> {
    let z = normal.normalized()?;
    let along = far - near;
    let length = along.len();
    if !length.is_finite() || length < MIN_EDGE {
        return None;
    }
    let x = along / length;
    let tol = IN_FACE + IN_FACE_SHARE * length;
    if x.dot(z).abs() > MAX_LEAN || (near - point).dot(z).abs() > tol || (far - point).dot(z).abs() > tol {
        return None;
    }
    let x = (x - z * x.dot(z)).normalized()?;
    let origin = if from_middle { near.lerp(far, 0.5) } else { near };
    Some(Frame { origin, z, x })
}

/// `frame` with its origin moved along `y = z × x` to the middle of a face's width: halfway
/// between the least and the most of the face's `boundary` points along `y`. Its place along `x`
/// and its axes stay; no boundary leaves it as it is.
pub(crate) fn centred_across(frame: Frame, boundary: &[Vec3]) -> Frame {
    let y = frame.z.cross(frame.x);
    let across = boundary.iter().map(|p| (*p - frame.origin).dot(y)).filter(|d| d.is_finite());
    let span = across.fold(None, |span: Option<(f64, f64)>, d| Some(span.map_or((d, d), |(lo, hi)| (lo.min(d), hi.max(d)))));
    match span {
        Some((lo, hi)) => Frame { origin: frame.origin + y * ((lo + hi) / 2.0), ..frame },
        None => frame,
    }
}

/// Points of face `index` of `b`'s exact boundary: each of its edges' path segments, at their
/// ends and seven points between (in `b`'s coordinates). Empty for a face without edges.
pub(crate) fn face_boundary(b: &solidcraft_engine::doc::ModelBody, index: usize) -> Vec<Vec3> {
    let mut points = Vec::new();
    for edge in solidcraft_engine::kernel::face_edge_indices(&b.body, index) {
        let Ok(path) = solidcraft_engine::kernel::edge_path(&b.body, edge) else { continue };
        for seg in &path {
            points.push(seg.start());
            points.extend((1..8).filter_map(|k| seg.along(f64::from(k) / 8.0).map(|(p, _)| p)));
            points.push(seg.end());
        }
    }
    points
}

/// Where a picking card starts: inside the 3D view's free area (clear of the ribbon, the browser
/// and the panels over the view), `space_16` in from its top left; `None` before the view is laid
/// out.
pub(crate) fn card_corner(app: &SolveApp) -> Option<egui::Pos2> {
    let t = Tokens::get();
    let rect = app.viewport.rect?;
    let view = crate::shell::content_rect(app, rect);
    let mut obstacles = crate::shell::covered_rects(app);
    if let Some(layout) = &app.viewport.layout {
        obstacles.extend([layout.navigation, layout.axis_view]);
    }
    let free = crate::viewport_layout::free_rect(view, &obstacles);
    let free = if free.is_positive() { free } else { view };
    Some(free.left_top() + vec2(t.space_16, t.space_16))
}

/// A frame drawn as its three axes from its origin, `len` long: x, y = z × x and z in the theme's
/// axis colours, and a dot at the origin.
pub(crate) fn paint_triad(painter: &egui::Painter, proj: &Proj, frame: &Frame, len: f64) {
    let t = Tokens::get();
    let axis = |dir: Vec3, colour: egui::Color32| {
        if let (Some(a), Some(b)) = (proj.to_screen(frame.origin), proj.to_screen(frame.origin + dir * len)) {
            painter.line_segment([a, b], Stroke::new(2.5, colour));
        }
    };
    axis(frame.x, t.axis_x);
    axis(frame.z.cross(frame.x), t.axis_y);
    axis(frame.z, t.axis_z);
    if let Some(c) = proj.to_screen(frame.origin) {
        painter.circle(c, 4.5, t.panel, Stroke::new(1.5, t.text));
    }
}

/// Points and directions in the world as a part placed at `placement` has them: the placement's
/// inverse. `None` for a placement that can't be inverted.
pub(crate) struct PartCoordinates(Mat);

impl PartCoordinates {
    pub(crate) fn of(placement: &Mat) -> Option<Self> {
        mat_inverse(placement).map(Self)
    }

    pub(crate) fn point(&self, p: Vec3) -> Vec3 {
        apply_point(&self.0, p)
    }

    pub(crate) fn direction(&self, v: Vec3) -> Option<Vec3> {
        apply_vector(&self.0, v).normalized()
    }
}

/// `frame` (in a part's coordinates) where the part sits, placed at `placement`.
pub(crate) fn in_world(placement: &Mat, frame: &Frame) -> Option<Frame> {
    Some(Frame {
        origin: apply_point(placement, frame.origin),
        z: apply_vector(placement, frame.z).normalized()?,
        x: apply_vector(placement, frame.x).normalized()?,
    })
}

/// The first and last of `points` when every point lies on the line through them.
pub(crate) fn straight(points: &[Vec3]) -> Option<(Vec3, Vec3)> {
    let (a, b) = (*points.first()?, *points.last()?);
    let length = a.dist(b);
    if !length.is_finite() || length < MIN_EDGE {
        return None;
    }
    let dir = (b - a).normalized()?;
    let tol = STRAIGHT + STRAIGHT_SHARE * length;
    points.iter().all(|p| (*p - a).cross(dir).len() <= tol).then_some((a, b))
}

/// The ends of edge `index` of `b` when it is straight: from its exact curve, or, for an edge
/// whose curve the record holds only as its surfaces, from the drawn edge.
pub(crate) fn straight_ends(b: &solidcraft_engine::doc::ModelBody, index: usize) -> Option<(Vec3, Vec3)> {
    let points: Vec<Vec3> = match solidcraft_engine::kernel::edge_path(&b.body, index) {
        Ok(path) if !path.is_empty() => {
            path.iter().flat_map(|seg| [Some(seg.start()), seg.along(0.5).map(|(p, _)| p), Some(seg.end())]).flatten().collect()
        }
        _ => b.mesh().edges.get(index)?.clone(),
    };
    straight(&points)
}

/// Whether a connection point is being picked.
pub(crate) fn active(app: &SolveApp) -> bool {
    app.hardware_connection.is_some()
}

/// Where the import's top occurrence sits in the world now; `None` once it is gone (undone).
fn placement(s: &Session, occurrence: u64) -> Option<Mat> {
    let doc = s.placed_document();
    doc.occurrences.iter().any(|o| o.id == occurrence).then(|| solidcraft_engine::doc::joints::occurrence_world(&doc, occurrence))
}

/// Start picking the connection point of the hardware STEP part at `path`: a checkpoint, the
/// part inserted as `name` and framed, and the card. Refused while a sketch or an edit in place
/// is open; an open dialog or tool is closed first.
pub(crate) fn start(app: &mut SolveApp, path: &str, name: &str) -> Result<(), String> {
    if active(app) {
        return Ok(());
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
    app.home.open = false;
    let checkpoint = app.run("edit.checkpoint", json!({}))?["checkpoint"].as_u64().ok_or("edit.checkpoint gave no checkpoint")?;
    let inserted = app.run("file.insert_step", json!({"path": path, "name": name}));
    let occurrence = match inserted.as_ref().map(|r| r["occurrence"].as_u64()) {
        Ok(Some(occurrence)) => occurrence,
        other => {
            let _ = app.run("edit.restore_checkpoint", json!({"checkpoint": checkpoint}));
            return Err(match other {
                Err(e) => e.clone(),
                Ok(_) => "the part came in without a placement".to_owned(),
            });
        }
    };
    let picking = pick_faces_and_edges(app);
    app.session.selection.clear();
    let cam = app.cam;
    let bounds = crate::dialogs_assembly::occurrence_bounds(&app.session, &[occurrence]);
    if !bounds.is_empty() {
        let mut framed = app.cam;
        framed.fit(&solidcraft_engine::render::view_bounds(&bounds));
        app.animate_to(framed);
    }
    app.hardware_connection = Some(Pick {
        path: path.to_owned(),
        name: name.to_owned(),
        checkpoint,
        session: app.session.uid,
        document: app.session.document_id.clone(),
        occurrence,
        face: None,
        edge: None,
        from_middle: false,
        centre_across: true,
        refused: None,
        picking,
        cam,
    });
    Ok(())
}

/// Whether `body` is one of the part's bodies (in the import's top occurrence or one inside it).
fn on_part(app: &SolveApp, pick: &Pick, body: &str) -> bool {
    crate::dialogs_assembly::placement_of_body(&app.session, body).is_some_and(|path| path.contains(&pick.occurrence))
}

/// The seat face a face pick stands for, or the message id saying why it is none.
fn seat(app: &SolveApp, pick: &Pick, sel: &Sel) -> Result<Seat, &'static str> {
    let Sel::Face { body, index, .. } = sel else { return Err("hardware_connection.not_flat") };
    if !on_part(app, pick, body) {
        return Err("hardware_connection.not_on_part");
    }
    let (point, normal) = crate::dialogs::planar_face(&app.session, body, *index).ok_or("hardware_connection.not_flat")?;
    let part = placement(&app.session, pick.occurrence).and_then(|m| PartCoordinates::of(&m)).ok_or("hardware_connection.not_on_part")?;
    let normal = part.direction(normal).ok_or("hardware_connection.not_flat")?;
    let world = crate::open_meshing::session_world(&app.session);
    let boundary = world.body(body).map(|b| face_boundary(b, *index)).unwrap_or_default().into_iter().map(|p| part.point(p)).collect();
    Ok(Seat { sel: sel.clone(), body: body.clone(), point: part.point(point), normal, boundary })
}

/// The reference edge an edge pick stands for, or the message id saying why it is none.
fn reference(app: &SolveApp, pick: &Pick, sel: &Sel) -> Result<Reference, &'static str> {
    let Sel::Edge { body, index, point } = sel else { return Err("hardware_connection.not_in_face") };
    if !on_part(app, pick, body) {
        return Err("hardware_connection.not_on_part");
    }
    let world = crate::open_meshing::session_world(&app.session);
    let b = world.body(body).ok_or("hardware_connection.not_on_part")?;
    let (a, z) = straight_ends(b, *index).ok_or("hardware_connection.not_in_face")?;
    let (near, far) = if a.dist(*point) <= z.dist(*point) { (a, z) } else { (z, a) };
    let part = placement(&app.session, pick.occurrence).and_then(|m| PartCoordinates::of(&m)).ok_or("hardware_connection.not_on_part")?;
    Ok(Reference { sel: sel.clone(), body: body.clone(), near: part.point(near), far: part.point(far) })
}

/// Take the faces and edges clicked since the last frame: a face is the seat face, an edge the
/// reference edge. The selection then shows the two picks.
fn take_picks(app: &mut SolveApp) {
    let Some(mut pick) = app.hardware_connection.take() else { return };
    let picked: Vec<Sel> = pick.face.iter().map(|f| f.sel.clone()).chain(pick.edge.iter().map(|e| e.sel.clone())).collect();
    let clicked: Vec<Sel> = app.session.selection.iter().filter(|sel| !picked.contains(sel)).cloned().collect();
    for sel in &clicked {
        match sel {
            Sel::Face { .. } => match seat(app, &pick, sel) {
                Ok(seat) => {
                    pick.face = Some(seat);
                    pick.refused = None;
                }
                Err(id) => pick.refused = Some(tr(id).to_owned()),
            },
            Sel::Edge { .. } => match reference(app, &pick, sel) {
                Ok(edge) => {
                    pick.edge = Some(edge);
                    pick.refused = None;
                }
                Err(id) => pick.refused = Some(tr(id).to_owned()),
            },
            _ => {}
        }
    }
    let shown: Vec<Sel> = pick.face.iter().map(|f| f.sel.clone()).chain(pick.edge.iter().map(|e| e.sel.clone())).collect();
    if app.session.selection != shown {
        app.session.selection = shown;
    }
    app.hardware_connection = Some(pick);
}

/// What a click picked before a picking mode: bodies, components, and the pick filter's kinds
/// turned off.
pub(crate) type Picking = (bool, bool, Vec<String>);

/// Make a click pick faces and edges only (not bodies, components or corners), for a picking
/// mode; returns what it picked before, for [`restore_picking`].
pub(crate) fn pick_faces_and_edges(app: &mut SolveApp) -> Picking {
    let picking = (app.ui.pick_bodies, app.ui.pick_components, app.ui.pick_off.clone());
    app.ui.pick_bodies = false;
    app.ui.pick_components = false;
    app.ui.pick_off.retain(|kind| !PICKED.contains(&kind.as_str()));
    if !app.ui.pick_off.iter().any(|kind| kind == NOT_PICKED) {
        app.ui.pick_off.push(NOT_PICKED.to_owned());
    }
    picking
}

/// Put the pick filter back as [`pick_faces_and_edges`] found it.
pub(crate) fn restore_picking(app: &mut SolveApp, picking: &Picking) {
    let (bodies, components, off) = picking.clone();
    app.ui.pick_bodies = bodies;
    app.ui.pick_components = components;
    app.ui.pick_off = off;
}

/// Put back what picking changed in the app: the pick filter and the view.
fn put_back(app: &mut SolveApp, pick: &Pick, view: bool) {
    restore_picking(app, &pick.picking);
    if view {
        app.animate_to(pick.cam);
    }
}

/// Go back to the checkpoint taken before the part came in, and stop picking.
fn finish(app: &mut SolveApp, pick: Pick) {
    if app.session.active_sketch.is_some() {
        app.cancel_sketch();
    }
    if app.dialog.is_some() {
        crate::dialogs::cancel(app);
    }
    if let Err(e) = app.run("edit.restore_checkpoint", json!({"checkpoint": pick.checkpoint})) {
        app.set_status(e, true);
    }
    app.session.selection.clear();
    put_back(app, &pick, true);
}

/// The design was switched away from (another tab) or replaced: its tab goes back to the
/// checkpoint behind the scenes, and picking stops.
fn abandon(app: &mut SolveApp, pick: Pick) {
    if let Some(slot) = app.docs.background_slot_mut(pick.session)
        && slot.session.document_id == pick.document
    {
        let _ = slot.session.execute("edit.restore_checkpoint", &json!({"checkpoint": pick.checkpoint}));
    }
    put_back(app, &pick, false);
}

/// Save: the frame goes to the part's data card, in the part's coordinates, and the design goes
/// back to the checkpoint. Refused (and the card stays up, saying why) when the picks make no
/// frame or the card can't be written.
fn save(app: &mut SolveApp) -> Result<(), String> {
    let Some(mut pick) = app.hardware_connection.take() else { return Ok(()) };
    let written = match pick.frame() {
        Some(f) => app
            .run(
                "hardware.card.set",
                json!({"path": pick.path, "connection": {"origin": [f.origin.x, f.origin.y, f.origin.z], "z": [f.z.x, f.z.y, f.z.z], "x": [f.x.x, f.x.y, f.x.z]}}),
            )
            .map(drop),
        None => Err(tr("hardware_connection.not_in_face").to_owned()),
    };
    if let Err(e) = written {
        pick.refused = Some(e.clone());
        app.hardware_connection = Some(pick);
        return Err(e);
    }
    finish(app, pick);
    crate::hardware_library::refresh(app);
    app.set_status(tr("hardware_connection.saved"), false);
    Ok(())
}

/// Cancel: the design goes back to the checkpoint, and no card is written.
fn cancel(app: &mut SolveApp) {
    if let Some(pick) = app.hardware_connection.take() {
        finish(app, pick);
    }
}

/// The card, while a connection point is being picked: what to pick, and Save and Cancel.
pub(crate) fn show(app: &mut SolveApp, ctx: &egui::Context) {
    let Some(pick) = app.hardware_connection.as_ref() else { return };
    if app.session.uid != pick.session || app.session.document_id != pick.document {
        if let Some(pick) = app.hardware_connection.take() {
            abandon(app, pick);
        }
        return;
    }
    // Picking is a mode: a dialog or tool started meanwhile would change a design that goes back.
    if app.dialog.is_some() || app.tool.is_some() {
        if app.dialog.is_some() {
            crate::dialogs::cancel(app);
        }
        app.tool = None;
        app.set_status(tr("hardware_connection.busy"), true);
    }
    take_picks(app);
    let Some(mut pick) = app.hardware_connection.take() else { return };
    let valid = pick.frame().is_some();
    let window = ui_kit::Window::new(egui::Id::unique("sc_hardware_connection_card"), "joint_origin", tr("hardware_connection.title"))
        .actions(tr("ui.save"), tr("ui.cancel"), None)
        .cards(1.25);
    let window = match card_corner(app) {
        Some(corner) => window.placed(egui::Align2::LEFT_TOP, corner),
        None => window.anchor(egui::Align2::CENTER_TOP, vec2(0.0, 140.0)),
    };
    let (closed, action) = window.show(ctx, valid, |ui| card(ui, &mut pick));
    app.hardware_connection = Some(pick);
    match action {
        ui_kit::Action::Ok => {
            let _ = save(app);
        }
        ui_kit::Action::Cancel => cancel(app),
        _ if closed => cancel(app),
        _ => {}
    }
}

/// The card's body: the part, the two picks and what each wants, the middle choice, and what is
/// wrong.
fn card(ui: &mut egui::Ui, pick: &mut Pick) {
    ui_kit::text(ui, &pick.name, ui_kit::Text::Report("hardware_connection_part", false));
    for (label, hint, picked) in [
        ("hardware_connection.seat_face", "hardware_connection.seat_face_hint", pick.face.is_some()),
        ("hardware_connection.reference_edge", "hardware_connection.reference_edge_hint", pick.edge.is_some()),
    ] {
        ui_kit::text(ui, tr(label), ui_kit::Text::Heading(label));
        let note = if picked { format!("✓ {}", tr("hardware_connection.picked")) } else { tr(hint).to_owned() };
        ui_kit::text(ui, &note, ui_kit::Text::Note(hint));
    }
    ui_kit::checkbox(ui, tr("hardware_connection.from_middle"), &mut pick.from_middle);
    ui_kit::checkbox(ui, tr("hardware_connection.centre_across"), &mut pick.centre_across);
    if let Some(problem) = pick.problem() {
        ui_kit::status_line(ui, &problem, ui_kit::StatusTone::Error);
    }
}

/// The frame as it will be saved, drawn where the part sits: x red, y green, z blue (the theme's
/// axis colours); with only the seat face picked, its normal.
pub(crate) fn overlay(app: &SolveApp, painter: &egui::Painter, proj: &Proj) {
    let Some(pick) = app.hardware_connection.as_ref() else { return };
    let Some(placement) = placement(&app.session, pick.occurrence) else { return };
    let t = Tokens::get();
    let len = triad_length(app);
    if let Some(frame) = pick.frame().and_then(|f| in_world(&placement, &f)) {
        paint_triad(painter, proj, &frame, len);
    } else if let Some(seat) = &pick.face {
        let at = apply_point(&placement, seat.point);
        if let Some(normal) = apply_vector(&placement, seat.normal).normalized()
            && let (Some(a), Some(b)) = (proj.to_screen(at), proj.to_screen(at + normal * len))
        {
            painter.line_segment([a, b], Stroke::new(2.5, t.axis_z));
        }
    }
}

/// How long a picking triad's axes are drawn: a share of the view's height.
pub(crate) fn triad_length(app: &SolveApp) -> f64 {
    app.cam.half_height() * 2.0 * 0.06
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn close(a: Vec3, b: Vec3) -> bool {
        a.dist(b) < 1e-9
    }

    #[test]
    fn a_seat_face_and_an_edge_in_it_make_the_frame() {
        let (point, normal) = (Vec3::new(5.0, 5.0, 2.0), Vec3::new(0.0, 0.0, 3.0));
        let (near, far) = (Vec3::new(1.0, 0.0, 2.0), Vec3::new(11.0, 0.0, 2.0));
        let f = frame_of(point, normal, near, far, false).unwrap();
        assert!(close(f.origin, near) && close(f.z, Vec3::Z) && close(f.x, Vec3::X), "{f:?}");
        let f = frame_of(point, normal, near, far, true).unwrap();
        assert!(close(f.origin, Vec3::new(6.0, 0.0, 2.0)), "from the middle: {f:?}");
        // x points away from the end nearest the click.
        let f = frame_of(point, normal, far, near, false).unwrap();
        assert!(close(f.origin, far) && close(f.x, -Vec3::X), "{f:?}");
        // An edge off the face's plane, leaning out of it, or of no length makes none.
        assert!(frame_of(point, normal, near + Vec3::Z, far + Vec3::Z, false).is_none());
        assert!(frame_of(point, normal, near, far + Vec3::Z, false).is_none());
        assert!(frame_of(point, normal, near, near, false).is_none());
        assert!(frame_of(point, Vec3::ZERO, near, far, false).is_none());
    }

    #[test]
    fn centred_across_moves_the_origin_to_the_middle_of_the_faces_width() {
        // A seat face 8 mm across (y from -1 to 7 with x along the edge, z up), its edge at y = 0.
        let frame = Frame { origin: Vec3::new(2.0, 0.0, 0.0), z: Vec3::Z, x: Vec3::X };
        let boundary = [Vec3::new(0.0, -1.0, 0.0), Vec3::new(40.0, -1.0, 0.0), Vec3::new(40.0, 7.0, 0.0), Vec3::new(0.0, 5.0, 0.0)];
        let centred = centred_across(frame, &boundary);
        assert!(close(centred.origin, Vec3::new(2.0, 3.0, 0.0)), "along x it stays: {centred:?}");
        assert!(close(centred.x, frame.x) && close(centred.z, frame.z));
        // y = z × x: with the seat face facing down, y runs the other way, and the middle is the same.
        let down = Frame { origin: Vec3::new(2.0, 0.0, 0.0), z: -Vec3::Z, x: Vec3::X };
        assert!(close(centred_across(down, &boundary).origin, Vec3::new(2.0, 3.0, 0.0)));
        assert_eq!(centred_across(frame, &[]), frame, "no boundary: as it was");
    }

    #[test]
    fn a_frame_picked_in_the_world_is_kept_in_the_parts_own_coordinates() {
        // The part turned a quarter turn about Z and moved: its own x is the world's y.
        let placement = solidcraft_engine::doc::rigid(Vec3::new(10.0, 20.0, 30.0), Vec3::ZERO, Vec3::Z, std::f64::consts::FRAC_PI_2);
        let own = Frame { origin: Vec3::new(1.0, 2.0, 3.0), z: -Vec3::Z, x: Vec3::X };
        let world = in_world(&placement, &own).unwrap();
        assert!(close(world.origin, Vec3::new(8.0, 21.0, 33.0)), "{world:?}");
        assert!(close(world.x, Vec3::Y) && close(world.z, -Vec3::Z), "{world:?}");
        let part = PartCoordinates::of(&placement).unwrap();
        assert!(close(part.point(world.origin), own.origin));
        assert!(close(part.direction(world.x).unwrap(), own.x) && close(part.direction(world.z * 4.0).unwrap(), own.z));
        // A frame made from the picks taken back through the placement is the part's own.
        let near = part.point(world.origin);
        let far = part.point(world.origin + world.x * 25.0);
        let seat = part.point(world.origin + world.z.cross(world.x) * 7.0);
        let made = frame_of(seat, part.direction(world.z).unwrap(), near, far, false).unwrap();
        assert!(close(made.origin, own.origin) && close(made.x, own.x) && close(made.z, own.z), "{made:?}");
        assert!(PartCoordinates::of(&[[0.0; 4]; 4]).is_none(), "a placement with no inverse");
    }

    #[test]
    fn only_a_straight_edge_has_ends() {
        let line = [Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0), Vec3::new(10.0, 0.0, 0.0)];
        assert_eq!(straight(&line), Some((Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0))));
        assert_eq!(straight(&[Vec3::ZERO, Vec3::new(5.0, 1.0, 0.0), Vec3::new(10.0, 0.0, 0.0)]), None, "an arc");
        assert_eq!(straight(&[Vec3::ZERO, Vec3::new(5.0, 1.0, 0.0), Vec3::ZERO]), None, "a closed edge");
        assert_eq!(straight(&[]), None);
    }

    /// A box part exported as STEP into a temporary folder, removed when dropped.
    struct Part(std::path::PathBuf);
    impl Part {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("solidcraft-hardware-connection-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let mut s = Session::default();
            s.execute("solid.box", &json!({"length": 30, "width": 20, "height": 10, "corner": [0, 0, 0]})).unwrap();
            s.execute("file.export", &json!({"path": dir.join("corner.step").to_string_lossy()})).unwrap();
            Part(dir)
        }
        fn step(&self) -> String {
            self.0.join("corner.step").to_string_lossy().into_owned()
        }
    }
    impl Drop for Part {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn picking_a_seat_face_and_an_edge_writes_the_card_and_leaves_the_design_as_it_was() {
        let part = Part::new();
        let mut app = crate::SolveApp::new(Session::default(), crate::Services::default());
        app.run("solid.box", json!({"length": 5, "width": 5, "height": 5, "corner": [100, 0, 0]})).unwrap();
        app.ui.pick_components = true;
        let (undo, doc) = (app.session.undo.len(), app.session.doc.clone());
        start(&mut app, &part.step(), "Corner").unwrap();
        assert!(active(&app) && !app.ui.pick_components, "a click picks faces and edges while picking");
        let occurrence = app.hardware_connection.as_ref().unwrap().occurrence;
        let world = crate::open_meshing::session_world(&app.session);
        let body = world
            .bodies
            .iter()
            .find(|b| crate::dialogs_assembly::placement_of_body(&app.session, &b.name).is_some_and(|p| p.contains(&occurrence)))
            .expect("the part's body");
        // Its bottom face (z = 0, facing down) and its edge along X from the origin.
        let face = (0..body.body.face_count())
            .find(|&i| crate::dialogs::planar_face(&app.session, &body.name, i).is_some_and(|(_, n)| n.z < -0.99))
            .expect("the bottom face");
        let edge = (0..body.body.edge_count())
            .find(|&i| {
                straight_ends(body, i).is_some_and(|(a, b)| {
                    let (lo, hi) = if a.x < b.x { (a, b) } else { (b, a) };
                    close(lo, Vec3::ZERO) && close(hi, Vec3::new(30.0, 0.0, 0.0))
                })
            })
            .expect("the edge along X");
        let name = body.name.clone();
        // A pick on the design's own box is not the part's.
        let own = world.bodies.iter().find(|b| b.name != name).map(|b| b.name.clone()).unwrap();
        app.session.selection = vec![Sel::Face { body: own, index: 0, point: Vec3::new(102.0, 2.0, 0.0) }];
        take_picks(&mut app);
        assert!(app.hardware_connection.as_ref().unwrap().face.is_none() && app.session.selection.is_empty());
        app.session.selection = vec![Sel::Face { body: name.clone(), index: face, point: Vec3::new(15.0, 10.0, 0.0) }];
        take_picks(&mut app);
        app.session.selection.push(Sel::Edge { body: name, index: edge, point: Vec3::new(2.0, 0.0, 0.0) });
        take_picks(&mut app);
        assert_eq!(app.session.selection.len(), 2, "the selection shows both picks");
        let pick = app.hardware_connection.as_mut().unwrap();
        assert!(pick.centre_across, "centred across by default");
        pick.centre_across = false;
        assert!(close(pick.frame().unwrap().origin, Vec3::ZERO), "without it, the end nearest the click");
        pick.centre_across = true;
        save(&mut app).unwrap();

        let card: Value = serde_json::from_str(&std::fs::read_to_string(part.0.join("corner.json")).unwrap()).unwrap();
        let v = |key: &str| {
            let a: Vec<f64> = card["connection"][key].as_array().unwrap().iter().map(|n| n.as_f64().unwrap()).collect();
            Vec3::new(a[0], a[1], a[2])
        };
        // The end nearest the click (x = 0), moved across the 20 mm wide face to its middle.
        assert!(close(v("origin"), Vec3::new(0.0, 10.0, 0.0)), "the end nearest the click, centred across: {card}");
        assert!(close(v("z"), -Vec3::Z), "out of the seat face: {card}");
        assert!(close(v("x"), Vec3::X), "along the edge: {card}");
        assert!(!active(&app));
        assert!(app.ui.pick_components, "the pick filter is put back");
        assert_eq!(app.session.undo.len(), undo, "no undo step is left");
        assert!(std::sync::Arc::ptr_eq(&app.session.doc, &doc) || *app.session.doc == *doc, "the part is gone again");
    }

    #[test]
    fn cancel_leaves_no_card_and_the_design_as_it_was() {
        let part = Part::new();
        let mut app = crate::SolveApp::new(Session::default(), crate::Services::default());
        let undo = app.session.undo.len();
        start(&mut app, &part.step(), "Corner").unwrap();
        assert_eq!(app.session.doc.occurrences.len(), 1, "the part is in for the picks");
        cancel(&mut app);
        assert!(!active(&app));
        assert!(app.session.doc.occurrences.is_empty() && app.session.undo.len() == undo);
        assert!(!part.0.join("corner.json").exists());
        // A missing file is refused, and leaves nothing behind.
        assert!(start(&mut app, &part.0.join("missing.step").to_string_lossy(), "Missing").is_err());
        assert!(!active(&app) && app.session.doc.occurrences.is_empty());
    }
}
