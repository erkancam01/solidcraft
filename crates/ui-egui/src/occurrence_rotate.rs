//! A right drag on a part turns it, with no command running: a free part (one a left drag moves,
//! `occurrence_drag`) turns like a trackball about the centre of its box, by `occurrence.move`; a
//! part that joints place and leave free to turn (`joint_drag`'s parts) turns as far as its
//! joints allow, by `joint.reach` with its box centre kept where it is and one of its axes turned.
//! A part mated face to face only spins about the faces' normal.
//!
//! The press only claims the right button: until it can no longer be a click it is still the
//! context menu's, and navigation leaves the claimed drag alone ([`claims_right`]). A press on a
//! grounded or fully placed part, on empty space or with a key held is the view's as before. The
//! whole turn is one undo step, Escape puts the part back, and an undo pressed mid-turn ends it.

use std::sync::Arc;

use egui::{Pos2, Rect};
use serde_json::json;
use solidcore_record::Vec3;
use solidcraft_engine::doc::{Document, apply_point, apply_vector, joints, mat_inverse};
use solidcraft_engine::geom::Aabb3;
use solidcraft_engine::geom::prelude::*;

use crate::SolveApp;
use crate::viewport::Proj;

/// How far a part turns for each point the pointer moves.
const RADIANS_PER_POINT: f64 = 0.01;

