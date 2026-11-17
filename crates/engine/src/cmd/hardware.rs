//! The hardware library: a folder of window-hardware parts (STEP files) and profiles (DXF sections
//! or SolidCraft designs), one group per subfolder. Each part may have a data card, `<file
//! stem>.json` beside its file: a part number, a description, what it is for and where it
//! connects. Listing reads the folder and the cards without changing the design; a broken card is
//! reported on its part and never fails the list.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::CommandSpec;
use crate::schema::hardware::{
    HardwareCard, HardwareCardSet, HardwareCardSetParams, HardwareConnection, HardwareGroup, HardwareKind, HardwareLibrary,
    HardwareLibraryListParams, HardwarePart, HardwareRole,
};
use crate::schema::{TypedContract, output_schema_for};
use crate::{Result, Session, params::bad};
use serde_json::{Map, Value};

const LIST_CONTRACT: TypedContract<HardwareLibraryListParams> = TypedContract::new();
const CARD_SET_CONTRACT: TypedContract<HardwareCardSetParams> = TypedContract::new();

/// Group folders listed; the rest are left out (`truncated`).
pub(crate) const MAX_GROUPS: usize = 64;
/// Parts listed, in all groups together.
pub(crate) const MAX_PARTS: usize = 2000;
/// Directory entries read from one folder, files and folders of any kind.
const MAX_ENTRIES: usize = 10_000;
/// A data card's size.
pub(crate) const MAX_CARD_BYTES: usize = 64 * 1024;
/// A card's part number and description, in characters.
const MAX_TEXT: usize = 256;
/// A connection coordinate's size.
const MAX_COORDINATE: f64 = 1e6;
/// A connection direction shorter than this is no direction.
const MIN_LENGTH: f64 = 1e-9;
/// The sine of the angle between `z` and `x` below which they count as parallel.
const MIN_SINE: f64 = 1e-6;
/// Group folders whose parts are profiles when their cards don't say (ASCII, any case).
const PROFILE_GROUPS: [&str; 4] = ["profil", "profile", "profiles", "profiller"];

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec::new("hardware.library.list", "Hardware Library", crate::checked!("hardware.library.list", list))
        .icon("folder")
        .noundo()
        .read_only()
        .doc_only()
        .contract(&LIST_CONTRACT)
        .output(output_schema_for::<HardwareLibrary>)
        .params(
            "folder: the library folder (each subfolder a group, one level deep; files directly in it are in group \"\") → folder, \
         groups: [{name, parts}], parts: [{name (file stem), file, path, group, kind: step|dxf|sc, role: hardware|profile (the \
         card's, else profile in a group folder named Profil or Profile), card: {part_no?, description?, role?, connection?: \
         {origin, z, x} (in the part's own coordinates)} | null, card_error? (a card that could not be read; the list still \
         succeeds), card_path (<file stem>.json beside the file; null for a .json design)}] sorted by group then name, truncated \
         (over 64 groups or 2000 parts). Parts: .step, .stp and .dxf files in any case; SolidCraft designs (.sc, and .json that is \
         no other part's card) only as profiles, in a Profil group or (.sc) with a card whose role is profile. Links and other \
         non-regular files are skipped. Never changes the design",
        ),
    CommandSpec::new("hardware.card.set", "Set Hardware Card", crate::checked!("hardware.card.set", card_set))
        .icon("settings")
        .noundo()
        .doc_only()
        .contract(&CARD_SET_CONTRACT)
        .output(output_schema_for::<HardwareCardSet>)
        .params(
            "path: the part's .step, .stp, .dxf or .sc file; part_no?, description?: text (at most 256 characters); role?: \
         hardware|profile; connection?: {origin, z, x} (in the part's own coordinates, each coordinate under 1e6; z and x not \
         zero, not parallel; saved as a unit z and the unit x square to it). A key left out keeps its value, null or \"\" removes \
         it; other keys in the card are kept. Writes <file stem>.json beside the file atomically → path, card_path, card. Never \
         changes the design",
        ),
];

