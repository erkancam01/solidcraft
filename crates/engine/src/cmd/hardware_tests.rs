use super::*;
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};

/// A library folder in the temporary folder, removed when dropped.
struct Library(PathBuf);
impl Library {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = solidcraft_io::vfs::temp_dir().join(format!("solidcraft-hardware-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Library(dir)
    }
    fn file(&self, relative: &str, contents: &str) -> String {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, contents).unwrap();
        path.to_string_lossy().into_owned()
    }
    fn folder(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
}
impl Drop for Library {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A library with hardware groups, a profile group, a part with a good card, one with a broken
/// card, a file directly in the folder, designs in and out of the profile group and files that are
/// not parts.
fn sample() -> Library {
    let lib = Library::new();
    lib.file("Köşe aktarma/YMB.00146 - TIRNAKLI KÖŞE AKTARMA.STEP", "ISO-10303-21;");
    lib.file(
        "Köşe aktarma/YMB.00146 - TIRNAKLI KÖŞE AKTARMA.json",
        r#"{"part_no": "YMB.00146", "description": "Corner drive", "connection": {"origin": [1, 2, 3], "z": [0, 0, 1], "x": [1, 0, 0]}, "supplier": "kept"}"#,
    );
    lib.file("Menteşe grubu/b-hinge.stp", "ISO-10303-21;");
    lib.file("Menteşe grubu/A-hinge.step", "ISO-10303-21;");
    lib.file("Menteşe grubu/A-hinge.json", "{ not json");
    lib.file("Menteşe grubu/notes.txt", "not a part");
    lib.file("Menteşe grubu/drawing.pdf", "%PDF");
    lib.file("Menteşe grubu/design.sc", "not read");
    lib.file("Menteşe grubu/stray.json", r#"{"part_no": "S"}"#);
    lib.file("Menteşe grubu/wing.sc", "not read");
    lib.file("Menteşe grubu/wing.json", r#"{"role": "profile"}"#);
    lib.file("Makas ve kol/.hidden.step", "ISO-10303-21;");
    lib.file("Profil/frame-70.DXF", "0\nEOF\n");
    lib.file("Profil/sash-70.dxf", "0\nEOF\n");
    lib.file("Profil/sash-70.json", r#"{"role": "hardware"}"#);
    lib.file("Profil/legacy.json", r#"{"features": []}"#);
    lib.file("Profil/sill.sc", "not read");
    lib.file("loose.stp", "ISO-10303-21;");
    lib.file("readme.md", "# library");
    std::fs::create_dir_all(lib.0.join("Kilit ispanyolet tij")).unwrap();
    lib
}

fn list(folder: &str) -> crate::Result<Value> {
    Session::default().execute("hardware.library.list", &json!({ "folder": folder }))
}

#[test]
fn lists_groups_and_parts_with_their_cards() {
    let lib = sample();
    let r = list(&lib.folder()).unwrap();
    assert_eq!(r["truncated"], false, "{r}");
    let groups: Vec<(String, u64)> =
        r["groups"].as_array().unwrap().iter().map(|g| (g["name"].as_str().unwrap().to_string(), g["parts"].as_u64().unwrap())).collect();
    assert_eq!(
        groups,
        [
            ("".to_string(), 1),
            ("Kilit ispanyolet tij".to_string(), 0),
            ("Köşe aktarma".to_string(), 1),
            ("Makas ve kol".to_string(), 0),
            ("Menteşe grubu".to_string(), 3),
            ("Profil".to_string(), 4)
        ],
        "sorted by name, the ungrouped files first; an empty group is listed with no parts"
    );
    let parts = r["parts"].as_array().unwrap();
    let names: Vec<&str> = parts.iter().map(|p| p["file"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        [
            "loose.stp",
            "YMB.00146 - TIRNAKLI KÖŞE AKTARMA.STEP",
            "A-hinge.step",
            "b-hinge.stp",
            "wing.sc",
            "frame-70.DXF",
            "legacy.json",
            "sash-70.dxf",
            "sill.sc"
        ],
        "STEP and DXF files in any case, designs only as profiles, by group then name (case-insensitive); hidden files, cards and other files skipped"
    );
    let corner = &parts[1];
    assert_eq!(corner["name"], "YMB.00146 - TIRNAKLI KÖŞE AKTARMA");
    assert_eq!(corner["group"], "Köşe aktarma");
    assert_eq!((corner["kind"].as_str(), corner["role"].as_str()), (Some("step"), Some("hardware")));
    assert_eq!(corner["card"]["part_no"], "YMB.00146");
    assert_eq!(corner["card"]["description"], "Corner drive");
    assert_eq!(corner["card"]["connection"], json!({"origin": [1.0, 2.0, 3.0], "z": [0.0, 0.0, 1.0], "x": [1.0, 0.0, 0.0]}));
    assert!(corner.get("card_error").is_none(), "{corner}");
    assert!(corner["card_path"].as_str().unwrap().ends_with("YMB.00146 - TIRNAKLI KÖŞE AKTARMA.json"), "{corner}");
    assert!(std::path::Path::new(corner["path"].as_str().unwrap()).is_file());

    let broken = &parts[2];
    assert_eq!(broken["card"], Value::Null, "{broken}");
    assert!(broken["card_error"].as_str().is_some_and(|e| e.contains("not JSON")), "a broken card is reported on its part: {broken}");
    let plain = &parts[3];
    assert_eq!((&plain["card"], plain.get("card_error")), (&Value::Null, None), "no card: {plain}");

    let wing = &parts[4];
    assert_eq!((wing["kind"].as_str(), wing["role"].as_str()), (Some("sc"), Some("profile")), "a design whose card says profile: {wing}");
    let (frame, legacy, sash, sill) = (&parts[5], &parts[6], &parts[7], &parts[8]);
    assert_eq!((frame["kind"].as_str(), frame["role"].as_str()), (Some("dxf"), Some("profile")), "a DXF in the Profil folder: {frame}");
    assert_eq!((legacy["kind"].as_str(), legacy["role"].as_str()), (Some("sc"), Some("profile")), "a .json design: {legacy}");
    assert_eq!((&legacy["card"], &legacy["card_path"]), (&Value::Null, &Value::Null), "a .json design has no card: {legacy}");
    assert_eq!(sash["role"], "hardware", "the card's role wins over its folder's: {sash}");
    assert_eq!((sill["kind"].as_str(), sill["role"].as_str()), (Some("sc"), Some("profile")), "{sill}");
    assert_eq!(parts[0]["group"], "");
}

#[test]
fn a_card_that_breaks_its_rules_is_an_error_on_its_part_only() {
    let lib = Library::new();
    let long = "x".repeat(300);
    for (name, card) in [
        ("long", json!({ "part_no": long }).to_string()),
        ("control", json!({"description": "a\u{7}b"}).to_string()),
        ("parallel", json!({"connection": {"origin": [0, 0, 0], "z": [0, 0, 1], "x": [0, 0, 2]}}).to_string()),
        ("zero", json!({"connection": {"origin": [0, 0, 0], "z": [0, 0, 0], "x": [1, 0, 0]}}).to_string()),
        ("huge", json!({"connection": {"origin": [1e7, 0, 0], "z": [0, 0, 1], "x": [1, 0, 0]}}).to_string()),
        ("array", "[1, 2]".to_string()),
        ("role", json!({"role": "door"}).to_string()),
    ] {
        lib.file(&format!("g/{name}.step"), "ISO-10303-21;");
        lib.file(&format!("g/{name}.json"), &card);
    }
    lib.file("g/big.step", "ISO-10303-21;");
    lib.file("g/big.json", &format!("{{\"description\": \"{}\"}}", "y".repeat(MAX_CARD_BYTES)));
    let r = list(&lib.folder()).unwrap();
    let parts = r["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 8, "{r}");
    for part in parts {
        assert_eq!(part["card"], Value::Null, "{part}");
        assert!(part["card_error"].as_str().is_some_and(|e| !e.is_empty()), "{part}");
    }
}

#[test]
fn a_folder_that_is_not_there_or_not_a_folder_is_refused() {
    let lib = sample();
    let file = lib.file("loose.stp", "ISO-10303-21;");
    for folder in [lib.0.join("missing").to_string_lossy().into_owned(), file, String::new(), "a\u{0}b".into()] {
        let error = list(&folder).unwrap_err().to_string();
        assert!(error.contains("hardware.library.list"), "{folder}: {error}");
    }
    assert!(Session::default().execute("hardware.library.list", &json!({})).is_err());
    assert!(Session::default().execute("hardware.library.list", &json!({"folder": lib.folder(), "extra": 1})).is_err());
}

#[test]
fn lists_stay_within_their_bounds() {
    let lib = Library::new();
    for i in 0..(MAX_GROUPS + 3) {
        std::fs::create_dir_all(lib.0.join(format!("group {i:03}"))).unwrap();
    }
    let r = list(&lib.folder()).unwrap();
    assert_eq!(r["groups"].as_array().unwrap().len(), MAX_GROUPS);
    assert_eq!(r["truncated"], true);

    let lib = Library::new();
    std::fs::create_dir_all(lib.0.join("many")).unwrap();
    for i in 0..(MAX_PARTS + 5) {
        std::fs::write(lib.0.join(format!("many/part {i:04}.stp")), "").unwrap();
    }
    let r = list(&lib.folder()).unwrap();
    assert_eq!(r["parts"].as_array().unwrap().len(), MAX_PARTS);
    assert_eq!(r["groups"][0]["parts"], MAX_PARTS);
    assert_eq!(r["truncated"], true);
}

#[test]
fn a_card_is_written_merged_and_read_back() {
    let lib = sample();
    let mut s = Session::default();
    let undo = s.undo.len();
    let part = lib.0.join("Menteşe grubu/b-hinge.stp").to_string_lossy().into_owned();
    let r = s
        .execute(
            "hardware.card.set",
            &json!({"path": part, "part_no": " HG-1 ", "description": "Hinge", "connection": {"origin": [0, 0, 5], "z": [0, 0, 2], "x": [1, 0, 1]}}),
        )
        .unwrap();
    assert_eq!(r["card"]["part_no"], "HG-1", "trimmed: {r}");
    assert_eq!(r["card"]["connection"]["z"], json!([0.0, 0.0, 1.0]), "z is saved as a unit vector: {r}");
    let x: Vec<f64> = r["card"]["connection"]["x"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    assert!((x[0] - 1.0).abs() < 1e-12 && x[1].abs() < 1e-12 && x[2].abs() < 1e-12, "x is square to z: {x:?}");
    let card_path = r["card_path"].as_str().unwrap().to_string();
    assert!(card_path.ends_with("b-hinge.json"), "{card_path}");
    let listed = list(&lib.folder()).unwrap();
    let hinge = listed["parts"].as_array().unwrap().iter().find(|p| p["file"] == "b-hinge.stp").unwrap().clone();
    assert_eq!(hinge["card"], r["card"], "the list reads what was written");

    // A key left out keeps its value; null and "" remove one; keys the card doesn't know stay.
    let corner = lib.0.join("Köşe aktarma/YMB.00146 - TIRNAKLI KÖŞE AKTARMA.STEP").to_string_lossy().into_owned();
    let r = s.execute("hardware.card.set", &json!({"path": corner, "description": "", "role": "hardware", "connection": null})).unwrap();
    assert_eq!(r["card"], json!({"part_no": "YMB.00146", "role": "hardware"}), "{r}");
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(r["card_path"].as_str().unwrap()).unwrap()).unwrap();
    assert_eq!(saved["supplier"], "kept", "{saved}");
    assert!(saved.get("description").is_none() && saved.get("connection").is_none(), "{saved}");
    assert_eq!(s.undo.len(), undo, "writing a card is no change to the design");
    assert!(!s.is_dirty());
}

#[test]
fn a_card_set_with_bad_values_writes_nothing() {
    let lib = sample();
    let mut s = Session::default();
    let part = lib.0.join("Menteşe grubu/b-hinge.stp").to_string_lossy().into_owned();
    let card = lib.0.join("Menteşe grubu/b-hinge.json");
    for p in [
        json!({"path": part, "part_no": "x".repeat(300)}),
        json!({"path": part, "description": "a\nb"}),
        json!({"path": part, "role": "door"}),
        json!({"path": part, "connection": {"origin": [0, 0, 0], "z": [0, 0, 1], "x": [0, 0, -3]}}),
        json!({"path": part, "connection": {"origin": [0, 0, 0], "z": [0, 0, 0], "x": [1, 0, 0]}}),
        json!({"path": part, "connection": {"origin": [0, 0, 1e6], "z": [0, 0, 1], "x": [1, 0, 0]}}),
        json!({"path": part, "connection": {"origin": [0, 0], "z": [0, 0, 1], "x": [1, 0, 0]}}),
        json!({"path": part, "colour": "red"}),
        json!({"path": lib.0.join("Menteşe grubu/notes.txt").to_string_lossy(), "part_no": "N"}),
        json!({"path": lib.0.join("Profil/legacy.json").to_string_lossy(), "part_no": "N"}),
        json!({"path": lib.0.join("Menteşe grubu/missing.step").to_string_lossy(), "part_no": "N"}),
        json!({"path": "", "part_no": "N"}),
    ] {
        assert!(s.execute("hardware.card.set", &p).is_err(), "{p}");
        assert!(!card.exists(), "{p}: nothing written");
    }
    // A card that is there but broken is not replaced.
    let broken = lib.0.join("Menteşe grubu/A-hinge.step").to_string_lossy().into_owned();
    let error = s.execute("hardware.card.set", &json!({"path": broken, "part_no": "A"})).unwrap_err().to_string();
    assert!(error.contains("not a JSON object"), "{error}");
    assert_eq!(std::fs::read_to_string(lib.0.join("Menteşe grubu/A-hinge.json")).unwrap(), "{ not json");
}

#[test]
fn the_library_obeys_the_agent_folder_scope() {
    let lib = sample();
    let outside = Library::new();
    let part = outside.file("p.step", "ISO-10303-21;");
    solidcraft_io::vfs::scoped(Some(solidcraft_io::vfs::PathScope::new([lib.0.clone()])), || {
        assert!(list(&lib.folder()).is_ok());
        let error = list(&outside.folder()).unwrap_err().to_string();
        assert!(error.contains("outside the folders this agent may use"), "{error}");
        let error = Session::default().execute("hardware.card.set", &json!({"path": part, "part_no": "P"})).unwrap_err().to_string();
        assert!(error.contains("outside the folders this agent may use"), "{error}");
    });
    assert!(!outside.0.join("p.json").exists());
}

#[test]
fn group_names_and_extensions_decide_the_role_and_kind() {
    assert_eq!(group_role("Profil"), HardwareRole::Profile);
    assert_eq!(group_role("PROFILE"), HardwareRole::Profile);
    assert_eq!(group_role("Menteşe grubu"), HardwareRole::Hardware);
    assert_eq!(group_role(""), HardwareRole::Hardware);
    assert_eq!(kind_of(Path::new("a.StP")), Some(HardwareKind::Step));
    assert_eq!(kind_of(Path::new("a.dxf")), Some(HardwareKind::Dxf));
    assert_eq!(kind_of(Path::new("a.SC")), Some(HardwareKind::Sc));
    assert_eq!(kind_of(Path::new("a.json")), None);
    assert_eq!(kind_of(Path::new("step")), None);
}

/// A folder typed or saved with `/` on Windows lists its parts, and writes their cards, with `\`
/// only: the paths read one way.
#[cfg(windows)]
#[test]
fn a_folder_typed_with_slashes_gives_paths_with_one_separator() {
    let lib = sample();
    let typed = lib.folder().replace('\\', "/");
    let listed = list(&typed).unwrap();
    assert!(!listed["folder"].as_str().unwrap().contains('/'), "{listed}");
    let parts = listed["parts"].as_array().unwrap();
    assert!(!parts.is_empty());
    for part in parts {
        for key in ["path", "card_path"] {
            let path = part[key].as_str().unwrap_or_default();
            assert!(!path.contains('/'), "{key}: {path}");
        }
    }
    let part = format!("{typed}/Menteşe grubu/b-hinge.stp");
    let r = Session::default().execute("hardware.card.set", &json!({"path": part, "part_no": "HG"})).unwrap();
    for key in ["path", "card_path"] {
        assert!(!r[key].as_str().unwrap().contains('/'), "{r}");
    }
    assert_eq!(native("C:/Lib/Profil/a.dxf"), r"C:\Lib\Profil\a.dxf");
}

// ---- Placement slots ----

fn v3(v: &Value) -> Vec3 {
    let a: Vec<f64> = v.as_array().unwrap().iter().map(|n| n.as_f64().unwrap()).collect();
    Vec3::new(a[0], a[1], a[2])
}

fn near(a: Vec3, b: Vec3) -> bool {
    a.dist(b) < 1e-6
}

/// A sash's right profile as the Çift Açılım template builds it: x FFB - 62 … FFB, y 0 … 70, z 0 …
/// FFH, its outer face (where the hardware sits) at x = FFB.
fn sash() -> Session {
    let mut s = Session::default();
    for (name, expression) in [("FFH", "1200 mm"), ("FFB", "900 mm"), ("KOL", "600 mm")] {
        s.execute("parameters.add", &json!({"name": name, "expression": expression})).unwrap();
    }
    s.execute(
        "solid.box",
        &json!({"length": "62 mm", "width": "70 mm", "height": "FFH", "center": ["FFB - 31 mm", "35 mm", "FFH/2"],
            "name": "Kanat sag", "body_name": "Kanat sag", "operation": "new"}),
    )
    .unwrap();
    s
}

fn slots(s: &mut Session) -> Vec<Value> {
    s.execute("hardware.slot.list", &json!({})).unwrap()["slots"].as_array().unwrap().clone()
}

#[test]
fn a_slot_places_along_and_across_the_corner_frame() {
    // The right profile's outer face at its bottom corner: z out of the face (+X), the frame's x
    // along the face's other edge (+Y) and y up the picked edge (+Z), so along is the second value.
    let corner = Corner { origin: Vec3::new(900.0, 0.0, 0.0), x: Vec3::Y, y: Vec3::Z, z: Vec3::X, along: Vec3::Z, across: Vec3::Y };
    let up = corner.axes();
    assert_eq!(up, Axes { along: [0.0, 1.0], across: [1.0, 0.0], turn: 90.0 });
    let p = place(&corner, 600.0, 35.0, 0);
    assert_eq!(p.values, [35.0, 600.0, 90.0], "across, along, a quarter turn so the part runs up the edge");
    assert!(near(p.origin, Vec3::new(900.0, 35.0, 600.0)) && near(p.x, Vec3::Z) && near(p.y, -Vec3::Y) && near(p.z, Vec3::X), "{p:?}");
    let p = place(&corner, 600.0, 35.0, 180);
    assert_eq!(p.values[2], -90.0);
    assert!(near(p.x, -Vec3::Z), "turned back down the edge: {p:?}");
    // The top corner: x runs down the picked edge, so along is the first value and no turn.
    let corner = Corner { origin: Vec3::new(900.0, 0.0, 1200.0), x: -Vec3::Z, y: Vec3::Y, z: Vec3::X, along: -Vec3::Z, across: Vec3::Y };
    let down = corner.axes();
    assert_eq!(down, Axes { along: [1.0, 0.0], across: [0.0, 1.0], turn: 0.0 });
    let p = place(&corner, 100.0, 35.0, 0);
    assert_eq!(p.values, [100.0, 35.0, 0.0]);
    assert!(near(p.origin, Vec3::new(900.0, 35.0, 1100.0)) && near(p.x, -Vec3::Z) && near(p.y, Vec3::Y), "{p:?}");
    let p = place(&corner, 0.0, 35.0, 270);
    assert_eq!(p.values[2], -90.0);
    assert!(near(p.x, -Vec3::Y) && near(p.y, -Vec3::Z), "{p:?}");
    assert_eq!(turn_degrees(90.0, 270), 0.0);
    assert_eq!(turn_degrees(0.0, 90), 90.0);
    assert_eq!(turn_degrees(90.0, 180), -90.0);
    assert_eq!(turn_degrees(45.0, 270), -45.0);

    let (values, limits) = joint_values(&up, "KOL", 600.0, &Across::Held(35.0), 90.0);
    assert_eq!(values, [json!(35.0), json!("KOL"), json!(90.0)]);
    assert_eq!(limits, [json!([35.0, 35.0]), Value::Null, json!([90.0, 90.0])]);
    let (values, limits) = joint_values(&down, "0", 0.0, &Across::Expression("W / 2".into()), 0.0);
    assert_eq!(values, [json!("0"), json!("W / 2"), json!(0.0)]);
    assert_eq!(limits, [Value::Null, Value::Null, json!([0.0, 0.0])], "an expression across drives it, unheld");

    assert_eq!(slug("Sağ üst köşe"), "sag-ust-kose");
    assert_eq!(slug("Ek kilit 1"), "ek-kilit-1");
    assert_eq!(slug("İç  KÖŞE!"), "ic-kose");
    assert_eq!(slug("…"), "slot");

    // A mitred corner (45°): the frame's x runs along the corner's other edge, so each shift
    // takes a share of along and of across, as an expression while one names a parameter.
    let h = std::f64::consts::FRAC_1_SQRT_2;
    let mitre = Corner { origin: Vec3::ZERO, x: Vec3::X, y: Vec3::Y, z: Vec3::Z, along: Vec3::new(h, h, 0.0), across: Vec3::new(h, -h, 0.0) };
    let axes = mitre.axes();
    assert!((axes.turn - 45.0).abs() < 1e-9, "{axes:?}");
    let (values, limits) = joint_values(&axes, "KOL", 50.0, &Across::Held(10.0), axes.turn);
    assert_eq!(values[..2], [json!("0.7071067812*(KOL) + 0.7071067812*(10 mm)"), json!("0.7071067812*(KOL) - 0.7071067812*(10 mm)")]);
    assert_eq!(limits[..2], [Value::Null, Value::Null], "the expressions drive them");
    let (values, limits) = joint_values(&axes, "50 mm", 50.0, &Across::Held(10.0), axes.turn);
    assert_eq!(values[..2], [json!(42.426407), json!(28.284271)], "numbers when neither names a parameter");
    assert_eq!(limits[..2], [json!([42.426407, 42.426407]), json!([28.284271, 28.284271])]);
    let p = place(&mitre, 50.0, 10.0, 0);
    assert!(near(p.origin, Vec3::new(60.0 * h, 40.0 * h, 0.0)) && near(p.x, mitre.along), "{p:?}");
}

/// A mitred profile's floor: a parallelogram 200 mm along +X, its other side `side` mm long
/// leaning `angle` deg from +X at the corner at the origin, 20 mm thick up Z; `KOL` 50 mm.
fn leaning(angle: f64, side: f64) -> (Session, Vec3) {
    let mut s = Session::default();
    s.execute("parameters.add", &json!({"name": "KOL", "expression": "50 mm"})).unwrap();
    let (sin, cos) = angle.to_radians().sin_cos();
    let lean = Vec3::new(side * cos, side * sin, 0.0);
    let profile = [[0.0, 0.0], [200.0, 0.0], [200.0 + lean.x, lean.y], [lean.x, lean.y]];
    for (command, params) in [
        ("sketch.create", json!({"plane": {"origin": [0, 0, 0], "x_dir": [1, 0, 0], "y_dir": [0, 1, 0]}, "name": "Profile"})),
        ("sketch.line", json!({"points": profile, "closed": true})),
        ("sketch.finish", json!({})),
        ("solid.extrude", json!({"distance": 20, "operation": "new", "body_name": "Profil"})),
    ] {
        s.execute(command, &params).unwrap_or_else(|e| panic!("{command}: {e}"));
    }
    (s, lean)
}

/// A slot at a corner that is not square (a mitred profile's 45°, and 89°) goes along the edge it
/// was picked on and across the floor square to that edge, its middle too, and the part placed
/// there follows a parameter along that edge.
#[test]
fn a_slot_at_a_corner_that_is_not_square_runs_along_its_picked_edge() {
    let lib = Library::new();
    let path = part(&lib, "washer", true);
    for angle in [45.0, 89.0] {
        let (mut s, lean) = leaning(angle, 100.0);
        let top = |v: Vec3| [v.x, v.y, 20.0];
        let corner = Vec3::new(0.0, 0.0, 20.0);
        let on = top(Vec3::new(100.0, 0.0, 0.0) + lean * 0.5);
        let de = Vec3::new(lean.x, lean.y, 0.0) / 100.0;
        // Square to the leaning edge, towards the face (the +X edge's side).
        let dn = Vec3::new(de.y, -de.x, 0.0);
        let inside = |p: Vec3| {
            let corners = [Vec3::ZERO, Vec3::new(200.0, 0.0, 0.0), Vec3::new(200.0, 0.0, 0.0) + lean, lean];
            (0..4).all(|i| (corners[(i + 1) % 4] - corners[i]).cross(p - corners[i]).z > 0.0) && (p.z - 20.0).abs() < 1e-6
        };
        let slot = |label: &str, at: Vec3, edge: Vec3, across: &str| {
            json!({"label": label, "group": "Makas ve kol", "edge": top(edge), "corner": top(at), "on": on, "along": "KOL", "across": across})
        };
        // The leaning edge from the corner at the origin, whose frame's x is the +X edge.
        let r = s.execute("hardware.slot.add", &slot("Kilit", corner, lean * 0.5, "10 mm")).unwrap();
        let (o, x, z) = slot_frame(&r);
        let d = o - corner;
        assert!((d.dot(de) - 50.0).abs() < 1e-6 && (d.dot(dn) - 10.0).abs() < 1e-6, "{angle}°: along the picked edge, across square to it: {r}");
        assert!(inside(o) && near(x, de) && near(z, Vec3::Z), "{angle}°: {r}");
        assert!(r["values"][0].as_str().is_some_and(|v| v.contains("KOL")) && r["values"][1].as_str().is_some_and(|v| v.contains("KOL")), "{r}");
        // The middle across the picked edge: half the face's width square to that edge.
        let r = s.execute("hardware.slot.add", &slot("Orta", corner, lean * 0.5, "middle")).unwrap();
        let (o, ..) = slot_frame(&r);
        let width = 200.0 * angle.to_radians().sin();
        assert!(((o - corner).dot(dn) - width / 2.0).abs() < 1e-6 && ((o - corner).dot(de) - 50.0).abs() < 1e-6, "{angle}°: {r}");
        // The long +X edge from the far corner (a mitred profile's side), whose frame's x is the
        // leaning edge there: along it back towards the origin, the middle half the face's width
        // square to it.
        let far = Vec3::new(200.0, 0.0, 20.0);
        let r = s.execute("hardware.slot.add", &slot("Yan", far, Vec3::new(100.0, 0.0, 0.0), "middle")).unwrap();
        let (o, x, _) = slot_frame(&r);
        assert!(near(o, Vec3::new(150.0, lean.y / 2.0, 20.0)) && near(x, -Vec3::X) && inside(o), "{angle}°: {r}");
        assert!(r["values"][0].as_str().is_some_and(|v| v.contains("KOL")), "{r}");
        // From the origin along +X, the frame's own x: as at a square corner.
        let r = s.execute("hardware.slot.add", &slot("Alt", corner, Vec3::new(100.0, 0.0, 0.0), "middle")).unwrap();
        let (o, x, _) = slot_frame(&r);
        assert!(near(o, Vec3::new(50.0, lean.y / 2.0, 20.0)) && near(x, Vec3::X), "{angle}°: {r}");
        assert_eq!(r["values"][0], "KOL", "{r}");

        // A part placed on the leaning slot sits on it and follows KOL along the edge.
        let placed = s.execute("hardware.place", &json!({"slot": "kilit", "path": path})).unwrap();
        assert!(placed.get("warnings").is_none(), "{placed}");
        let occurrence = placed["occurrence"].as_u64().unwrap();
        let (o, x, z) = connection_now(&s, occurrence);
        assert!(near(o, corner + de * 50.0 + dn * 10.0) && near(x, de) && near(z, -Vec3::Z), "{angle}°: {o:?} {x:?} {z:?}");
        s.execute("parameters.change", &json!({"changes": {"KOL": "70 mm"}})).unwrap();
        let (o, x, _) = connection_now(&s, occurrence);
        assert!(near(o, corner + de * 70.0 + dn * 10.0) && near(x, de), "{angle}°: {o:?} {x:?}");
        let listed = slots(&mut s);
        let (o, ..) = slot_frame(&listed[0]);
        assert!(near(o, corner + de * 70.0 + dn * 10.0) && inside(o), "{angle}°: {:?}", listed[0]);
    }
}

#[test]
fn slots_are_kept_in_the_design_and_follow_its_parameters() {
    let mut s = sash();
    let undo = s.undo.len();
    let p = json!({"label": "Kol", "group": "Makas ve kol", "edge": [900, 0, 600], "corner": [900, 0, 0], "on": [900, 35, 600], "along": "KOL"});
    let mut preview = p.clone();
    preview["preview"] = json!(true);
    let r = s.execute("hardware.slot.add", &preview).unwrap();
    assert_eq!((r["preview"].as_bool(), s.undo.len()), (Some(true), undo), "a preview changes nothing: {r}");
    assert!(s.doc.metadata.get(SLOTS_KEY).is_none());

    let r = s.execute("hardware.slot.add", &p).unwrap();
    assert_eq!(s.undo.len(), undo + 1, "one undo step");
    assert_eq!(r["slot"]["id"], "kol");
    assert_eq!((r["slot"]["along"].as_str(), r["slot"]["across"].as_str(), r["slot"]["angle"].as_u64()), (Some("KOL"), Some("middle"), Some(0)));
    assert!(near(v3(&r["frame"]["origin"]), Vec3::new(900.0, 35.0, 600.0)), "{r}");
    assert!(near(v3(&r["frame"]["z"]), Vec3::X) && near(v3(&r["frame"]["x"]), Vec3::Z), "{r}");
    assert_eq!(r["values"], json!([35.0, "KOL", 90.0]), "{r}");
    assert_eq!(r["limits"], json!([[35.0, 35.0], null, [90.0, 90.0]]), "{r}");
    assert!(near(v3(&r["joint_a"]["corner"]), Vec3::new(900.0, 0.0, 0.0)) && r["joint_a"]["occurrence"] == 0, "{r}");
    let stored = s.doc.metadata.get(SLOTS_KEY).unwrap().clone();
    assert_eq!((stored["version"].as_u64(), &stored["placed"]), (Some(1), &json!([])), "{stored}");
    assert!(stored["slots"][0]["origin"]["snap"].is_object(), "the origin is kept with its names: {stored}");

    let r = s
        .execute(
            "hardware.slot.add",
            &json!({"label": "Sağ üst köşe", "group": "Köşe aktarma", "edge": [900, 0, 600], "corner": [900, 0, 1200], "on": [900, 35, 600], "when": "FFH > 1200 mm"}),
        )
        .unwrap();
    assert_eq!((r["slot"]["id"].as_str(), r["active"].as_bool()), (Some("sag-ust-kose"), Some(false)), "{r}");
    assert_eq!(r["values"], json!(["0", 35.0, 0.0]), "x runs down the edge from the top corner: {r}");
    assert!(near(v3(&r["frame"]["origin"]), Vec3::new(900.0, 35.0, 1200.0)) && near(v3(&r["frame"]["x"]), -Vec3::Z), "{r}");

    // The slots follow the parameters: the face moves and grows, found again by name.
    s.execute("parameters.change", &json!({"changes": {"FFH": "1500 mm", "FFB": "1000 mm", "KOL": "700 mm"}})).unwrap();
    let listed = slots(&mut s);
    assert_eq!(listed.len(), 2);
    let (kol, top) = (&listed[0], &listed[1]);
    assert!(kol.get("error").is_none() && kol["active"] == true, "{kol}");
    assert!(near(v3(&kol["frame"]["origin"]), Vec3::new(1000.0, 35.0, 700.0)), "{kol}");
    assert!(near(v3(&top["frame"]["origin"]), Vec3::new(1000.0, 35.0, 1500.0)) && top["active"] == true, "{top}");
    assert!(near(v3(&top["joint_a"]["corner"]), Vec3::new(1000.0, 0.0, 1500.0)), "{top}");
    assert!(near(v3(&top["joint_a"]["edge"]), Vec3::new(1000.0, 0.0, 750.0)), "the edge's middle: {top}");
    // joint_a is an origin joint.create reads back to the same corner, by the same names.
    let back = super::super::joints::origin_param(&s, Some(&top["joint_a"]), "test").unwrap();
    let kept = serde_json::from_value::<HardwareSlot>(top.clone()).unwrap().origin;
    let names = |o: &JointOrigin| match &o.snap {
        Snap::Edge { edge, face, corner, .. } => (edge.clone(), face.clone(), corner.clone()),
        _ => (None, None, None),
    };
    assert_eq!(names(&back), names(&kept), "the same names");
    assert_eq!(s.execute("hardware.slot.list", &json!({"group": "Köşe aktarma"})).unwrap()["slots"].as_array().unwrap().len(), 1);

    // Edit and delete: one undo step each.
    let undo = s.undo.len();
    let r = s.execute("hardware.slot.edit", &json!({"id": "kol", "along": "KOL - 50 mm", "angle": 180, "flip": true, "label": "Kol rozet"})).unwrap();
    assert_eq!(s.undo.len(), undo + 1);
    assert_eq!((r["slot"]["label"].as_str(), r["slot"]["flip"].as_bool()), (Some("Kol rozet"), Some(true)));
    assert!(near(v3(&r["frame"]["origin"]), Vec3::new(1000.0, 35.0, 650.0)) && near(v3(&r["frame"]["x"]), -Vec3::Z), "{r}");
    assert_eq!(r["values"], json!([35.0, "KOL - 50 mm", -90.0]), "{r}");
    let r = s.execute("hardware.slot.edit", &json!({"id": "sag-ust-kose", "when": null})).unwrap();
    assert_eq!((r["slot"]["when"].clone(), r["active"].as_bool()), (Value::Null, Some(true)));
    let r = s.execute("hardware.slot.delete", &json!({"id": "kol"})).unwrap();
    assert_eq!((r["slots"].as_u64(), s.undo.len()), (Some(1), undo + 3));
    assert_eq!(slots(&mut s).len(), 1);
}

/// The Çift Açılım template's sash (its four profiles, as the plugin's boxes stand) takes the
/// placement slots the plugin's template adds, each where its hardware goes, and they follow the
/// sash's size.
#[test]
fn the_tilt_and_turn_templates_slots_sit_where_its_hardware_goes() {
    let mut s = Session::default();
    for (name, expression) in [("FFH", "1400 mm"), ("FFB", "900 mm"), ("KOL", "600 mm"), ("KA", "120 mm"), ("IS", "300 mm")] {
        s.execute("parameters.add", &json!({"name": name, "expression": expression})).unwrap();
    }
    for (name, size, center) in [
        ("Kanat sol", ["62 mm", "70 mm", "FFH"], ["31 mm", "35 mm", "FFH/2"]),
        ("Kanat sag", ["62 mm", "70 mm", "FFH"], ["FFB - 31 mm", "35 mm", "FFH/2"]),
        ("Kanat ust", ["FFB - 124 mm", "70 mm", "62 mm"], ["FFB/2", "35 mm", "FFH - 31 mm"]),
        ("Kanat alt", ["FFB - 124 mm", "70 mm", "62 mm"], ["FFB/2", "35 mm", "31 mm"]),
    ] {
        s.execute(
            "solid.box",
            &json!({"length": size[0], "width": size[1], "height": size[2], "center": center, "name": name, "body_name": name, "operation": "new"}),
        )
        .unwrap();
    }
    // The template's slots (aksesuar-eklentisi/src/sablon.rs, yerlesim_noktalari) at FFH 1400, FFB 900.
    let slot = |id: &str, corner: [f64; 3], edge: [f64; 3], on: [f64; 3], along: &str| {
        json!({"id": id, "label": id, "group": "g", "occurrence": 0, "edge": edge, "corner": corner, "on": on, "along": along,
            "across": "middle", "angle": 0, "flip": false})
    };
    let mut lock = slot("ek-kilit-1", [0.0, 0.0, 0.0], [0.0, 0.0, 700.0], [0.0, 35.0, 700.0], "(KA + KOL - IS/2)/2");
    lock["when"] = json!("FFH > 1200 mm");
    for p in [
        slot("sol-alt-kose", [0.0, 0.0, 0.0], [0.0, 0.0, 700.0], [0.0, 35.0, 700.0], "0"),
        slot("sol-ust-kose", [0.0, 0.0, 1400.0], [0.0, 0.0, 700.0], [0.0, 35.0, 700.0], "0"),
        slot("sag-ust-kose", [900.0, 0.0, 1400.0], [900.0, 0.0, 700.0], [900.0, 35.0, 700.0], "0"),
        slot("sag-alt-kose", [900.0, 0.0, 0.0], [900.0, 0.0, 700.0], [900.0, 35.0, 700.0], "0"),
        slot("alt-mentese", [900.0, 0.0, 0.0], [869.0, 0.0, 0.0], [869.0, 35.0, 0.0], "0"),
        slot("ust-mentese", [900.0, 0.0, 1400.0], [869.0, 0.0, 1400.0], [869.0, 35.0, 1400.0], "0"),
        slot("makas", [838.0, 0.0, 1400.0], [450.0, 0.0, 1400.0], [450.0, 35.0, 1400.0], "0"),
        slot("kol", [0.0, 0.0, 0.0], [0.0, 0.0, 700.0], [31.0, 0.0, 700.0], "KOL"),
        lock,
    ] {
        s.execute("hardware.slot.add", &p).unwrap_or_else(|e| panic!("{p}: {e}"));
    }
    let at = |s: &mut Session| -> Vec<(String, Vec3, Vec3, bool)> {
        slots(s)
            .iter()
            .map(|v| {
                assert!(v.get("error").is_none(), "{v}");
                (v["id"].as_str().unwrap().to_owned(), v3(&v["frame"]["origin"]), v3(&v["frame"]["z"]), v["active"].as_bool().unwrap())
            })
            .collect()
    };
    let expected = [
        ("sol-alt-kose", Vec3::new(0.0, 35.0, 0.0), -Vec3::X),
        ("sol-ust-kose", Vec3::new(0.0, 35.0, 1400.0), -Vec3::X),
        ("sag-ust-kose", Vec3::new(900.0, 35.0, 1400.0), Vec3::X),
        ("sag-alt-kose", Vec3::new(900.0, 35.0, 0.0), Vec3::X),
        ("alt-mentese", Vec3::new(900.0, 35.0, 0.0), -Vec3::Z),
        ("ust-mentese", Vec3::new(900.0, 35.0, 1400.0), Vec3::Z),
        ("makas", Vec3::new(838.0, 35.0, 1400.0), Vec3::Z),
        ("kol", Vec3::new(31.0, 0.0, 600.0), -Vec3::Y),
        ("ek-kilit-1", Vec3::new(0.0, 35.0, 285.0), -Vec3::X),
    ];
    let listed = at(&mut s);
    assert_eq!(listed.len(), expected.len());
    for ((id, origin, z, active), (want, place, out)) in listed.iter().zip(expected) {
        assert_eq!(id, want);
        assert!(near(*origin, place) && near(*z, out) && *active, "{id}: {origin:?} z {z:?}");
    }
    // A smaller, wider sash: the slots move with it, and the extra lock is out of use.
    s.execute("parameters.change", &json!({"changes": {"FFH": "1200 mm", "FFB": "1000 mm"}})).unwrap();
    let listed = at(&mut s);
    let find = |id: &str| listed.iter().find(|x| x.0 == id).unwrap().clone();
    assert!(near(find("sag-ust-kose").1, Vec3::new(1000.0, 35.0, 1200.0)), "{:?}", find("sag-ust-kose"));
    assert!(near(find("makas").1, Vec3::new(938.0, 35.0, 1200.0)), "{:?}", find("makas"));
    assert!(near(find("alt-mentese").1, Vec3::new(1000.0, 35.0, 0.0)), "{:?}", find("alt-mentese"));
    assert!(!find("ek-kilit-1").3 && find("kol").3);
}

#[test]
fn a_slot_that_breaks_its_rules_is_refused_and_one_gone_bad_is_an_error_on_it_only() {
    let mut s = sash();
    // Other tools' keys and the placed parts are kept as they are.
    s.execute("document.metadata.set", &json!({"key": SLOTS_KEY, "value": {"version": 1, "slots": [], "placed": [{"slot": "x"}], "note": "kept"}}))
        .unwrap();
    let ok = json!({"label": "Alt menteşe", "group": "Menteşe grubu", "edge": [900, 0, 600], "corner": [900, 0, 0]});
    let with = |key: &str, value: Value| {
        let mut p = ok.clone();
        p[key] = value;
        p
    };
    let undo = s.undo.len();
    for p in [
        with("along", json!("NOPE")),
        with("across", json!("MISSING + 1 mm")),
        with("angle", json!(45)),
        with("when", json!("NOPE > 1")),
        with("when", json!("FFH")),
        with("label", json!("  ")),
        with("label", json!("a\u{7}")),
        with("id", json!("Not A Slug")),
        with("edge", json!([2000, 0, 0])),
        with("edge", json!([900, 0])),
        with("corner", json!([1e12, 0, 0])),
        with("colour", json!("red")),
    ] {
        assert!(s.execute("hardware.slot.add", &p).is_err(), "{p}");
    }
    assert_eq!(s.undo.len(), undo, "nothing changed");
    s.execute("hardware.slot.add", &with("id", json!("alt"))).unwrap();
    assert!(s.execute("hardware.slot.add", &with("id", json!("alt"))).is_err(), "ids are unique");
    let r = s.execute("hardware.slot.add", &ok).unwrap();
    assert_eq!(r["slot"]["id"], "alt-mentese");
    let stored = s.doc.metadata.get(SLOTS_KEY).unwrap().clone();
    assert_eq!((&stored["placed"], &stored["note"]), (&json!([{"slot": "x"}]), &json!("kept")), "{stored}");

    // A slot whose expression no longer evaluates is listed with its error; the others are fine.
    let mut broken = stored.clone();
    broken["slots"][0]["along"] = json!("GONE");
    broken["slots"].as_array_mut().unwrap().push(json!({"id": "odd", "label": 3}));
    s.execute("document.metadata.set", &json!({"key": SLOTS_KEY, "value": broken})).unwrap();
    let r = s.execute("hardware.slot.list", &json!({})).unwrap();
    let listed = r["slots"].as_array().unwrap();
    assert_eq!(listed.len(), 2, "{r}");
    assert!(listed[0]["error"].as_str().is_some_and(|e| e.contains("along")) && listed[0]["frame"].is_null() && listed[0]["active"] == false, "{r}");
    assert!(listed[1].get("error").is_none() && listed[1]["frame"].is_object(), "{r}");
    assert_eq!(r["unreadable"].as_array().map(Vec::len), Some(1), "{r}");
    // The unreadable entry stays as it is through a change.
    s.execute("hardware.slot.delete", &json!({"id": "alt"})).unwrap();
    assert_eq!(s.doc.metadata.get(SLOTS_KEY).unwrap()["slots"].as_array().map(Vec::len), Some(2));
    assert!(s.execute("hardware.slot.edit", &json!({"id": "missing", "label": "x"})).is_err());
    assert!(s.execute("hardware.slot.edit", &json!({"id": "alt-mentese", "edge": [900, 0, 600]})).is_err(), "edge needs corner");
    assert!(s.execute("hardware.slot.delete", &json!({"id": "missing"})).is_err());

    // A slot list in another format is never changed.
    s.execute("document.metadata.set", &json!({"key": SLOTS_KEY, "value": {"version": 2, "slots": []}})).unwrap();
    assert!(s.execute("hardware.slot.list", &json!({})).is_err());
    assert!(s.execute("hardware.slot.add", &ok).is_err());
}

// ---- Placing parts ----

/// A part from the STEP corpus (`tests/step-conformance`) copied into the library's "Makas ve
/// kol" group, with a card; with `connection`, its connection is the cylinder's bottom face (the
/// corpus cylinders stand on the origin): z down out of it, x along +X.
fn part(lib: &Library, name: &str, connection: bool) -> String {
    let from = format!("{}/../../tests/step-conformance/{name}.step", env!("CARGO_MANIFEST_DIR"));
    let to = lib.0.join(format!("Makas ve kol/{name}.step"));
    std::fs::create_dir_all(to.parent().unwrap()).unwrap();
    std::fs::copy(&from, &to).unwrap();
    let mut card = json!({"part_no": format!("P-{name}")});
    if connection {
        card["connection"] = json!({"origin": [0, 0, 0], "z": [0, 0, -1], "x": [1, 0, 0]});
    }
    std::fs::write(lib.0.join(format!("Makas ve kol/{name}.json")), card.to_string()).unwrap();
    to.to_string_lossy().into_owned()
}

/// The part's connection frame in the world as its occurrence is placed now: origin, x, z.
fn connection_now(s: &Session, occurrence: u64) -> (Vec3, Vec3, Vec3) {
    let c = HardwareConnection { origin: [0.0; 3], z: [0.0, 0.0, -1.0], x: [1.0, 0.0, 0.0] };
    let m = mat_mul(&joints::occurrence_world(&s.doc, occurrence), &connection_mat(&c).unwrap());
    (column(&m, 3), column(&m, 0), column(&m, 2))
}

/// A row-major 4x4 matrix from a reply, as a placement.
fn placement_of(v: &Value) -> Mat {
    let rows: Vec<Vec<f64>> = serde_json::from_value(v.clone()).unwrap();
    std::array::from_fn(|c| std::array::from_fn(|r| rows[r][c]))
}

fn slot_frame(v: &Value) -> (Vec3, Vec3, Vec3) {
    (v3(&v["frame"]["origin"]), v3(&v["frame"]["x"]), v3(&v["frame"]["z"]))
}

#[test]
fn a_part_placed_on_a_slot_sits_on_it_and_follows_its_parameters() {
    let lib = Library::new();
    // One cylinder placed twice: a top occurrence holding two, joined by rigid joints.
    let path = part(&lib, "cylinder_twice", true);
    let mut s = sash();
    let slot = |label: &str, along: &str, angle: u16| json!({"label": label, "group": "Makas ve kol", "edge": [900, 0, 600], "corner": [900, 0, 0], "on": [900, 35, 600], "along": along, "angle": angle});
    s.execute("hardware.slot.add", &slot("Kol", "KOL", 0)).unwrap();
    s.execute("hardware.slot.add", &slot("Kol 2", "KOL - 300 mm", 90)).unwrap();
    let (undo, components) = (s.undo.len(), s.doc.components.len());
    let preview = s.execute("hardware.place", &json!({"slot": "kol", "path": path, "preview": true})).unwrap();
    assert_eq!(
        (preview["preview"].as_bool(), s.undo.len(), s.doc.components.len()),
        (Some(true), undo, components),
        "a preview changes nothing: {preview}"
    );
    assert!(preview["occurrence"].is_null() && preview["name"] == "P-cylinder_twice", "{preview}");

    let r = s.execute("hardware.place", &json!({"slot": "kol", "path": path})).unwrap();
    assert_eq!(s.undo.len(), undo + 1, "one undo step");
    assert_eq!((r["reused"].as_bool(), r["preview"].as_bool()), (Some(false), Some(false)), "{r}");
    assert!(r.get("warnings").is_none(), "{r}");
    let (component, occurrence, joint) = (r["component"].as_u64().unwrap(), r["occurrence"].as_u64().unwrap(), r["joint"].as_u64().unwrap());
    let j = s.doc.assembly.joints.iter().find(|j| j.id == joint).expect("the joint");
    assert_eq!((j.kind, j.b.occurrence, j.name.as_str()), (joints::JointKind::Planar, occurrence, "Kol: P-cylinder_twice"));
    let (o, x, z) = connection_now(&s, occurrence);
    assert!(near(o, Vec3::new(900.0, 35.0, 600.0)) && near(z, -Vec3::X) && near(x, Vec3::Z), "on the slot, z against its z: {o:?} {x:?} {z:?}");
    assert!(!moved(&joints::occurrence_world(&s.doc, occurrence), &placement_of(&preview["transform"])), "the preview is where it went: {preview}");
    let components_placed = s.doc.components.len();
    assert!(components_placed > components);

    // It follows the parameters the slot uses.
    s.execute("parameters.change", &json!({"changes": {"KOL": "700 mm", "FFB": "1000 mm"}})).unwrap();
    let (o, x, z) = connection_now(&s, occurrence);
    assert!(near(o, Vec3::new(1000.0, 35.0, 700.0)) && near(z, -Vec3::X) && near(x, Vec3::Z), "{o:?} {x:?} {z:?}");

    // The slot is taken now; the list says by what.
    let e = s.execute("hardware.place", &json!({"slot": "kol", "path": path})).unwrap_err().to_string();
    assert!(e.contains(TAKEN), "{e}");
    let listed = slots(&mut s);
    assert_eq!((listed[0]["placed"]["occurrence"].as_u64(), listed[0]["placed"]["joint"].as_u64()), (Some(occurrence), Some(joint)), "{listed:?}");
    assert!(listed[1].get("placed").is_none(), "{listed:?}");

    // The same file again: another occurrence of its component, nothing read again.
    let undo = s.undo.len();
    let preview = s.execute("hardware.place", &json!({"slot": "kol-2", "path": path, "preview": true})).unwrap();
    assert_eq!(preview["reused"], true, "{preview}");
    let r2 = s.execute("hardware.place", &json!({"slot": "kol-2", "path": path})).unwrap();
    assert_eq!((r2["reused"].as_bool(), r2["component"].as_u64(), s.undo.len()), (Some(true), Some(component), undo + 1), "{r2}");
    assert_eq!(s.doc.components.len(), components_placed, "one definition");
    assert_eq!(s.doc.occurrences.iter().filter(|o| o.component == component).count(), 2);
    let second = r2["occurrence"].as_u64().unwrap();
    let (o, x, z) = connection_now(&s, second);
    let (so, sx, sz) = slot_frame(&slots(&mut s)[1]);
    assert!(near(o, so) && near(x, sx) && near(z, -sz) && near(so, Vec3::new(1000.0, 35.0, 400.0)), "{o:?} {x:?} {z:?}: {so:?} {sx:?} {sz:?}");
    assert!(!moved(&joints::occurrence_world(&s.doc, second), &placement_of(&preview["transform"])), "{preview}");
    let (o, ..) = connection_now(&s, occurrence);
    assert!(near(o, Vec3::new(1000.0, 35.0, 700.0)), "the first stays: {o:?}");

    // Taken away again: one undo step each, the component with the last.
    let undo = s.undo.len();
    let u = s.execute("hardware.unplace", &json!({"slot": "kol"})).unwrap();
    assert_eq!((u["component_removed"].as_bool(), u["occurrence"].as_u64(), s.undo.len()), (Some(false), Some(occurrence), undo + 1), "{u}");
    assert!(!s.doc.occurrences.iter().any(|o| o.id == occurrence) && !s.doc.assembly.joints.iter().any(|j| j.id == joint));
    let (o, ..) = connection_now(&s, second);
    assert!(near(o, Vec3::new(1000.0, 35.0, 400.0)), "the other stays: {o:?}");
    let u = s.execute("hardware.unplace", &json!({"slot": "kol-2"})).unwrap();
    assert_eq!((u["component_removed"].as_bool(), s.doc.components.len()), (Some(true), components), "{u}");
    assert_eq!(s.doc.metadata.get(SLOTS_KEY).unwrap()["placed"], json!([]));
    assert!(s.execute("hardware.unplace", &json!({"slot": "kol"})).is_err(), "nothing there");
    // Undo puts the last one back where it was.
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(s.doc.occurrences.iter().any(|o| o.id == second));
    assert_eq!(slots(&mut s)[1]["placed"]["occurrence"].as_u64(), Some(second));
}

#[test]
fn placing_needs_a_connection_a_slot_and_a_readable_part() {
    let lib = Library::new();
    let bare = part(&lib, "cylinder", false);
    let good = part(&lib, "washer", true);
    let broken = lib.file("Makas ve kol/broken.step", "ISO-10303-21;");
    lib.file("Makas ve kol/broken.json", r#"{"connection": {"origin": [0, 0, 0], "z": [0, 0, 1], "x": [1, 0, 0]}}"#);
    let mut s = sash();
    let on = json!({"label": "Alt", "group": "Makas ve kol", "edge": [900, 0, 600], "corner": [900, 0, 0], "on": [900, 35, 600]});
    s.execute("hardware.slot.add", &on).unwrap();
    let (undo, components, occurrences) = (s.undo.len(), s.doc.components.len(), s.doc.occurrences.len());
    let error = |s: &mut Session, p: Value| s.execute("hardware.place", &p).unwrap_err().to_string();
    let e = error(&mut s, json!({"slot": "alt", "path": bare}));
    assert!(e.contains(NO_CONNECTION), "{e}");
    let e = error(&mut s, json!({"slot": "missing", "path": good}));
    assert!(e.contains("no placement slot"), "{e}");
    let e = error(&mut s, json!({"slot": "alt", "path": broken}));
    assert!(!e.is_empty(), "a file that doesn't read");
    for p in [
        json!({"slot": "alt", "path": lib.file("Makas ve kol/notes.txt", "x")}),
        json!({"slot": "alt", "path": lib.0.join("Makas ve kol/missing.step").to_string_lossy()}),
        json!({"slot": "alt", "path": ""}),
        json!({"slot": "alt"}),
        json!({"slot": "alt", "path": good, "colour": 1}),
    ] {
        assert!(s.execute("hardware.place", &p).is_err(), "{p}");
    }
    assert!(s.execute("hardware.unplace", &json!({"slot": "alt"})).is_err(), "nothing placed");
    assert_eq!((s.undo.len(), s.doc.components.len(), s.doc.occurrences.len()), (undo, components, occurrences), "nothing changed");
    // A part deleted by hand frees its slot.
    let r = s.execute("hardware.place", &json!({"slot": "alt", "path": good})).unwrap();
    s.execute("selection.delete", &json!({"occurrences": [r["occurrence"]]})).unwrap();
    assert!(slots(&mut s)[0].get("placed").is_none());
    let again = s.execute("hardware.place", &json!({"slot": "alt", "path": good})).unwrap();
    assert_eq!(again["reused"], false, "its component went with it: {again}");
    assert_eq!(s.doc.metadata.get(SLOTS_KEY).unwrap()["placed"].as_array().map(Vec::len), Some(1), "the stale entry went");
}

/// A slot with a condition takes a part of its own (never another occurrence of one another
/// placement brought in, nor one a slot without a condition shares), and one whose condition does
/// not hold now takes it hidden: its Import feature suppressed, with a warning.
#[test]
fn a_conditional_slot_takes_a_part_of_its_own_hidden_while_its_condition_does_not_hold() {
    let lib = Library::new();
    let path = part(&lib, "washer", true);
    let mut s = sash();
    let slot = |label: &str, along: &str, when: Option<&str>| {
        let mut p =
            json!({"label": label, "group": "Makas ve kol", "edge": [900, 0, 600], "corner": [900, 0, 0], "on": [900, 35, 600], "along": along});
        if let Some(when) = when {
            p["when"] = json!(when);
        }
        p
    };
    for p in [
        slot("Ek", "300 mm", Some("FFH > 1300 mm")),
        slot("Ust", "900 mm", Some("FFH > 1000 mm")),
        slot("Alt", "100 mm", None),
        slot("Alt 2", "200 mm", None),
    ] {
        s.execute("hardware.slot.add", &p).unwrap();
    }
    let imports = |s: &Session, component: u64| -> Vec<bool> {
        s.doc
            .features
            .iter()
            .filter(|f| f.component == component && matches!(f.kind, solidcraft_doc::FeatureKind::Import { .. }))
            .map(|f| f.suppressed)
            .collect()
    };
    let bodies = |s: &Session, component: u64| s.model.state().bodies.iter().filter(|b| s.doc.model_body_component(b) == component).count();

    // Out of its condition now: placed all the same, hidden, in one undo step.
    let undo = s.undo.len();
    let preview = s.execute("hardware.place", &json!({"slot": "ek", "path": path, "preview": true})).unwrap();
    assert_eq!((preview["active"].as_bool(), preview["preview"].as_bool(), s.undo.len()), (Some(false), Some(true), undo), "{preview}");
    let hidden = s.execute("hardware.place", &json!({"slot": "ek", "path": path})).unwrap();
    assert_eq!(s.undo.len(), undo + 1, "one undo step");
    assert_eq!((hidden["active"].as_bool(), hidden["reused"].as_bool()), (Some(false), Some(false)), "{hidden}");
    assert!(hidden["warnings"].as_array().is_some_and(|w| w.iter().any(|w| w == HIDDEN)), "{hidden}");
    let (ek, ek_occurrence) = (hidden["component"].as_u64().unwrap(), hidden["occurrence"].as_u64().unwrap());
    assert_eq!(imports(&s, ek), [true], "its Import feature is suppressed");
    assert_eq!(bodies(&s, ek), 0, "nothing of it shows");
    let (o, ..) = connection_now(&s, ek_occurrence);
    assert!(near(o, Vec3::new(900.0, 35.0, 300.0)), "it sits on its slot all the same: {o:?}");
    let listed = slots(&mut s);
    assert_eq!((listed[0]["active"].as_bool(), listed[0]["placed"]["occurrence"].as_u64()), (Some(false), Some(ek_occurrence)), "{listed:?}");

    // The same file on another conditional slot, in use: a component of its own, shown.
    let preview = s.execute("hardware.place", &json!({"slot": "ust", "path": path, "preview": true})).unwrap();
    assert_eq!((preview["reused"].as_bool(), preview["active"].as_bool()), (Some(false), Some(true)), "{preview}");
    let shown = s.execute("hardware.place", &json!({"slot": "ust", "path": path})).unwrap();
    assert!(shown.get("warnings").is_none() && shown["reused"] == false && shown["active"] == true, "{shown}");
    let ust = shown["component"].as_u64().unwrap();
    assert_ne!(ust, ek, "never the hidden part's component");
    assert_eq!(imports(&s, ust), [false]);
    assert!(bodies(&s, ust) > 0);

    // A slot without a condition never shares a conditional part's component; the next one shares
    // its own.
    let alt = s.execute("hardware.place", &json!({"slot": "alt", "path": path})).unwrap();
    let component = alt["component"].as_u64().unwrap();
    assert!(alt["reused"] == false && component != ek && component != ust, "{alt}");
    let alt2 = s.execute("hardware.place", &json!({"slot": "alt-2", "path": path})).unwrap();
    assert_eq!((alt2["reused"].as_bool(), alt2["component"].as_u64()), (Some(true), Some(component)), "{alt2}");

    // Taken away, the hidden part takes its own component with it; the others stay.
    let u = s.execute("hardware.unplace", &json!({"slot": "ek"})).unwrap();
    assert_eq!(u["component_removed"].as_bool(), Some(true), "{u}");
    assert!(imports(&s, ek).is_empty() && imports(&s, ust) == [false] && bodies(&s, ust) > 0);
}

/// Editing a slot that holds a part moves the part with it in the same undo step, keeping the turn
/// given to it there (a flip of the slot flips it); deleting the slot takes the part away too, in
/// one step that undo brings back.
#[test]
fn editing_or_deleting_a_slot_takes_its_part_along() {
    let lib = Library::new();
    let path = part(&lib, "washer", true);
    let mut s = sash();
    s.execute(
        "hardware.slot.add",
        &json!({"label": "Kol", "group": "Makas ve kol", "edge": [900, 0, 600], "corner": [900, 0, 0], "on": [900, 35, 600], "along": "KOL"}),
    )
    .unwrap();
    let r = s.execute("hardware.place", &json!({"slot": "kol", "path": path})).unwrap();
    let (occurrence, joint) = (r["occurrence"].as_u64().unwrap(), r["joint"].as_u64().unwrap());
    // Turned half a turn on its slot, as the placement card's Turn 180° does.
    s.execute("joint.edit", &json!({"joint": joint, "angle": 180})).unwrap();
    let (_, turned, _) = connection_now(&s, occurrence);
    let undo = s.undo.len();
    s.execute("hardware.slot.edit", &json!({"id": "kol", "along": "KOL - 100 mm", "label": "Kol rozet"})).unwrap();
    assert_eq!(s.undo.len(), undo + 1, "one undo step");
    let (o, x, z) = connection_now(&s, occurrence);
    assert!(near(o, Vec3::new(900.0, 35.0, 500.0)) && near(x, turned) && near(z, -Vec3::X), "moved with its slot, still turned: {o:?} {x:?} {z:?}");
    let listed = slots(&mut s);
    let now = listed[0]["placed"]["joint"].as_u64().unwrap();
    let j = s.doc.assembly.joints.iter().find(|j| j.id == now).expect("the part's joint");
    assert_eq!((j.b.occurrence, j.name.as_str()), (occurrence, "Kol rozet: P-washer"));
    assert_eq!(s.doc.assembly.joints.iter().filter(|j| j.b.occurrence == occurrence).count(), 1, "the old joint went");
    // It follows the slot's parameters as before.
    s.execute("parameters.change", &json!({"changes": {"KOL": "700 mm"}})).unwrap();
    assert!(near(connection_now(&s, occurrence).0, Vec3::new(900.0, 35.0, 600.0)));
    s.execute("hardware.slot.edit", &json!({"id": "kol", "flip": true})).unwrap();
    assert!(near(connection_now(&s, occurrence).2, Vec3::X), "a flipped slot flips its part");
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(near(connection_now(&s, occurrence).2, -Vec3::X), "undo puts it back");

    let (undo, components) = (s.undo.len(), s.doc.components.len());
    let r = s.execute("hardware.slot.delete", &json!({"id": "kol"})).unwrap();
    assert_eq!(s.undo.len(), undo + 1, "one undo step");
    assert_eq!((r["slots"].as_u64(), r["unplaced"]["occurrence"].as_u64()), (Some(0), Some(occurrence)), "{r}");
    assert!(!s.doc.occurrences.iter().any(|o| o.id == occurrence) && s.doc.components.len() < components, "the part and its component went");
    assert!(!s.doc.assembly.joints.iter().any(|j| j.b.occurrence == occurrence));
    assert_eq!(s.doc.metadata.get(SLOTS_KEY).unwrap()["placed"], json!([]));
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(s.doc.occurrences.iter().any(|o| o.id == occurrence));
    assert_eq!(slots(&mut s)[0]["placed"]["occurrence"].as_u64(), Some(occurrence));
    // A slot without a part deletes as before.
    s.execute("hardware.unplace", &json!({"slot": "kol"})).unwrap();
    let r = s.execute("hardware.slot.delete", &json!({"id": "kol"})).unwrap();
    assert!(r.get("unplaced").is_none(), "{r}");
}

/// A part placed before whose component comes and goes on its own (its Import feature
/// suppressed, or switched by a configuration rule) is never placed again as another occurrence
/// of that component: the new part would come and go with it.
#[test]
fn a_hidden_or_switched_part_is_not_placed_again_as_another_occurrence() {
    let lib = Library::new();
    let path = part(&lib, "washer", true);
    let mut s = sash();
    for (label, along) in [("Alt", "100 mm"), ("Alt 2", "200 mm"), ("Alt 3", "300 mm")] {
        s.execute(
            "hardware.slot.add",
            &json!({"label": label, "group": "Makas ve kol", "edge": [900, 0, 600], "corner": [900, 0, 0], "on": [900, 35, 600], "along": along}),
        )
        .unwrap();
    }
    let import = |s: &Session, component: u64| {
        s.doc.features.iter().find(|f| f.component == component && matches!(f.kind, solidcraft_doc::FeatureKind::Import { .. })).unwrap().id
    };
    let first = s.execute("hardware.place", &json!({"slot": "alt", "path": path})).unwrap()["component"].as_u64().unwrap();
    assert_eq!(s.execute("hardware.place", &json!({"slot": "alt-2", "path": path, "preview": true})).unwrap()["reused"], true);
    // A rule switches it (as Koşullu Parçaları Güncelle's do).
    let feature = import(&s, first);
    let rule: solidcraft_doc::config::ConfigRule = serde_json::from_value(json!({"id": "kosul-alt-off", "name": "Koşullu parça: Alt",
        "condition": {"type": "always"}, "effect": {"type": "drive_suppression", "feature": feature, "value": false}}))
    .unwrap();
    s.doc_mut().configs.rules.push(rule);
    let second = s.execute("hardware.place", &json!({"slot": "alt-2", "path": path})).unwrap();
    assert!(second["reused"] == false && second["component"].as_u64() != Some(first), "{second}");
    // Suppressed: the same.
    let other = second["component"].as_u64().unwrap();
    s.execute("timeline.suppress", &json!({"feature": import(&s, other), "suppressed": true})).unwrap();
    let preview = s.execute("hardware.place", &json!({"slot": "alt-3", "path": path, "preview": true})).unwrap();
    assert_eq!(preview["reused"], false, "{preview}");
    let third = s.execute("hardware.place", &json!({"slot": "alt-3", "path": path})).unwrap();
    assert!(third["reused"] == false && ![Some(first), Some(other)].contains(&third["component"].as_u64()), "{third}");
}

/// A data card an editor saved with a UTF-8 byte order mark (Notepad) reads, lists and merges.
#[test]
fn a_card_with_a_byte_order_mark_reads() {
    let lib = Library::new();
    let part = lib.file("Menteşe grubu/hinge.step", "ISO-10303-21;");
    lib.file("Menteşe grubu/hinge.json", "\u{feff}{\"part_no\": \"H-1\", \"supplier\": \"kept\"}");
    let r = list(&lib.folder()).unwrap();
    let p = r["parts"].as_array().unwrap().iter().find(|p| p["name"] == "hinge").unwrap().clone();
    assert!(p["card_error"].is_null() && p["card"]["part_no"] == "H-1", "{p}");
    let r = Session::default().execute("hardware.card.set", &json!({"path": part, "description": "Hinge"})).unwrap();
    assert_eq!((r["card"]["part_no"].as_str(), r["card"]["description"].as_str()), (Some("H-1"), Some("Hinge")), "{r}");
    let written = std::fs::read_to_string(lib.0.join("Menteşe grubu/hinge.json")).unwrap();
    assert!(written.contains("\"supplier\": \"kept\"") && !written.starts_with('\u{feff}'), "{written}");
}