/// What turns the part.
#[derive(Clone, Debug)]
enum Turn {
    /// Nothing holds it: `occurrence.move` turns this placement (its exact path).
    Free { path: Vec<u64> },
    /// Joints place it: `joint.reach` turns it within them.
    Joints { occurrence: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// The right button went down on the part; the press may still be a click.
    Pressed,
    /// The part follows the pointer.
    Turning,
    /// Escape or an undo ended the turn; the rest of the press does nothing.
    Spent,
}

/// A right press on a part that can turn, and the turn under way.
#[derive(Clone, Debug)]
pub struct Rotate {
    turn: Turn,
    /// The centre of the part's box where the press began, in the part's own coordinates.
    local: Vec3,
    /// Where the pointer was when the part last turned.
    last: Pos2,
    phase: Phase,
    /// Where Escape takes the design back to (`edit.checkpoint`).
    checkpoint: Option<u64>,
    /// The design the turn's undo step holds, once the first frame that turned made it; it stays
    /// on top of the history while the turn runs (an undo pressed mid-turn takes it and ends it).
    step: Option<Arc<Document>>,
}

/// A part is turning.
pub fn active(app: &SolveApp) -> bool {
    app.viewport.occurrence_rotate.as_ref().is_some_and(|r| r.phase == Phase::Turning)
}

/// The right button that is down belongs to a part (pressed on one that can turn): navigation
/// does not move the view with it.
pub fn claims_right(app: &SolveApp) -> bool {
    app.viewport.occurrence_rotate.is_some()
}

/// The centre of the box around the bodies under the placement `path`, in the world.
fn centre(app: &SolveApp, path: &[u64]) -> Option<Vec3> {
    let s = &app.session;
    let world = crate::open_meshing::session_world(s);
    let bb = s
        .doc
        .body_frames(&s.model.state())
        .into_iter()
        .filter(|(_, _, p, _)| p.starts_with(path))
        .filter_map(|(name, ..)| world.body(&name).map(|b| b.mesh().bounds()))
        .fold(Aabb3::EMPTY, |bb, b| bb.union(&b));
    (!bb.is_empty()).then(|| bb.center()).filter(|c| c.is_finite())
}

/// Where the part is: its placement in the world.
fn frame(app: &SolveApp, turn: &Turn) -> Option<[[f64; 4]; 4]> {
    match turn {
        Turn::Free { path } => app.session.occurrence_placement(path).ok().map(|x| x.world),
        Turn::Joints { occurrence } => Some(joints::occurrence_world(&app.session.doc, *occurrence)),
    }
}

/// What a right press at `p` would turn, and its box centre in its own coordinates: a free part
/// alone (no chain of joints moves with it), or a part its joints leave free to move.
fn target(app: &SolveApp, proj: &Proj, p: Pos2) -> Option<(Turn, Vec3)> {
    let (body, _) = crate::occurrence_drag::grabbed_face(app, proj, p)?;
    let s = &app.session;
    let local = |turn: Turn, path: &[u64]| {
        let c = centre(app, path)?;
        let inv = mat_inverse(&frame(app, &turn)?)?;
        Some((turn, apply_point(&inv, c)))
    };
    if let Some(path) = crate::occurrence_drag::placement_of(app, &body)
        && s.drag_group(&path).is_ok_and(|group| group.len() == 1)
    {
        return local(Turn::Free { path: path.clone() }, &path);
    }
    let path = crate::dialogs_assembly::placement_of_body(s, &body)?;
    let occurrence = path.last().copied().filter(|o| *o != 0)?;
    if !s.doc.assembly.joints.iter().any(|j| !j.suppressed && (j.a.occurrence == occurrence || j.b.occurrence == occurrence)) {
        return None;
    }
    let sol = joints::solve(&s.doc, &s.model.state());
    if !sol.world.contains_key(&occurrence) || sol.dof.get(&occurrence).is_none_or(|d| *d == 0) {
        return None;
    }
    local(Turn::Joints { occurrence }, &path)
}

/// Nothing else has the pointer or the view: no command, dialog, sketch, menu, drag or ring, and
/// no key held with the press.
fn free_to_turn(app: &SolveApp, mods: egui::Modifiers, other_buttons: bool) -> bool {
    app.tool.is_none()
        && app.dialog.is_none()
        && app.session.active_sketch.is_none()
        && app.viewport.context_menu.is_none()
        && !(mods.shift || mods.ctrl || mods.alt || mods.command || mods.mac_cmd)
        && !other_buttons
        && !crate::mouse_gesture::active(app)
        && !crate::render_workspace::active(app)
        && !crate::occurrence_drag::active(app)
        && !crate::joint_drag::active(app)
        && !crate::select_other::is_open(app)
}

/// One frame of the right button over the view (after navigation): a press on a part that can
/// turn claims it, the turn starts once the press can no longer be a click, and the part follows
/// the pointer until the button is let go.
pub(crate) fn update(app: &mut SolveApp, ctx: &egui::Context, proj: &Proj, rect: Rect, axes: Rect, inside: bool) {
    let (pressed, down, still, origin, pos, mods, other_buttons) = ctx.input(|i| {
        (
            i.pointer.button_pressed(egui::PointerButton::Secondary),
            i.pointer.secondary_down(),
            i.pointer.could_any_button_be_click(),
            i.pointer.press_origin(),
            i.pointer.latest_pos(),
            i.modifiers,
            i.pointer.primary_down() || i.pointer.middle_down(),
        )
    });
    if !down || pressed {
        // Let go (or a new press without one): the part stays where it was turned.
        end(app);
        app.viewport.occurrence_rotate = None;
    }
    if !down {
        return;
    }
    if pressed
        && inside
        && free_to_turn(app, mods, other_buttons)
        && let Some(a) = origin.filter(|a| a.is_finite() && rect.contains(*a) && !axes.contains(*a) && !crate::shell::covered(app, *a))
        && let Some((turn, local)) = target(app, proj, a)
    {
        app.viewport.occurrence_rotate = Some(Rotate { turn, local, last: a, phase: Phase::Pressed, checkpoint: None, step: None });
    }
    if !still && app.viewport.occurrence_rotate.as_ref().is_some_and(|r| r.phase == Phase::Pressed) {
        let checkpoint = app.session.execute("edit.checkpoint", &json!({})).ok().and_then(|r| r["checkpoint"].as_u64());
        if let Some(r) = app.viewport.occurrence_rotate.as_mut() {
            r.phase = Phase::Turning;
            r.checkpoint = checkpoint;
        }
    }
    if active(app) {
        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        if let Some(p) = pos.filter(|p| p.is_finite()) {
            to(app, proj, p);
        }
    }
}

/// Turn the part by the pointer's move from where it last turned to `p`: about the axis across
/// the move, in the plane through the part's centre that faces the view, so the side nearest the
/// eye follows the pointer.
fn to(app: &mut SolveApp, proj: &Proj, p: Pos2) {
    let Some(r) = app.viewport.occurrence_rotate.clone().filter(|r| r.phase == Phase::Turning) else { return };
    // Something else took the turn's undo step (undo pressed mid-turn): the turn ends there.
    if let Some(doc) = &r.step
        && !app.session.undo.last().is_some_and(|step| Arc::ptr_eq(&step.doc, doc))
    {
        end(app);
        return;
    }
    let moved = p - r.last;
    if !moved.is_finite() || moved.length() < 0.5 {
        return;
    }
    if let Some(x) = app.viewport.occurrence_rotate.as_mut() {
        x.last = p;
    }
    let Some(world) = frame(app, &r.turn) else {
        end(app);
        return;
    };
    let c = apply_point(&world, r.local);
    let (o0, d0) = proj.ray(r.last);
    let (o1, d1) = proj.ray(p);
    let on_plane = |o: Vec3, d: Vec3| {
        let facing = d.dot(d1);
        (facing.abs() > 1e-9).then(|| o + d * ((c - o).dot(d1) / facing))
    };
    let (Some(w0), Some(w1)) = (on_plane(o0, d0), on_plane(o1, d1)) else { return };
    let angle = f64::from(moved.length()) * RADIANS_PER_POINT;
    let Some(k) = (w1 - w0).cross(d1).normalized().filter(|k| k.is_finite() && c.is_finite() && angle.is_finite()) else { return };
    let (id, params) = match &r.turn {
        Turn::Free { path } => (
            "occurrence.move",
            json!({"occurrence": path.last(), "path": path, "space": "world", "axis": [k.x, k.y, k.z], "angle": angle, "origin": [c.x, c.y, c.z]}),
        ),
        Turn::Joints { occurrence } => {
            // The part's axis most across the turn's: turned about `k`, it says the whole turn.
            let Some(u) = [Vec3::X, Vec3::Y, Vec3::Z]
                .map(|v| apply_vector(&world, v))
                .into_iter()
                .filter_map(|v| v.normalized())
                .min_by(|a, b| a.dot(k).abs().total_cmp(&b.dot(k).abs()))
            else {
                return;
            };
            let z = crate::joint_drag::turned(u, k, angle);
            (
                "joint.reach",
                json!({"occurrence": occurrence, "at": [c.x, c.y, c.z], "point": [c.x, c.y, c.z], "axis": [u.x, u.y, u.z],
                    "z": [z.x, z.y, z.z], "quiet": true, "continuous": true}),
            )
        }
    };
    // One undo step for the whole turn: the first frame's, kept on top while the others run.
    let own = if r.step.is_some() { app.session.undo.pop() } else { None };
    let before = app.session.doc.clone();
    let ran = match r.turn {
        // A frame of a gesture is not echoed on the command line; `occurrence.move` has no
        // `continuous` to say so, so it runs in the engine alone.
        Turn::Free { .. } => app.session.execute(id, &params).is_ok(),
        Turn::Joints { .. } => app.run(id, params).is_ok(),
    };
    let pushed = ran && !Arc::ptr_eq(&before, &app.session.doc);
    match own {
        Some(step) => {
            if pushed {
                app.session.undo.pop();
            }
            app.session.undo.push(step);
        }
        None if pushed => {
            let step = app.session.undo.last().map(|step| step.doc.clone());
            if let Some(r) = app.viewport.occurrence_rotate.as_mut() {
                r.step = step;
            }
        }
        None => {}
    }
}

/// The turn is over: the part stays where it was turned. The press keeps the right button until
/// it is let go.
pub fn end(app: &mut SolveApp) {
    let Some(r) = app.viewport.occurrence_rotate.as_mut().filter(|r| r.phase == Phase::Turning) else { return };
    r.phase = Phase::Spent;
    if let Some(checkpoint) = r.checkpoint.take() {
        let _ = app.session.execute("edit.restore_checkpoint", &json!({"checkpoint": checkpoint, "forget": true}));
    }
}

/// Escape: the part goes back, and the turn leaves no undo step. True when a turn was cancelled.
pub fn cancel(app: &mut SolveApp) -> bool {
    let Some(r) = app.viewport.occurrence_rotate.as_mut().filter(|r| r.phase == Phase::Turning) else { return false };
    r.phase = Phase::Spent;
    let (checkpoint, step) = (r.checkpoint.take(), r.step.take());
    match checkpoint {
        Some(checkpoint) => drop(app.session.execute("edit.restore_checkpoint", &json!({"checkpoint": checkpoint}))),
        // Without a checkpoint the turn's own step takes it back.
        None if step.is_some() => drop(app.session.execute("edit.undo", &json!({}))),
        None => {}
    }
    true
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use solidcore_record::Vec3;
    use solidcraft_engine::doc::{apply_point, apply_vector, joints};
    use solidcraft_engine::geom::prelude::*;

    use crate::scenario::Harness;
    use crate::test_support::play;

    fn run(h: &mut Harness, id: &str, p: Value) -> Value {
        h.app.run(id, p.clone()).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
    }

    fn screen(h: &mut Harness, p: Vec3) -> egui::Pos2 {
        let [x, y] = h.at(&json!({"world": [p.x, p.y, p.z]})).expect("on screen");
        egui::pos2(x as f32, y as f32)
    }

    fn right(h: &mut Harness, at: egui::Pos2, pressed: bool) {
        h.app.synthetic.push(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: Default::default(),
        });
    }