/// `path` with the platform's own separators: on Windows a `/` typed into or saved in a path is a
/// `\`, so the paths listed under a folder, and a part's card path, read one way.
fn native(path: &str) -> String {
    if cfg!(windows) { path.replace('/', "\\") } else { path.to_owned() }
}

fn path(cmd: &str, what: &str, value: &str) -> Result<()> {
    if value.is_empty() || value.len() > 4096 || value.chars().any(char::is_control) {
        return Err(bad(cmd, format!("invalid {what} path")));
    }
    solidcraft_io::vfs::check(value).map_err(|e| bad(cmd, e.to_string()))
}

fn reply(cmd: &str, reply: impl serde::Serialize) -> Result<Value> {
    serde_json::to_value(reply).map_err(|e| bad(cmd, e.to_string()))
}

/// What a file that can have a data card holds, by its extension (any case): STEP, DXF or a
/// design container (`.sc` or an older design extension). `None`: anything else, `.json` too.
pub(crate) fn kind_of(path: &Path) -> Option<HardwareKind> {
    let ext = path.extension()?.to_str()?;
    if ext.eq_ignore_ascii_case("step") || ext.eq_ignore_ascii_case("stp") {
        Some(HardwareKind::Step)
    } else if ext.eq_ignore_ascii_case("dxf") {
        Some(HardwareKind::Dxf)
    } else if solidcraft_io::is_design_ext(ext) {
        Some(HardwareKind::Sc)
    } else {
        None
    }
}

fn is_json(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("json"))
}

/// A file stem compared as the file system does on Windows and macOS (any case).
fn stem_key(path: &Path) -> Option<String> {
    path.file_stem().map(|stem| stem.to_string_lossy().to_lowercase())
}

/// Whether parts in `group` are profiles when their cards don't say.
fn profile_group(group: &str) -> bool {
    PROFILE_GROUPS.iter().any(|name| group.trim().eq_ignore_ascii_case(name))
}

/// What a part in `group` is for when its card doesn't say.
fn group_role(group: &str) -> HardwareRole {
    if profile_group(group) { HardwareRole::Profile } else { HardwareRole::Hardware }
}

/// A file or folder directly in a folder: its name and path.
type Entry = (String, PathBuf);

/// The regular files and the folders directly in `dir`, each sorted by name (case-insensitive
/// first). Links, other kinds of entry, hidden names (`.…`) and names that are not UTF-8 are
/// skipped, and at most [`MAX_ENTRIES`] entries are read.
fn entries(dir: &Path) -> std::io::Result<(Vec<Entry>, Vec<Entry>)> {
    let mut files = Vec::new();
    let mut folders = Vec::new();
    for entry in std::fs::read_dir(dir)?.take(MAX_ENTRIES) {
        let Ok(entry) = entry else { continue };
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_symlink() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else { continue };
        if name.starts_with('.') {
            continue;
        }
        if kind.is_dir() {
            folders.push((name, entry.path()));
        } else if kind.is_file() {
            files.push((name, entry.path()));
        }
    }
    for list in [&mut files, &mut folders] {
        list.sort_by_cached_key(|(name, _)| (name.to_lowercase(), name.clone()));
    }
    Ok((files, folders))
}

