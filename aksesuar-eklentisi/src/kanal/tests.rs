#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use serde_json::{Value, json};

use super::*;
use crate::test_ana::{TestAna, altin};

// A sash bar "Kanat" in the design's root: its groove floor is the top face z = 0, from y = 0 to
// y = 62; the picked edge runs along x at y = 62 from x = 62 to x = 838. Beside it is the bar's
// side face y = 62 (larger). The part "Makas" lies elsewhere, its seat face facing down, its long
// edges along y.
const FLOOR: u64 = 1;
const SIDE: u64 = 4;
const SEAT: u64 = 2;

fn kanat_faces() -> Value {
    json!({"body": "Kanat", "faces": [
        {"index": 0, "area": 54320.0, "centroid": [450, 0, -35], "plane_normal": [0, -1, 0], "point": [450, 0, -35]},
        {"index": FLOOR, "area": 48112.0, "centroid": [450, 31, 0], "plane_normal": [0, 0, 1], "point": [450, 31, 0]},
        {"index": 3, "area": 48112.0, "centroid": [450, 31, -70], "plane_normal": [0, 0, -1], "point": [450, 31, -70]},
        {"index": SIDE, "area": 54320.0, "centroid": [450, 62, -35], "plane_normal": [0, 1, 0], "point": [450, 62, -35]},
        {"index": 5, "area": 100.0, "centroid": [450, 62, -35], "plane_normal": null, "point": null}
    ]})
}

fn makas_faces() -> Value {
    json!({"body": "Makas", "faces": [
        {"index": 0, "area": 3000.0, "centroid": [500, 500, 122], "plane_normal": [0, 0, 1], "point": [500, 500, 122]},
        {"index": SEAT, "area": 3000.0, "centroid": [500, 500, 100], "plane_normal": [0, 0, -1], "point": [501, 450, 100]},
        {"index": 7, "area": 300.0, "centroid": [500, 500, 110], "plane_normal": null}
    ]})
}

fn makas_edges() -> Value {
    json!({"body": "Makas", "edges": [
        {"index": 0, "name": "a|b", "mid": [492.5, 500, 100], "length": 200.0, "start": [492.5, 600, 100], "end": [492.5, 400, 100]},
        {"index": 1, "name": "a|c", "mid": [500, 400, 100], "length": 15.0, "start": [492.5, 400, 100], "end": [507.5, 400, 100]},
        {"index": 2, "name": "a|d", "mid": [492.5, 400, 111], "length": 22.0, "start": [492.5, 400, 100], "end": [492.5, 400, 122]},
        {"index": 3, "name": "arc", "mid": [0, 0, 100], "length": 400.0, "start": [500, 300, 100], "end": [500, 300.1, 100]},
        {"index": 4, "name": null, "mid": [0, 0, 0], "length": 0.0, "start": null, "end": null}
    ]})
}

fn seat_item(extra: Value) -> Value {
    let mut v = json!({"kind": "face", "type": "face", "body": "Makas", "index": SEAT, "point": [501, 450, 100], "planar": true});
    if let (Some(m), Some(e)) = (v.as_object_mut(), extra.as_object()) {
        m.extend(e.clone());
    }
    v
}

fn edge_item() -> Value {
    json!({"kind": "edge", "type": "edge", "body": "Kanat", "index": 9, "point": [400, 62, 0], "start": [62, 62, 0], "end": [838, 62, 0], "curve": "line", "length_mm": 776.0})
}

fn floor_item() -> Value {
    json!({"kind": "face", "type": "face", "body": "Kanat", "index": FLOOR, "point": [450, 31, 0]})
}