    /// Right press at `a` and move to `b` in 12 frames, holding the button there.
    fn press_and_move(h: &mut Harness, a: egui::Pos2, b: egui::Pos2) {
        h.app.synthetic.push(egui::Event::PointerMoved(a));
        right(h, a, true);
        h.app.synthetic.extend((1..=12).map(|k| egui::Event::PointerMoved(a + (b - a) * (k as f32 / 12.0))));
        play(h);
    }

    fn release(h: &mut Harness, b: egui::Pos2) {
        right(h, b, false);
        play(h);
    }

    /// The occurrence's axes in the world, and where its own point `local` is.
    fn placed(h: &Harness, occurrence: u64, local: Vec3) -> ([Vec3; 3], Vec3) {
        let m = joints::occurrence_world(&h.app.session.doc, occurrence);
        ([Vec3::X, Vec3::Y, Vec3::Z].map(|v| apply_vector(&m, v)), apply_point(&m, local))
    }

    fn turned_from_start(axes: &[Vec3; 3]) -> bool {
        axes.iter().zip([Vec3::X, Vec3::Y, Vec3::Z]).any(|(a, b)| a.dist(b) > 0.1)
    }

    /// A grounded base (the design's first part) and a free 10 mm cube beside it, its centre at
    /// (25, 5, 5), seen from the top: (harness, base occurrence, free occurrence).
    fn two_parts() -> (Harness, u64, u64) {
        let mut h = Harness::new();
        h.app.home.open = false;
        let mut occurrences = Vec::new();
        for (name, corner) in [("Base", [-5, -5, 0]), ("Free", [20, 0, 0])] {
            run(&mut h, "component.activate", json!({"component": "root"}));
            occurrences.push(run(&mut h, "component.create", json!({"name": name}))["occurrence"].as_u64().expect("occurrence"));
            run(&mut h, "solid.box", json!({"length": 10, "width": 10, "height": 10, "corner": corner}));
        }
        run(&mut h, "component.activate", json!({"component": "root"}));
        run(&mut h, "occurrence.ground", json!({"occurrence": occurrences[0], "grounded": true}));
        h.call("ui.view", json!({"view": "top"}));
        h.app.cam.distance = 200.0;
        h.frames(3);
        (h, occurrences[0], occurrences[1])
    }

