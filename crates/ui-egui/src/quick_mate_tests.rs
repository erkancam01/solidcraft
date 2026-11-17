use serde_json::json;
use solidcore_record::Vec3;
use solidcraft_engine::Session;
use solidcraft_engine::geom::prelude::*;

use super::*;
use crate::Services;

/// A grounded plate (100 x 100 x 10, top at z 10) and a free 20 mm block above it at (30, 40, 50).
fn plate_and_block() -> SolveApp {
    let mut s = Session::default();
    let mut run = |id: &str, p: Value| s.execute(id, &p).unwrap_or_else(|e| panic!("{id}: {e}"));
    for (name, size, corner) in [("Plate", [100, 100, 10], [0, 0, 0]), ("Block", [20, 20, 20], [30, 40, 50])] {
        run("component.activate", json!({"component": "root"}));
        run("component.create", json!({ "name": name }));
        run("solid.box", json!({"length": size[0], "width": size[1], "height": size[2], "corner": corner}));
    }
    run("component.activate", json!({"component": "root"}));
    run("occurrence.ground", json!({"occurrence": "Plate:1", "grounded": true}));
    SolveApp::new(s, Services::default())
}

/// A pick of the face a point of the exact geometry lies on (the nearest face of any body).
fn face(app: &SolveApp, p: Vec3) -> Sel {
    let st = app.session.world_state().unwrap();
    let mut best: Option<(f64, Sel)> = None;
    for b in &st.bodies {
        let m = b.mesh();
        for (t, f) in m.triangles.iter().zip(&m.tri_face) {
            let Some([a, bb, c]) = m.tri(t) else { continue };
            let d = crate::preview::point_triangle_dist(p, a, bb, c);
            if best.as_ref().is_none_or(|(bd, _)| d < *bd) {
                best = Some((d, Sel::Face { body: b.name.clone(), index: *f as usize, point: p }));
            }
        }
    }
    best.map(|x| x.1).unwrap()
}

const PLATE_TOP: Vec3 = Vec3::new(5.0, 5.0, 10.0);
const BLOCK_BOTTOM: Vec3 = Vec3::new(31.0, 41.0, 50.0);

/// The app with the plate's top and the block's bottom selected (in `order`), a few frames drawn.
fn selected(order: [Vec3; 2]) -> crate::scenario::Harness {
    let mut h = crate::scenario::Harness::new();
    h.app = plate_and_block();
    h.app.session.selection = order.iter().map(|p| face(&h.app, *p)).collect();
    h.frames(2);
    h.step(&json!({"call": "ui.view", "params": {"view": "iso"}})).unwrap();
    h.step(&json!({"call": "ui.view", "params": {"view": "fit"}})).unwrap();
    h.frames(3);
    h
}

/// The block's lowest z, from its world bounds.
fn block_bottom(app: &SolveApp) -> f64 {
    let st = app.session.world_state().unwrap();
    st.bodies
        .iter()
        .filter(|b| crate::dialogs_assembly::occurrence_of_body(&app.session, &b.name).is_some_and(|o| o != 0))
        .filter(|b| app.session.doc.occurrences.iter().any(|o| !o.grounded && Some(o.id) == crate::dialogs_assembly::occurrence_of_body(&app.session, &b.name)))
        .map(|b| b.mesh().bounds().min.z)
        .fold(f64::INFINITY, f64::min)
}

#[test]
fn two_faces_on_two_parts_offer_the_rules_and_a_click_mates_them() {
    let mut h = selected([PLATE_TOP, BLOCK_BOTTOM]);
    // Two planes: mate, parallel, perpendicular, distance and angle; nothing round.
    for rule in ["mate", "parallel", "perpendicular", "distance", "angle"] {
        assert!(crate::scenario::handle_at(&format!("quick_mate:{rule}")).is_some(), "{rule} is offered");
    }
    assert!(crate::scenario::handle_at("quick_mate:concentric").is_none(), "two planes are not concentric");
    let undo = h.app.session.undo.len();
    h.step(&json!({"click": {"handle": "quick_mate:mate"}})).unwrap();
    h.frames(2);
    let joints = &h.app.session.doc.assembly.joints;
    assert_eq!(joints.iter().filter_map(|j| j.constraint).collect::<Vec<_>>(), [Constraint::Mate]);
    assert_eq!(h.app.session.undo.len(), undo + 1, "one undo step");
    assert!((block_bottom(&h.app) - 10.0).abs() < 1e-6, "the block sits on the plate: {}", block_bottom(&h.app));
    assert!(h.app.quick_mate.added.is_some(), "the card now offers Flip");
}

