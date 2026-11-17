use serde_json::{Value, json};

use super::{HardwareCardSet, HardwareCardSetParams, HardwareLibrary, HardwareLibraryListParams};
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