    /// A right drag on a free part turns it about its box centre, which stays where it is, in one
    /// undo step that undo takes back; the drag opens no menu.
    #[test]
    fn a_right_drag_turns_a_free_part_in_one_undo_step() {
        let (mut h, _, free) = two_parts();
        let centre = Vec3::new(25.0, 5.0, 5.0);
        let undo = h.app.session.undo.len();
        let a = screen(&mut h, Vec3::new(25.0, 5.0, 10.0));
        let b = a + egui::vec2(80.0, 30.0);
        press_and_move(&mut h, a, b);
        assert!(super::active(&h.app), "the right drag turns the part");
        release(&mut h, b);
        assert!(h.app.viewport.occurrence_rotate.is_none());
        assert!(h.app.viewport.context_menu.is_none(), "a right drag opens no menu");
        let (axes, at) = placed(&h, free, centre);
        assert!(turned_from_start(&axes), "{axes:?}");
        assert!(at.dist(centre) < 1e-6, "it turns about its centre: {at:?}");
        assert_eq!(h.app.session.undo.len(), undo + 1, "one undo step for the turn");
        run(&mut h, "edit.undo", json!({}));
        let (axes, _) = placed(&h, free, centre);
        assert!(!turned_from_start(&axes), "{axes:?}");
    }

    /// A right click on a free part still opens the menu and turns nothing.
    #[test]
    fn a_right_click_on_a_free_part_opens_the_menu() {
        let (mut h, _, free) = two_parts();
        let a = screen(&mut h, Vec3::new(25.0, 5.0, 10.0));
        h.app.synthetic.push(egui::Event::PointerMoved(a));
        right(&mut h, a, true);
        right(&mut h, a, false);
        play(&mut h);
        assert!(h.app.viewport.context_menu.is_some(), "a right click opens the menu");
        assert!(h.app.viewport.occurrence_rotate.is_none());
        assert!(!turned_from_start(&placed(&h, free, Vec3::ZERO).0));
    }

