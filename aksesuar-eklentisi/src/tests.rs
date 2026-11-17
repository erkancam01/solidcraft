#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use serde_json::{Value, json};

use crate::test_ana::TestAna;
use crate::{GIRISLER, Hata, MANIFEST, calistir};

fn manifest() -> Value {
    serde_json::from_str(MANIFEST).unwrap()
}

/// Commands SolidCraft marks read (`.read_only()` in crates/engine/src/cmd): the only ones a
/// read entry may call.
const READ_COMMANDS: &[&str] = &["document.inspect", "model.edges", "model.faces"];

#[test]
fn the_host_accepts_the_manifest() {
    let parsed = craft_plugin::parse_manifest(MANIFEST.as_bytes(), "aksesuar").unwrap();
    let runtime = parsed.runtime.unwrap();
    let mut names: Vec<&str> = runtime.entries.iter().map(|e| e.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, GIRISLER);
    assert_eq!(runtime.module, "aksesuar.wasm");
    assert!(runtime.permissions.files.iter().any(|f| f.name == crate::tij::IZIN && f.write));
    assert!(runtime.permissions.command_groups.is_empty(), "no sensitive command groups");
    assert!(MANIFEST.len() < craft_plugin::MAX_MANIFEST_BYTES);
    let limits = craft_plugin::HostLimits::default();
    limits.check(&runtime.limits).unwrap();
}

#[test]
fn every_entry_has_a_turkish_menu_and_ribbon_title() {
    for entry in manifest()["runtime"]["entries"].as_array().unwrap() {
        let slots = entry["slots"].as_array().unwrap();
        let menu = slots.iter().find(|s| s["kind"] == "menu").unwrap()["name"].as_str().unwrap();
        let ribbon = slots.iter().find(|s| s["kind"] == "ribbon").unwrap()["name"].as_str().unwrap();
        assert_eq!(menu, ribbon);
        // Not a Design tab name (that would move it), not File/Help (that would add a host item).
        for taken in ["model", "file", "help", "sketch", "assemble", "inspect"] {
            assert_ne!(menu.to_lowercase(), taken);
        }
        assert!(crate::ozellikler::SATIRLAR.iter().any(|l| l.contains(menu)), "help lists {menu}");
    }
}

/// The checks `plugin_menu::form` in crates/ui-egui makes: a schema it cannot draw leaves Run off.
fn drawable(schema: &Value) -> Result<(), String> {
    const ANNOTATIONS: &[&str] = &["type", "title", "description", "default", "unit", "x-unit", "units", "examples", "$comment", "deprecated"];
    let o = schema.as_object().ok_or("not an object")?;
    if o.get("type") != Some(&json!("object")) {
        return Err("not an object schema".into());
    }
    let props = o.get("properties").and_then(Value::as_object).ok_or("no properties")?;
    for key in o.get("required").and_then(Value::as_array).into_iter().flatten() {
        if !props.contains_key(key.as_str().ok_or("required name")?) {
            return Err(format!("required {key} has no field"));
        }
    }
    for (name, p) in props {
        let p = p.as_object().ok_or("property")?;
        let allowed: &[&str] = match p.get("type").and_then(Value::as_str) {
            Some("number") => &["minimum", "maximum", "exclusiveMinimum", "exclusiveMaximum", "multipleOf"],
            Some("integer") => &["minimum", "maximum", "exclusiveMinimum", "exclusiveMaximum"],
            Some("boolean") => &[],
            Some("string") => &["minLength", "maxLength", "format", "enum", "oneOf"],
            other => return Err(format!("{name}: type {other:?} is not drawn")),
        };
        if let Some(k) = p.keys().find(|k| !ANNOTATIONS.contains(&k.as_str()) && !allowed.contains(&k.as_str())) {
            return Err(format!("{name}: `{k}` is not drawn"));
        }
    }
    Ok(())
}

#[test]
fn every_entry_opens_a_dialog_the_app_can_draw() {
    for entry in manifest()["runtime"]["entries"].as_array().unwrap() {
        drawable(&entry["input_schema"]).unwrap_or_else(|e| panic!("{}: {e}", entry["name"]));
    }
}

#[test]
fn unknown_entries_and_bad_params_are_refused_in_turkish() {
    let mut ana = TestAna::new(|_, _| Ok(json!({})));
    let e = calistir(&mut ana, "nope", &json!({})).unwrap_err();
    assert!(matches!(e, Hata::Girdi { ref neden, .. } if neden.contains("Bilinmeyen")), "{e:?}");
    let e = calistir(&mut ana, "ozellikler", &json!([1])).unwrap_err();
    assert!(matches!(e, Hata::Girdi { ref neden, .. } if neden.contains("JSON nesnesi")), "{e:?}");
    assert!(ana.komutlar.is_empty());
}

#[test]
fn the_help_entry_calls_nothing_and_names_every_entry() {
    let mut ana = TestAna::new(|_, _| panic!("no commands"));
    let out = calistir(&mut ana, "ozellikler", &Value::Null).unwrap();
    let text = out.as_array().unwrap().iter().map(|l| l.as_str().unwrap()).collect::<Vec<_>>().join("\n");
    for title in ["Kanala Yerleştir", "Tij ve Malzeme Listesi", "Çift Açılım Şablonu", "Aksesuar Özellikleri", "EK KILIT 1", "COCUK KILIDI"] {
        assert!(text.contains(title), "{title}");
    }
}

/// Every command an entry sends is in the manifest, and read entries send read commands only.
pub fn commands_are_granted(entry: &str, sent: &[&str]) {
    let m = manifest();
    let listed: Vec<&str> = m["runtime"]["commands"].as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect();
    let read = m["runtime"]["entries"].as_array().unwrap().iter().any(|e| e["name"] == entry && e["effect"] == "read");
    for c in sent {
        assert!(listed.contains(c), "{entry} sends {c}, which plugin.json does not list");
        if read {
            assert!(READ_COMMANDS.contains(c), "read entry {entry} sends edit command {c}");
        }
    }
}

#[test]
fn the_manifest_lists_its_commands_sorted_and_once() {
    let m = manifest();
    let listed: Vec<&str> = m["runtime"]["commands"].as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect();
    let mut sorted = listed.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(listed, sorted);
}