/// A design answering as SolidCraft would, with `items` selected.
fn design(items: Vec<Value>) -> TestAna {
    TestAna::new(move |komut, p| {
        Ok(match (komut, p.get("body").and_then(Value::as_str)) {
            ("parameters.list", _) => json!({"parameters": [{"name": "KOL"}, {"name": "FFH"}]}),
            ("selection.get", _) => json!({"count": items.len(), "items": items.clone()}),
            ("model.faces", Some("Kanat")) => kanat_faces(),
            ("model.faces", Some("Makas")) => makas_faces(),
            ("model.edges", Some("Makas")) => makas_edges(),
            ("component.from_bodies", _) => json!({"components": [{"component": 7, "body": "Makas", "feature": 30}]}),
            ("component.list", _) => json!({"active": 0, "pending_moves": [], "components": [
                {"id": 0, "name": "Pencere", "occurrences": []},
                {"id": 7, "name": "Makas", "occurrences": [{"id": 12, "name": "Makas:1", "parent": 0, "grounded": false}]}]}),
            ("joint.create", _) => json!({"joint": 40, "name": p["name"], "warnings": []}),
            other => panic!("unexpected {other:?} {p}"),
        })
    })
}

fn joint(ana: &TestAna) -> Value {
    ana.komutlar.iter().find(|(c, _)| c == "joint.create").map(|(_, p)| p.clone()).unwrap()
}

#[test]
fn a_part_on_a_root_body_gets_a_component_and_one_planar_joint() {
    let mut ana = design(vec![seat_item(json!({})), edge_item(), floor_item()]);
    let out = calistir(&mut ana, &json!({"mesafe": "KOL", "sondan": false, "cevir": false})).unwrap();
    assert_eq!(
        ana.adlar(),
        ["parameters.list", "selection.get", "model.faces", "model.faces", "model.edges", "component.from_bodies", "component.list", "joint.create"]
    );
    crate::tests::commands_are_granted("kanala-yerlestir", &ana.adlar());
    assert_eq!(ana.komutlar[5].1, json!({"bodies": ["Makas"]}));
    let j = joint(&ana);
    altin("kanal_eklem.json", &j);
    // The floor frame at the corner (62, 62, 0): z up, x along the floor's other edge (-y),
    // y along the picked edge (+x), so the distance is the second value and the part turns 90°.
    assert_eq!(j["type"], "planar");
    assert_eq!(j["a"], json!({"occurrence": 0, "edge": [450.0, 62.0, 0.0], "corner": [62.0, 62.0, 0.0], "on": [450.0, 31.0, 0.0]}));
    assert_eq!(j["b"], json!({"occurrence": 12, "face": [501.0, 450.0, 100.0], "x": [0.0, 1.0, 0.0]}));
    assert_eq!(j["values"], json!([31.0, "KOL", 90.0]));
    assert_eq!(j["limits"], json!([[31.0, 31.0], null, [90.0, 90.0]]));
    assert_eq!((j["flip"].clone(), j["angle"].clone(), j["offset"].clone()), (json!(false), json!(0), json!(0)));
    assert_eq!(out["eklem"], 40);
    assert!(out["mesaj"].as_str().unwrap().contains("kendi bileşenine"), "{out}");
}

#[test]
fn measuring_from_the_end_puts_the_edge_on_the_frames_x() {
    let mut ana = design(vec![floor_item(), edge_item(), seat_item(json!({"occurrence_path": [3], "component": {"id": 5, "name": "Makas"}}))]);
    calistir(&mut ana, &json!({"mesafe": "250,5", "sondan": true})).unwrap();
    assert!(!ana.adlar().contains(&"component.from_bodies"), "a part already in a component is used as it is");
    assert!(!ana.adlar().contains(&"parameters.list"), "a plain number needs no parameter check");
    let j = joint(&ana);
    assert_eq!(j["a"]["corner"], json!([838.0, 62.0, 0.0]));
    assert_eq!(j["b"]["occurrence"], 3);
    assert_eq!(j["values"], json!([250.5, 31.0, 0.0]));
    assert_eq!(j["limits"], json!([null, [31.0, 31.0], [0.0, 0.0]]));
}

