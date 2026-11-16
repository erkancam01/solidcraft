//! `aksesuar-harness <package folder>`: loads plugin.json and aksesuar.wasm from the folder, then
//! runs every entry of the component in SolidCraft's plugin runtime against a scripted host that
//! answers like the engine. It checks what reaches the host (commands, files, transactions), not
//! CAD results: the engine is not part of this run.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use craft_plugin::{CommandSchema, Effect, FileGrant, HostError, Manifest, PluginHost};
use serde_json::{Value, json};

const READ: &[&str] = &["document.inspect", "model.edges", "model.faces"];

struct Host {
    commands: Vec<String>,
    sent: Vec<(String, Value)>,
    folder: PathBuf,
    selection: Value,
    next_feature: u64,
    begins: usize,
    commits: usize,
    rollbacks: usize,
    logs: Vec<String>,
}

impl Host {
    fn new(manifest: &Manifest, folder: &Path, selection: Value) -> Self {
        let commands = manifest.runtime.as_ref().map(|r| r.commands.iter().cloned().collect()).unwrap_or_default();
        Host {
            commands,
            sent: Vec::new(),
            folder: folder.to_path_buf(),
            selection,
            next_feature: 100,
            begins: 0,
            commits: 0,
            rollbacks: 0,
            logs: Vec::new(),
        }
    }
    fn names(&self) -> Vec<&str> {
        self.sent.iter().map(|(c, _)| c.as_str()).collect()
    }
}

fn kanat_faces() -> Value {
    json!({"body": "Kanat", "faces": [
        {"index": 1, "area": 48112.0, "centroid": [450, 31, 0], "plane_normal": [0, 0, 1], "point": [450, 31, 0]},
        {"index": 4, "area": 54320.0, "centroid": [450, 62, -35], "plane_normal": [0, 1, 0], "point": [450, 62, -35]}
    ]})
}

fn makas_faces() -> Value {
    json!({"body": "Makas", "faces": [
        {"index": 2, "area": 3000.0, "centroid": [500, 500, 100], "plane_normal": [0, 0, -1], "point": [501, 450, 100]}
    ]})
}

fn makas_edges() -> Value {
    json!({"body": "Makas", "edges": [
        {"index": 0, "name": "a|b", "mid": [492.5, 500, 100], "length": 200.0, "start": [492.5, 600, 100], "end": [492.5, 400, 100]},
        {"index": 1, "name": "a|c", "mid": [500, 400, 100], "length": 15.0, "start": [492.5, 400, 100], "end": [507.5, 400, 100]}
    ]})
}

fn inspect() -> Value {
    let body = |name: &str, min: [f64; 3], max: [f64; 3]| json!({"name": name, "closed": true, "bbox": {"min": min, "max": max}, "visible": true});
    json!({"name": "Çift açılım", "units": "mm",
    "params": [{"name": "FFH", "expression": "1200 mm", "unit": "mm", "value": 1200.0, "model": false, "comment": "", "error": null}],
    "bodies": [
        body("Kanat sol", [0.0, 0.0, -70.0], [62.0, 1200.0, 0.0]),
        body("TIJ sol alt", [-8.5, 15.0, -40.0], [-1.5, 343.0, -30.0]),
        body("TIJ ust", [120.0, 1201.5, -40.0], [478.0, 1208.5, -30.0])
    ]})
}

impl PluginHost for Host {
    fn command_catalog(&self) -> Vec<CommandSchema> {
        self.commands
            .iter()
            .map(|name| CommandSchema {
                name: name.clone(),
                input_schema: json!({"type": "object"}),
                output_schema: json!({}),
                effect: if READ.contains(&name.as_str()) { Effect::Read } else { Effect::Edit },
                group: None,
            })
            .collect()
    }
    fn run_command(&mut self, name: &str, args: &Value) -> craft_plugin::Result<Value> {
        self.sent.push((name.to_string(), args.clone()));
        let body = args.get("body").and_then(Value::as_str);
        Ok(match (name, body) {
            ("parameters.list", _) => json!({"parameters": [{"name": "KOL"}]}),
            ("selection.get", _) => self.selection.clone(),
            ("model.faces", Some("Kanat")) => kanat_faces(),
            ("model.faces", Some("Makas")) => makas_faces(),
            ("model.edges", Some("Makas")) => makas_edges(),
            ("component.from_bodies", _) => json!({"components": [{"component": 7, "body": "Makas", "feature": 30}]}),
            ("component.list", _) => json!({"components": [{"id": 7, "name": "Makas", "occurrences": [{"id": 12, "name": "Makas:1"}]}]}),
            ("joint.create", _) => json!({"joint": 40, "warnings": []}),
            ("document.inspect", _) => inspect(),
            ("solid.box", _) => {
                self.next_feature += 1;
                json!({"feature": self.next_feature, "name": args["name"]})
            }
            ("config.configure", _) => json!({"active": "Default", "rows": [{"id": 1, "name": "Default"}]}),
            ("parameters.add" | "appearance.assign" | "config.rule" | "config.activate", _) => json!({}),
            _ => return Err(HostError::CommandFailed { command: name.into(), reason: "not scripted".into() }),
        })
    }
    fn begin_transaction(&mut self) -> craft_plugin::Result<()> {
        self.begins += 1;
        Ok(())
    }
    fn commit_transaction(&mut self) -> craft_plugin::Result<()> {
        self.commits += 1;
        Ok(())
    }
    fn rollback_transaction(&mut self) -> craft_plugin::Result<()> {
        self.rollbacks += 1;
        Ok(())
    }
    fn granted_folders(&self) -> Vec<FileGrant> {
        vec![FileGrant { name: "cikti".into(), folder: self.folder.clone(), read: false, write: true }]
    }
    fn granted_command_groups(&self) -> BTreeSet<String> {
        BTreeSet::new()
    }
    fn log(&mut self, message: &str) {
        self.logs.push(message.to_string());
    }
    fn progress(&mut self, _: f32, _: &str) {}
}

