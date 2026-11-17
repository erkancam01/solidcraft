use serde_json::{Value, json};

use super::{
    HardwareCardSet, HardwareCardSetParams, HardwareLibrary, HardwareLibraryListParams, HardwareSlotAddParams, HardwareSlotDeleteParams,
    HardwareSlotDeleted, HardwareSlotEditParams, HardwareSlotList, HardwareSlotListParams, HardwareSlotSet,
};
use crate::Session;
use crate::schema::tests::{assert_keys, assert_strict_keeps_todays_errors, conforms};
use crate::schema::{ParamCheck, input_schema_for, output_schema_for};

/// A library folder in the temporary folder, removed when dropped.
struct Folder(std::path::PathBuf);

impl Folder {
    fn new(name: &str) -> Self {
        let dir = solidcraft_io::vfs::temp_dir().join(format!("solidcraft-hardware-schema-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Köşe aktarma")).unwrap();
        std::fs::create_dir_all(dir.join("Profil")).unwrap();
        std::fs::write(dir.join("Köşe aktarma/corner.STEP"), "ISO-10303-21;").unwrap();
        std::fs::write(dir.join("Köşe aktarma/corner.json"), r#"{"part_no": "C-1"}"#).unwrap();
        std::fs::write(dir.join("Köşe aktarma/broken.step"), "ISO-10303-21;").unwrap();
        std::fs::write(dir.join("Köşe aktarma/broken.json"), "[").unwrap();
        std::fs::write(dir.join("Profil/sash.dxf"), "0\nEOF\n").unwrap();
        std::fs::write(dir.join("Profil/frame.json"), "{}").unwrap();
        Folder(dir)
    }
    fn path(&self, relative: &str) -> String {
        self.0.join(relative).to_string_lossy().into_owned()
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn schemas_and_the_key_check_agree() {
    assert_keys(input_schema_for::<HardwareLibraryListParams>(), &["folder"]);
    assert_keys(input_schema_for::<HardwareCardSetParams>(), &["connection", "description", "part_no", "path", "role"]);
    let slot = ["across", "along", "angle", "corner", "edge", "flip", "group", "id", "label", "occurrence", "on", "preview", "when"];
    assert_keys(input_schema_for::<HardwareSlotAddParams>(), &slot);
    assert_keys(input_schema_for::<HardwareSlotEditParams>(), &slot);
    assert_keys(input_schema_for::<HardwareSlotDeleteParams>(), &["id"]);
    assert_keys(input_schema_for::<HardwareSlotListParams>(), &["group"]);
}

/// A sash profile, x 838 … 900, y 0 … 70, z 0 … 1200, for the slot commands.
fn sash() -> Session {
    let mut s = Session::default();
    s.execute("solid.box", &json!({"length": 62, "width": 70, "height": 1200, "corner": [838, 0, 0]})).unwrap();
    s
}

/// The slot commands reply as their output schemas say, in Warn without warnings.
#[test]
fn slot_commands_reply_with_their_schemas() {
    let mut s = sash();
    s.param_check = ParamCheck::Warn;
    let run = |s: &mut Session, id: &str, p: Value| s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"));
    let matches = |schema: &schemars::Schema, r: &Value| {
        let schema = schema.as_value();
        assert!(conforms(schema, schema, r), "{r} doesn't match {schema}");
        assert!(r.get("param_warnings").is_none(), "{r}");
    };
    let (set, listed, deleted) = (output_schema_for::<HardwareSlotSet>(), output_schema_for::<HardwareSlotList>(), output_schema_for::<HardwareSlotDeleted>());
    let add = json!({"label": "Kol", "group": "Makas ve kol", "edge": [900, 0, 600], "corner": [900, 0, 0], "on": [900, 35, 600], "along": 250});
    let mut preview = add.clone();
    preview["preview"] = json!(true);
    matches(&set, &run(&mut s, "hardware.slot.add", preview));
    matches(&set, &run(&mut s, "hardware.slot.add", add));
    matches(&set, &run(&mut s, "hardware.slot.edit", json!({"id": "kol", "angle": 90, "when": "1 > 0"})));
    let r = run(&mut s, "hardware.slot.list", json!({}));
    matches(&listed, &r);
    assert_eq!(r["slots"].as_array().map(Vec::len), Some(1), "{r}");
    matches(&deleted, &run(&mut s, "hardware.slot.delete", json!({"id": "kol"})));
    matches(&listed, &run(&mut s, "hardware.slot.list", json!({"group": "Makas ve kol"})));
}

#[test]
fn strict_keeps_todays_errors() {
    let folder = Folder::new("strict");
    assert_strict_keeps_todays_errors(&[
        ("hardware.library.list", json!({"folder": ""})),
        ("hardware.library.list", json!({"folder": folder.path("missing")})),
        ("hardware.card.set", json!({"path": ""})),
        ("hardware.card.set", json!({"path": folder.path("Profil/sash.txt")})),
        ("hardware.card.set", json!({"path": folder.path("Profil/missing.dxf")})),
        ("hardware.card.set", json!({"path": folder.path("Profil/sash.dxf"), "part_no": "a\u{7}"})),
        ("hardware.card.set", json!({"path": folder.path("Profil/sash.dxf"), "connection": {"origin": [0, 0, 0], "z": [0, 0, 1], "x": [0, 0, 1]}})),
        ("hardware.slot.add", json!({"label": "", "group": "", "edge": [0, 0, 0], "corner": [0, 0, 0]})),
        ("hardware.slot.add", json!({"label": "A", "group": "", "edge": [5, 5, 5], "corner": [0, 0, 0], "angle": 45})),
        ("hardware.slot.edit", json!({"id": "missing"})),
        ("hardware.slot.delete", json!({"id": "missing"})),
    ]);
}

/// Listing a library and writing a card reply as their output schemas say, in Warn without
/// warnings; like the handlers, warn and strict refuse an unknown key.
#[test]
fn hardware_commands_reply_with_their_schemas() {
    let folder = Folder::new("replies");
    let (listed, set) = (output_schema_for::<HardwareLibrary>(), output_schema_for::<HardwareCardSet>());
    let mut s = Session { param_check: ParamCheck::Warn, ..Session::default() };
    let run = |s: &mut Session, id: &str, p: Value| s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"));
    let matches = |schema: &schemars::Schema, r: &Value| {
        let schema = schema.as_value();
        assert!(conforms(schema, schema, r), "{r} doesn't match {schema}");
        assert!(r.get("param_warnings").is_none(), "{r}");
    };
    let r = run(&mut s, "hardware.library.list", json!({"folder": folder.path("")}));
    matches(&listed, &r);
    assert_eq!(r["parts"].as_array().map(Vec::len), Some(4), "{r}");
    let r = run(
        &mut s,
        "hardware.card.set",
        json!({"path": folder.path("Profil/sash.dxf"), "part_no": "S-70", "role": "profile", "connection": {"origin": [0, 0, 0], "z": [0, 0, 1], "x": [1, 0, 0]}}),
    );
    matches(&set, &r);
    let r = run(&mut s, "hardware.card.set", json!({"path": folder.path("Profil/sash.dxf"), "part_no": null, "connection": null}));
    matches(&set, &r);
    matches(&listed, &run(&mut s, "hardware.library.list", json!({"folder": folder.path("")})));
    for level in [ParamCheck::Warn, ParamCheck::Strict] {
        let mut s = Session { param_check: level, ..Session::default() };
        let error =
            s.execute("hardware.library.list", &json!({"folder": folder.path(""), "fldr": 1})).err().map(|e| e.to_string()).unwrap_or_default();
        assert!(error.contains("fldr"), "{level:?}: {error}");
    }
}