#[test]
fn turning_over_adds_half_a_turn() {
    let mut ana = design(vec![seat_item(json!({"occurrence_path": [3]})), edge_item(), floor_item()]);
    calistir(&mut ana, &json!({"mesafe": "FFH/2 - 15 mm", "cevir": true})).unwrap();
    assert_eq!(joint(&ana)["values"], json!([31.0, "FFH/2 - 15 mm", -90.0]));
    let mut ana = design(vec![seat_item(json!({"occurrence_path": [3]})), edge_item(), floor_item()]);
    calistir(&mut ana, &json!({"mesafe": "0", "cevir": true, "sondan": true})).unwrap();
    assert_eq!(joint(&ana)["values"], json!([0.0, 31.0, 180.0]));
}

#[test]
fn without_a_floor_the_larger_face_beside_the_edge_is_used() {
    let mut ana = design(vec![seat_item(json!({"occurrence_path": [3]})), edge_item()]);
    calistir(&mut ana, &json!({"mesafe": "100"})).unwrap();
    let j = joint(&ana);
    // The side face y = 62: z = +y; the edge runs along its x; the middle is 35 mm down.
    assert_eq!(j["a"]["on"], json!([450.0, 62.0, -35.0]));
    assert_eq!(j["values"], json!([100.0, 35.0, 0.0]));
}

#[test]
fn the_floor_frame_case_matches_the_engines_corner_rule() {
    // joints_edge.rs: x = de when z · (de × df) >= 0, else df; df runs into the face.
    let k = |a: V3, b: V3| Kenar { oge: Oge { govde: "K".into(), indeks: 0, nokta: None, yerlesim: json!(0) }, bas: a, son: b };
    let floor = Yuz { normal: V3::new(0.0, 0.0, 1.0), merkez: V3::new(0.0, 0.0, 0.0), nokta: V3::new(0.0, 0.0, 0.0), alan: 1.0 };
    let seat = Yuz { normal: V3::new(0.0, 0.0, -1.0), merkez: V3::new(0.0, 0.0, 5.0), nokta: V3::new(0.0, 0.0, 5.0), alan: 1.0 };
    // A 20 x 10 floor centred on the origin; each of its four edges, from each end.
    let corners = [V3::new(-10.0, -5.0, 0.0), V3::new(10.0, -5.0, 0.0), V3::new(10.0, 5.0, 0.0), V3::new(-10.0, 5.0, 0.0)];
    for i in 0..4 {
        let (a, b) = (corners[i], corners[(i + 1) % 4]);
        for sondan in [false, true] {
            let plan = planla(&k(a, b), &floor, &seat, V3::new(1.0, 0.0, 0.0), sondan, false).unwrap();
            let (q, far) = if sondan { (b, a) } else { (a, b) };
            let de = far.eksi(q).birim().unwrap();
            // The corner's other edge: from q to its other neighbour.
            let other = corners.iter().copied().find(|c| *c != a && *c != b && (c.x == q.x || c.y == q.y)).unwrap();
            let df = other.eksi(q).birim().unwrap();
            let engine_x_is_edge = floor.normal.nokta(de.capraz(df)) >= 0.0;
            assert_eq!(plan.x_kenar_boyunca, engine_x_is_edge, "edge {i} sondan {sondan}");
            let half_width = other.eksi(q).boy() / 2.0;
            assert!((plan.yanal - half_width).abs() < 1e-9, "edge {i}: {} vs {half_width}", plan.yanal);
        }
    }
}