fn check(ok: bool, what: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(what.to_string()) }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("PASS: every Aksesuar entry ran in the plugin runtime");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("FAILED: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let dir = std::env::args().nth(1).map(PathBuf::from).ok_or("usage: aksesuar-harness <package folder>")?;
    let manifest_bytes = std::fs::read(dir.join("plugin.json")).map_err(|e| format!("plugin.json: {e}"))?;
    let manifest = craft_plugin::parse_manifest(&manifest_bytes, "aksesuar").map_err(|e| format!("manifest: {e}"))?;
    let module = manifest.runtime.as_ref().map(|r| r.module.clone()).ok_or("manifest has no runtime")?;
    let component = std::fs::read(dir.join(&module)).map_err(|e| format!("{module}: {e}"))?;
    craft_plugin::HostLimits::default().check(&manifest.runtime.as_ref().ok_or("runtime")?.limits).map_err(|e| e.to_string())?;
    let out = std::env::temp_dir().join(format!("aksesuar-harness-{}", std::process::id()));
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let entry = |selection: Value, name: &str, params: Value| {
        let host = Host::new(&manifest, &out, selection);
        let (host, result) = craft_plugin_wasm::run(host, &manifest, &component, name, &params);
        println!(
            "{name} {params}: {} command(s), result {}",
            host.sent.len(),
            match &result {
                Ok(v) => v.to_string().chars().take(200).collect::<String>(),
                Err(e) => format!("error: {e}"),
            }
        );
        (host, result)
    };

    // The help text: no commands, a list of lines.
    let (host, r) = entry(json!({}), "ozellikler", json!({}));
    let lines = r.map_err(|e| format!("ozellikler: {e}"))?;
    check(host.sent.is_empty() && lines.as_array().is_some_and(|l| l.len() > 10), "ozellikler returns its help lines")?;

    // The template: 1 + 13 parameters + 22 boxes and looks + CK + configure, rules, activate.
    let (host, r) = entry(json!({}), "cift-acilim-sablonu", json!({"FFH": 1400, "FFB": 1100, "KOL": 700, "cocuk_kilidi": true}));
    let v = r.map_err(|e| format!("cift-acilim-sablonu: {e}"))?;
    let names = host.names();
    check(names.len() == 62, &format!("template sends 62 commands, sent {}", names.len()))?;
    check(names.last() == Some(&"config.activate"), "template ends by activating its configuration")?;
    check(v["ek_kilit"] == 121 && v["cocuk_kilidi"] == 122, &format!("template reads the lock ids: {v}"))?;
    let ck = host.sent.iter().find(|(c, p)| c == "parameters.add" && p["name"] == "CK").map(|(_, p)| p["expression"].clone());
    check(ck == Some(json!("1")), "child lock checkbox sets CK = 1")?;
    check(host.commits == 1 && host.rollbacks == 0, "template commits once")?;

    // The cut list: one read command, one CSV in the granted folder.
    let (host, r) = entry(json!({}), "tij-listesi", json!({"dosya": "harness"}));
    let v = r.map_err(|e| format!("tij-listesi: {e}"))?;
    check(host.names() == ["document.inspect"], "tij-listesi reads the design once")?;
    let csv = std::fs::read(out.join("harness.csv")).map_err(|e| format!("harness.csv: {e}"))?;
    check(csv.starts_with(&[0xEF, 0xBB, 0xBF]), "CSV starts with a UTF-8 BOM")?;
    let text = String::from_utf8_lossy(&csv);
    check(text.contains("TIJ sol alt;328\r\n") && text.contains("TIJ ust;358\r\n"), &format!("CSV lists the bars: {text}"))?;
    check(v["tij_sayisi"] == 2, "tij-listesi counts two bars")?;

    // Seating a part: selection, faces, edges, a component for the root body, one planar joint.
    let selection = json!({"count": 3, "items": [
        {"kind": "face", "body": "Makas", "index": 2, "point": [501, 450, 100]},
        {"kind": "edge", "body": "Kanat", "index": 9, "start": [62, 62, 0], "end": [838, 62, 0], "curve": "line"},
        {"kind": "face", "body": "Kanat", "index": 1, "point": [450, 31, 0]}
    ]});
    let (host, r) = entry(selection, "kanala-yerlestir", json!({"mesafe": "KOL"}));
    r.map_err(|e| format!("kanala-yerlestir: {e}"))?;
    let joint = host.sent.iter().find(|(c, _)| c == "joint.create").map(|(_, p)| p.clone()).ok_or("no joint.create")?;
    check(joint["type"] == "planar" && joint["values"] == json!([31.0, "KOL", 90.0]), &format!("planar joint: {joint}"))?;
    check(joint["b"]["occurrence"] == 12, "the part's new occurrence is joined")?;

    // A refusal: nothing selected. The runtime rolls back and reports the input in Turkish.
    let (host, r) = entry(json!({"count": 0, "items": []}), "kanala-yerlestir", json!({"mesafe": "10"}));
    match r {
        Err(HostError::InvalidInput { reason, .. }) if reason.contains("Hiçbir şey seçili değil") => {}
        other => return Err(format!("empty selection should be a Turkish input error, got {other:?}")),
    }
    check(host.commits == 0 && host.rollbacks == 1, "a refused entry rolls back")?;

    // An unknown entry is refused by the runtime before the guest runs.
    let (_, r) = entry(json!({}), "yok", json!({}));
    check(r.is_err(), "unknown entry refused")?;

    let _ = std::fs::remove_dir_all(&out);
    Ok(())
}