/// `connection` as a frame: `z` a unit vector, `x` the unit vector square to it in the plane of
/// the two. Refuses coordinates that are not finite or too large, a zero direction, and parallel
/// directions.
fn frame(c: &HardwareConnection) -> std::result::Result<HardwareConnection, String> {
    if c.origin.iter().chain(&c.z).chain(&c.x).any(|v| !v.is_finite() || v.abs() >= MAX_COORDINATE) {
        return Err("connection coordinates must be finite numbers under 1e6 in size".into());
    }
    let length = |v: &[f64; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let (lz, lx) = (length(&c.z), length(&c.x));
    if lz < MIN_LENGTH || lx < MIN_LENGTH {
        return Err("connection `z` and `x` must not be zero".into());
    }
    let z = c.z.map(|v| v / lz);
    let x = c.x.map(|v| v / lx);
    let along = z[0] * x[0] + z[1] * x[1] + z[2] * x[2];
    // What is left of x without its share along z is square to z.
    let square = [x[0] - along * z[0], x[1] - along * z[1], x[2] - along * z[2]];
    let sine = length(&square);
    if sine < MIN_SINE {
        return Err("connection `z` and `x` must not be parallel".into());
    }
    Ok(HardwareConnection { origin: c.origin, z, x: square.map(|v| v / sine) })
}

/// A data card from its JSON: an object whose known keys are valid. Keys it doesn't know are
/// ignored.
fn card_from(value: &Value) -> std::result::Result<HardwareCard, String> {
    if !value.is_object() {
        return Err("a data card must be a JSON object".into());
    }
    let card: HardwareCard = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
    for (key, text) in [("part_no", &card.part_no), ("description", &card.description)] {
        if let Some(text) = text
            && (text.chars().count() > MAX_TEXT || text.chars().any(char::is_control))
        {
            return Err(format!("`{key}` must be at most {MAX_TEXT} characters, without control characters"));
        }
    }
    if let Some(connection) = &card.connection {
        frame(connection)?;
    }
    Ok(card)
}

/// The data card at `path`: `Ok(None)` when there is none.
fn read_card(path: &str) -> std::result::Result<Option<HardwareCard>, String> {
    let bytes = match solidcraft_io::vfs::read(path, MAX_CARD_BYTES) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let value: Value = serde_json::from_slice(&bytes).map_err(|e| format!("not JSON: {e}"))?;
    card_from(&value).map(Some)
}

/// A part's data card path: `<file stem>.json` beside it.
fn card_path(part: &Path) -> String {
    part.with_extension("json").to_string_lossy().into_owned()
}

/// The parts among one group's files, at most `room` of them, and whether more were left out. A
/// `.json` file is a design only in a profile group and only when it is no other part's card; a
/// design container outside a profile group is a part only when its card says it is a profile.
fn group_parts(group: &str, files: Vec<Entry>, room: usize) -> (Vec<HardwarePart>, bool) {
    let profiles = profile_group(group);
    // The stems of the files that can have a card: a `.json` of the same stem is that card.
    let carded: BTreeSet<String> = files.iter().filter(|(_, path)| kind_of(path).is_some()).filter_map(|(_, path)| stem_key(path)).collect();
    let mut parts = Vec::new();
    for (file, path) in files {
        let (kind, card_path) = match kind_of(&path) {
            Some(kind) => (kind, Some(card_path(&path))),
            None if profiles && is_json(&path) && stem_key(&path).is_some_and(|stem| !carded.contains(&stem)) => (HardwareKind::Sc, None),
            None => continue,
        };
        let Some(text) = path.to_str().map(str::to_owned) else { continue };
        let (card, card_error) = match card_path.as_deref().map(read_card) {
            Some(Ok(card)) => (card, None),
            Some(Err(e)) => (None, Some(e)),
            None => (None, None),
        };
        let role = card.as_ref().and_then(|c| c.role).unwrap_or_else(|| group_role(group));
        if kind == HardwareKind::Sc && role != HardwareRole::Profile && !profiles {
            continue;
        }
        if parts.len() >= room {
            return (parts, true);
        }
        let name = path.file_stem().map_or_else(|| file.clone(), |stem| stem.to_string_lossy().into_owned());
        parts.push(HardwarePart { name, file, path: text, group: group.to_owned(), kind, role, card, card_error, card_path });
    }
    (parts, false)
}

fn list(_s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "hardware.library.list";
    let request: HardwareLibraryListParams = serde_json::from_value(p.clone()).map_err(|e| bad(cmd, e.to_string()))?;
    let folder = native(&request.folder);
    path(cmd, "library folder", &folder)?;
    let root = Path::new(&folder);
    if !std::fs::metadata(root).is_ok_and(|m| m.is_dir()) {
        return Err(bad(cmd, format!("`{folder}` is not a folder")));
    }
    let (files, folders) = entries(root).map_err(|e| bad(cmd, format!("could not read the library folder: {e}")))?;
    let mut truncated = folders.len() > MAX_GROUPS;
    let mut sources: Vec<(String, Vec<Entry>)> = vec![(String::new(), files)];
    for (name, folder) in folders.into_iter().take(MAX_GROUPS) {
        // A group folder that can't be read is listed empty.
        sources.push((name, entries(&folder).map(|(files, _)| files).unwrap_or_default()));
    }
    let mut groups = Vec::new();
    let mut parts: Vec<HardwarePart> = Vec::new();
    for (group, files) in sources {
        let (found, full) = group_parts(&group, files, MAX_PARTS.saturating_sub(parts.len()));
        truncated |= full;
        if !group.is_empty() || !found.is_empty() {
            groups.push(HardwareGroup { name: group, parts: found.len() });
        }
        parts.extend(found);
    }
    reply(cmd, HardwareLibrary { folder, groups, parts, truncated, param_warnings: Vec::new() })
}

/// Apply one key of `hardware.card.set` to the card's object: left out keeps it, `None` removes it.
fn merge(object: &mut Map<String, Value>, key: &str, value: Option<Option<Value>>) {
    match value {
        None => {}
        Some(None) => {
            object.remove(key);
        }
        Some(Some(value)) => {
            object.insert(key.into(), value);
        }
    }
}

fn card_set(_s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "hardware.card.set";
    let request: HardwareCardSetParams = serde_json::from_value(p.clone()).map_err(|e| bad(cmd, e.to_string()))?;
    let part_path = native(&request.path);
    path(cmd, "part", &part_path)?;
    let part = Path::new(&part_path);
    if kind_of(part).is_none() {
        return Err(bad(cmd, "the part must be a .step, .stp, .dxf or .sc file"));
    }
    if !std::fs::metadata(part).is_ok_and(|m| m.is_file()) {
        return Err(bad(cmd, format!("no part file `{part_path}`")));
    }
    let card_path = card_path(part);
    path(cmd, "data card", &card_path)?;
    // Merged with the card there, whose keys this version doesn't know are kept.
    let mut object = match solidcraft_io::vfs::read(&card_path, MAX_CARD_BYTES) {
        Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
            Ok(Value::Object(object)) => object,
            _ => return Err(bad(cmd, format!("`{card_path}` is not a JSON object: correct or remove it first"))),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Map::new(),
        Err(e) => return Err(bad(cmd, format!("could not read the data card: {e}"))),
    };
    let text = |value: Option<Option<String>>| value.map(|v| v.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty()).map(Value::String));
    merge(&mut object, "part_no", text(request.part_no));
    merge(&mut object, "description", text(request.description));
    let role = request.role.map(|role| role.map(serde_json::to_value).transpose()).transpose().map_err(|e| bad(cmd, e.to_string()))?;
    merge(&mut object, "role", role);
    let connection = match request.connection {
        Some(Some(c)) => Some(Some(serde_json::to_value(frame(&c).map_err(|e| bad(cmd, e))?).map_err(|e| bad(cmd, e.to_string()))?)),
        Some(None) => Some(None),
        None => None,
    };
    merge(&mut object, "connection", connection);
    let value = Value::Object(object);
    let card = card_from(&value).map_err(|e| bad(cmd, format!("the data card would be invalid: {e}")))?;
    let mut bytes = serde_json::to_vec_pretty(&value).map_err(|e| bad(cmd, e.to_string()))?;
    bytes.push(b'\n');
    if bytes.len() > MAX_CARD_BYTES {
        return Err(bad(cmd, format!("the data card would be over {} KiB", MAX_CARD_BYTES / 1024)));
    }
    solidcraft_io::write_atomic(Path::new(&card_path), &bytes, false).map_err(|e| bad(cmd, format!("could not write the data card: {e}")))?;
    reply(cmd, HardwareCardSet { path: part_path, card_path, card, param_warnings: Vec::new() })
}

#[cfg(test)]
#[path = "hardware_tests.rs"]
mod tests;