    /// A right drag on the grounded part turns nothing and adds no undo step.
    #[test]
    fn a_right_drag_on_a_grounded_part_turns_nothing() {
        let (mut h, base, _) = two_parts();
        let undo = h.app.session.undo.len();
        let a = screen(&mut h, Vec3::new(2.0, -2.0, 10.0));
        let b = a + egui::vec2(80.0, 30.0);
        press_and_move(&mut h, a, b);
        assert!(h.app.viewport.occurrence_rotate.is_none() && !super::active(&h.app), "the grounded part is not turned");
        release(&mut h, b);
        assert!(!turned_from_start(&placed(&h, base, Vec3::ZERO).0));
        assert_eq!(h.app.session.undo.len(), undo);
    }

    /// Escape during a turn puts the part back and leaves no undo step; the rest of the press
    /// turns nothing.
    #[test]
    fn escape_cancels_a_turn() {
        let (mut h, _, free) = two_parts();
        let undo = h.app.session.undo.len();
        let a = screen(&mut h, Vec3::new(25.0, 5.0, 10.0));
        let b = a + egui::vec2(80.0, 30.0);
        press_and_move(&mut h, a, b);
        assert!(turned_from_start(&placed(&h, free, Vec3::ZERO).0), "it turned while dragged");
        for pressed in [true, false] {
            h.app.synthetic.push(egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed, repeat: false, modifiers: Default::default() });
        }
        play(&mut h);
        assert!(!super::active(&h.app));
        assert!(!turned_from_start(&placed(&h, free, Vec3::ZERO).0), "Escape puts it back");
        h.app.synthetic.push(egui::Event::PointerMoved(b + egui::vec2(40.0, 0.0)));
        release(&mut h, b + egui::vec2(40.0, 0.0));
        assert!(!turned_from_start(&placed(&h, free, Vec3::ZERO).0), "the rest of the press turns nothing");
        assert_eq!(h.app.session.undo.len(), undo);
    }

    /// A block mated face to face on a grounded plate, seen from the front, only spins about the
    /// plate's normal when right-dragged across and up: its bottom stays on the plate.
    #[test]
    fn a_part_mated_to_a_plane_only_spins_about_its_normal() {
        let mut h = Harness::new();
        h.app.home.open = false;
        let mut occurrences = Vec::new();
        for (name, size, corner) in [("Plate", [100, 100, 10], [0, 0, 0]), ("Block", [20, 20, 20], [30, 40, 50])] {
            run(&mut h, "component.activate", json!({"component": "root"}));
            occurrences.push(run(&mut h, "component.create", json!({"name": name}))["occurrence"].as_u64().expect("occurrence"));
            run(&mut h, "solid.box", json!({"length": size[0], "width": size[1], "height": size[2], "corner": corner}));
        }
        run(&mut h, "component.activate", json!({"component": "root"}));
        let (plate, block) = (occurrences[0], occurrences[1]);
        run(&mut h, "occurrence.ground", json!({"occurrence": plate, "grounded": true}));
        run(
            &mut h,
            "assembly.constraint.create",
            json!({"type": "mate", "a": {"occurrence": "Plate", "face": [5, 5, 10]}, "b": {"occurrence": "Block", "face": [31, 41, 50]}}),
        );
        // The block's bottom (z 50 in its own coordinates) is on the plate's top (z 10).
        let seat = Vec3::new(40.0, 50.0, 50.0);
        assert!((placed(&h, block, seat).1.z - 10.0).abs() < 1e-6, "{:?}", placed(&h, block, seat).1);
        h.call("ui.view", json!({"view": "front"}));
        h.app.cam.distance = 300.0;
        h.frames(3);
        // The middle of the block's front face, then across and up.
        let a = screen(&mut h, Vec3::new(40.0, 40.0, 20.0));
        let b = a + egui::vec2(100.0, -60.0);
        let undo = h.app.session.undo.len();
        press_and_move(&mut h, a, b);
        assert!(super::active(&h.app), "the right drag turns the block");
        release(&mut h, b);
        let (axes, at) = placed(&h, block, seat);
        assert!(axes[2].dist(Vec3::Z) < 1e-6, "it does not tilt: {axes:?}");
        assert!(axes[0].dist(Vec3::X) > 0.1, "it spins about the normal: {axes:?}");
        assert!((at.z - 10.0).abs() < 1e-6, "its bottom stays on the plate: {at:?}");
        assert_eq!(h.app.session.undo.len(), undo + 1, "one undo step for the turn");
    }
}