#[test]
fn the_held_part_stays_whichever_is_picked_first() {
    // The block picked first, the grounded plate second: the block still moves.
    let mut h = selected([BLOCK_BOTTOM, PLATE_TOP]);
    h.step(&json!({"click": {"handle": "quick_mate:mate"}})).unwrap();
    h.frames(2);
    assert!(h.app.quick_mate.error.is_none(), "{:?}", h.app.quick_mate.error);
    assert!((block_bottom(&h.app) - 10.0).abs() < 1e-6, "{}", block_bottom(&h.app));
}

#[test]
fn distance_asks_for_its_value_first() {
    let mut h = selected([PLATE_TOP, BLOCK_BOTTOM]);
    h.step(&json!({"click": {"handle": "quick_mate:distance"}})).unwrap();
    h.frames(2);
    assert!(h.app.session.doc.assembly.joints.is_empty(), "nothing is added before the value");
    assert_eq!(h.app.quick_mate.asking, Some(Constraint::Distance));
    h.app.quick_mate.value = "25 mm".into();
    h.frames(1);
    h.step(&json!({"click": {"handle": "quick_mate:apply"}})).unwrap();
    h.frames(2);
    let j = h.app.session.doc.assembly.joints.iter().find(|j| j.constraint.is_some()).expect("a rule was added");
    assert_eq!(j.constraint, Some(Constraint::Distance));
    assert!((block_bottom(&h.app) - 35.0).abs() < 1e-6, "25 mm above the plate: {}", block_bottom(&h.app));
}

#[test]
fn flip_turns_the_added_rule_over() {
    let mut h = selected([PLATE_TOP, BLOCK_BOTTOM]);
    h.step(&json!({"click": {"handle": "quick_mate:mate"}})).unwrap();
    h.frames(2);
    let flip = |app: &SolveApp| app.session.doc.assembly.joints.iter().find(|j| j.constraint.is_some()).map(|j| j.flip);
    let before = flip(&h.app);
    h.step(&json!({"click": {"handle": "quick_mate:flip"}})).unwrap();
    h.frames(2);
    assert_ne!(flip(&h.app), before, "Flip turns the rule over");
}

#[test]
fn two_faces_of_one_part_or_a_command_show_no_card() {
    let h = selected([Vec3::new(31.0, 41.0, 50.0), Vec3::new(31.0, 41.0, 70.0)]);
    assert!(pair(&h.app).is_none(), "one part: nothing to mate");
    let mut h = selected([PLATE_TOP, BLOCK_BOTTOM]);
    assert!(pair(&h.app).is_some() && idle(&h.app));
    h.app.start("solid.box");
    assert!(!idle(&h.app), "a command running hides the card");
}

#[test]
fn a_reference_on_the_designs_own_bodies_is_the_held_side() {
    // A sash modelled in the design itself (root bodies) and a part beside it, picked part first.
    let mut s = Session::default();
    s.execute("solid.box", &json!({"length": 100, "width": 100, "height": 10})).unwrap();
    s.execute("component.create", &json!({"name": "Block"})).unwrap();
    s.execute("solid.box", &json!({"length": 20, "width": 20, "height": 20, "corner": [30, 40, 50]})).unwrap();
    s.execute("component.activate", &json!({"component": "root"})).unwrap();
    s.execute("occurrence.ground", &json!({"occurrence": "Block:1", "grounded": false})).unwrap();
    let mut app = SolveApp::new(s, Services::default());
    app.session.selection = vec![face(&app, BLOCK_BOTTOM), face(&app, PLATE_TOP)];
    let p = pair(&app).expect("two parts: the design and the block");
    let params = create_params(&app, Constraint::Mate, &p, "");
    assert_eq!(params["b"], p.params[0], "the block moves: {params}");
    app.run("assembly.constraint.create", params).unwrap();
    assert!((block_bottom(&app) - 10.0).abs() < 1e-6, "{}", block_bottom(&app));
}