#[test]
fn the_selection_is_checked_with_turkish_reasons() {
    let why = |items: Vec<Value>, p: Value| match calistir(&mut design(items), &p) {
        Err(Hata::Girdi { neden, .. }) => neden,
        other => panic!("{other:?}"),
    };
    let p = json!({"mesafe": "10"});
    assert!(why(vec![], p.clone()).contains("Hiçbir şey seçili değil"));
    assert!(why(vec![seat_item(json!({}))], p.clone()).contains("kenar seçin"));
    assert!(why(vec![seat_item(json!({})), edge_item(), edge_item()], p.clone()).contains("seçili kenar sayısı: 2"));
    assert!(why(vec![edge_item(), floor_item()], p.clone()).contains("oturma yüzü seçili değil"));
    assert!(why(vec![seat_item(json!({})), seat_item(json!({})), edge_item()], p.clone()).contains("Fazla yüz"));
    let mut arc = edge_item();
    arc["curve"] = json!("arc");
    assert!(why(vec![seat_item(json!({})), arc], p.clone()).contains("düz değil"));
    let mut body = edge_item();
    body["kind"] = json!("body");
    assert!(why(vec![seat_item(json!({})), body], p.clone()).contains("yüz ve kenar dışında"));
    let mut curved = seat_item(json!({}));
    curved["index"] = json!(7);
    assert!(why(vec![curved, edge_item(), floor_item()], p.clone()).contains("düz (düzlemsel)"));
    let mut wrong_floor = floor_item();
    wrong_floor["index"] = json!(3);
    assert!(why(vec![seat_item(json!({})), edge_item(), wrong_floor], p.clone()).contains("kanal tabanı yüzünün üzerinde değil"));
    let same = json!({"occurrence_path": [4]});
    let mut e = edge_item();
    e["occurrence_path"] = json!([4]);
    let mut f = floor_item();
    f["occurrence_path"] = json!([4]);
    assert!(why(vec![seat_item(same), e, f], p).contains("aynı bileşende"));
}

#[test]
fn distances_are_checked_against_the_designs_parameters() {
    let why = |p: Value| match calistir(&mut design(vec![seat_item(json!({})), edge_item(), floor_item()]), &p) {
        Err(Hata::Girdi { alan, neden }) => {
            assert_eq!(alan, "mesafe");
            neden
        }
        other => panic!("{other:?}"),
    };
    assert!(why(json!({"mesafe": "KOLL"})).contains("`KOLL`"));
    assert!(why(json!({"mesafe": ""})).contains("boş"));
    assert!(why(json!({"mesafe": "KOL;"})).contains("`;`"));
    assert!(matches!(calistir(&mut design(vec![]), &json!({"mesafe": 5})), Err(Hata::Girdi { .. })));
    assert!(matches!(calistir(&mut design(vec![]), &json!({"mesafe": "5", "cevir": "evet"})), Err(Hata::Girdi { .. })));
}

#[test]
fn the_part_axis_is_its_longest_straight_edge_in_the_seat() {
    assert_eq!(uzun_eksen(&makas_edges(), V3::new(0.0, 0.0, -1.0)).unwrap(), V3::new(0.0, 1.0, 0.0));
    // Seated on a side face, the 200 mm edge runs along the normal: the 22 mm edge along z wins
    // over the 15 mm one, and the 400 mm arc is not straight.
    assert_eq!(uzun_eksen(&makas_edges(), V3::new(0.0, 1.0, 0.0)).unwrap(), V3::new(0.0, 0.0, 1.0));
    assert!(uzun_eksen(&json!({"edges": []}), V3::new(0.0, 0.0, 1.0)).is_err());
    assert!(uzun_eksen(&json!({}), V3::new(0.0, 0.0, 1.0)).is_err());
}

#[test]
fn hostile_replies_fail_without_panicking() {
    for reply in [json!(null), json!({"items": "x"}), json!({"items": [{"kind": "edge"}]}), json!({"items": [1, 2]})] {
        assert!(secimi_oku(&reply).is_err(), "{reply}");
    }
    assert!(yuz(&json!({"faces": [{"index": 1, "plane_normal": [0, 0, 0], "centroid": [0, 0, 0]}]}), 1, "x").is_err());
    assert!(
        tabani_bul(
            &json!({"faces": 3}),
            &Kenar {
                oge: Oge { govde: "K".into(), indeks: 0, nokta: None, yerlesim: json!(0) },
                bas: V3::new(0.0, 0.0, 0.0),
                son: V3::new(1.0, 0.0, 0.0)
            }
        )
        .is_err()
    );
}
