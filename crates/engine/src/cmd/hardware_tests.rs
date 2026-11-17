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
