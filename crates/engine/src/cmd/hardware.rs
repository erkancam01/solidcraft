//! The hardware library: a folder of window-hardware parts (STEP files) and profiles (DXF sections
//! or SolidCraft designs), one group per subfolder. Each part may have a data card, `<file
//! stem>.json` beside its file: a part number, a description, what it is for and where it
//! connects. Listing reads the folder and the cards without changing the design; a broken card is
//! reported on its part and never fails the list.
//!
//! Placement slots ("yerleşim noktaları", `hardware.slot.*`) are named places on the sash where a
//! part of a library group goes, kept in the design's metadata (`tr.aksesuar.yerlesim`). Each
//! starts from an edge-corner joint origin on the floor face (the profile face the hardware sits
//! on), which the design finds again by name as its parameters change, and says how far along the
//! edge and across the floor the part's connection point goes and how it is turned. Listing them
//! works out each one's frame in the world and the planar joint (`joint.create`'s `a`, `values`
//! and `limits`) that puts a part's connection frame on it, as the plugin's "Kanala Yerleştir"
//! makes it.
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use solidcore_record::Vec3;
use solidcraft_doc::expr::Kind;
use solidcraft_doc::joints::{self, JointOrigin, Snap};
use solidcraft_doc::metadata::Owner;
use solidcraft_doc::{IDENTITY, Mat, apply_point, mat_inverse, mat_mul};
use solidcraft_geom::prelude::*;

use super::CommandSpec;
use crate::schema::hardware::{
    HardwareCard, HardwareCardSet, HardwareCardSetParams, HardwareConnection, HardwareGroup, HardwareKind, HardwareLibrary,
    HardwareLibraryListParams, HardwarePart, HardwareRole, HardwareSlot, HardwareSlotAddParams, HardwareSlotDeleteParams, HardwareSlotDeleted,
    HardwareSlotEditParams, HardwareSlotFrame, HardwareSlotList, HardwareSlotListParams, HardwareSlotListed, HardwareSlotSet,
};
use crate::schema::{TypedContract, output_schema_for};
use crate::{Result, Session, params::bad};
use serde_json::{Map, Value, json};

const LIST_CONTRACT: TypedContract<HardwareLibraryListParams> = TypedContract::new();
const CARD_SET_CONTRACT: TypedContract<HardwareCardSetParams> = TypedContract::new();
const SLOT_ADD_CONTRACT: TypedContract<HardwareSlotAddParams> = TypedContract::new();
const SLOT_DELETE_CONTRACT: TypedContract<HardwareSlotDeleteParams> = TypedContract::new();
const SLOT_EDIT_CONTRACT: TypedContract<HardwareSlotEditParams> = TypedContract::new();
const SLOT_LIST_CONTRACT: TypedContract<HardwareSlotListParams> = TypedContract::new();

/// The design metadata key the placement slots are kept under.
pub const SLOTS_KEY: &str = "tr.aksesuar.yerlesim";
/// The slot list's format.
const SLOTS_VERSION: u64 = 1;
/// Slots one design keeps.
pub(crate) const MAX_SLOTS: usize = 128;
/// A slot id's length.
const MAX_ID: usize = 64;
/// `across` for the middle of the floor face's width.
pub const MIDDLE: &str = "middle";
/// The turns a slot takes (deg).
pub const ANGLES: [u16; 4] = [0, 90, 180, 270];
/// How far along or across a slot may be (mm).
const MAX_DISTANCE: f64 = 1e6;
/// An expression's length, as the commands take it.
const MAX_EXPRESSION: usize = 4096;

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
    CommandSpec::new("hardware.slot.add", "Add Placement Slot", crate::checked!("hardware.slot.add", slot_add))
        .icon("joint_origin")
        .contract(&SLOT_ADD_CONTRACT)
        .output(output_schema_for::<HardwareSlotSet>)
        .params(
            "label, group (the library group whose parts go there), edge: [x, y, z] (a point on a straight edge of the floor face, \
         the profile face the hardware sits on; world), corner: [x, y, z] (the edge's end nearest it is the corner the slot is \
         measured from), on?: [x, y, z] (a point on the floor face; default the larger planar face beside the edge), occurrence?: \
         id (default the one at edge), along?: mm or expression along the edge from the corner (default \"0\"), across?: mm, \
         expression or \"middle\" (the middle of the floor face's width, the default), angle?: 0|90|180|270 (the part's turn about \
         the floor's normal; at 0 its connection x runs along the edge away from the corner), flip?: bool (the joint's flip), \
         when?: condition expression (the slot is in use while it is not 0, e.g. \"FFH > 1200 mm\"), id?: slug (default made from \
         the label), preview?: bool (work it out, change nothing) → slot, frame {origin, x, y, z} (world), active, joint_a, values, \
         limits (as hardware.slot.list gives them), preview. The edge, the floor face and the corner are kept by name, so the slot \
         follows the design's parameters. Kept in the design's metadata (tr.aksesuar.yerlesim), one undo step",
        ),
    CommandSpec::new("hardware.slot.delete", "Delete Placement Slot", crate::checked!("hardware.slot.delete", slot_delete))
        .icon("delete")
        .doc_only()
        .contract(&SLOT_DELETE_CONTRACT)
        .output(output_schema_for::<HardwareSlotDeleted>)
        .params("id → id, slots (how many are left). One undo step"),
    CommandSpec::new("hardware.slot.edit", "Edit Placement Slot", crate::checked!("hardware.slot.edit", slot_edit))
        .icon("joint_origin")
        .contract(&SLOT_EDIT_CONTRACT)
        .output(output_schema_for::<HardwareSlotSet>)
        .params(
            "id; label?, group?, along?, across?, angle?, flip?, when? (null or \"\": always in use) as hardware.slot.add takes them \
         (a key left out keeps its value); edge? and corner? (with on?, occurrence?) pick the slot's corner again; preview?: bool → \
         slot, frame, active, joint_a, values, limits, preview. One undo step",
        ),
    CommandSpec::new("hardware.slot.list", "List Placement Slots", crate::checked!("hardware.slot.list", slot_list))
        .icon("joint_origin")
        .noundo()
        .read_only()
        .contract(&SLOT_LIST_CONTRACT)
        .output(output_schema_for::<HardwareSlotList>)
        .params(
            "group?: only that library group's slots → slots: [{id, label, group, origin, along, across, angle, flip, when, active \
         (its condition holds now), frame: {origin, x, y, z} | null (world, after along, across and angle: the origin where the \
         part's connection point goes, z out of the floor face, x the way the part's connection x runs), error? (why it can't be \
         placed now; the list still succeeds), joint_a, values, limits: what joint.create {type: \"planar\", a: joint_a, b: <the \
         part's connection frame>, values, limits, flip} takes to put the part there (values: the shift along a's x and y, one of \
         them along as its expression, the other across; then the turn in deg; limits hold across and the turn)}], unreadable? \
         (entries this version can't read). Never changes the design",
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

// ---- Placement slots ----

/// A slot's edge-corner frame in the world: the corner, z out of the floor face, x and y along
/// the floor's two edges at the corner, away from it into the face (right-handed), as the joint
/// origin gives it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Corner {
    pub origin: Vec3,
    pub x: Vec3,
    pub y: Vec3,
    pub z: Vec3,
    /// The frame's x runs along the picked edge (else its y does).
    pub along_x: bool,
}

/// Where a part's connection frame goes on a slot: the planar joint's values (the shift along
/// the corner frame's x and y, mm, and the turn about its z, deg) and the frame they come to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Placement {
    pub values: [f64; 3],
    pub origin: Vec3,
    pub x: Vec3,
    pub y: Vec3,
    pub z: Vec3,
}

/// The planar joint's turn (deg, in (-180, 180]) for a slot's `angle`: at 0 the part's x runs
/// along the picked edge, which is the corner frame's x (no turn) or its y (a quarter turn).
pub(crate) fn turn_degrees(along_x: bool, angle: u16) -> i32 {
    let turn = (i32::from(angle) + if along_x { 0 } else { 90 }).rem_euclid(360);
    if turn > 180 { turn - 360 } else { turn }
}

/// Where a slot puts a part's connection frame: `along` the picked edge from the corner, `across`
/// the floor (both into the face), turned `angle`. The planar joint's values are the shift along
/// the corner frame's x and y, so `along` is the first when x runs along the edge, else the
/// second; the frame: z out of the floor, x the corner frame's x turned by the joint's turn.
pub(crate) fn place(c: &Corner, along: f64, across: f64, angle: u16) -> Placement {
    let turn = turn_degrees(c.along_x, angle);
    let values = if c.along_x { [along, across, f64::from(turn)] } else { [across, along, f64::from(turn)] };
    let origin = c.origin + c.x * values[0] + c.y * values[1];
    // A whole number of quarter turns, exactly.
    let (sin, cos) = match turn {
        0 => (0.0, 1.0),
        90 => (1.0, 0.0),
        180 | -180 => (0.0, -1.0),
        -90 => (-1.0, 0.0),
        t => f64::from(t).to_radians().sin_cos(),
    };
    let x = c.x * cos + c.y * sin;
    Placement { values, origin, x, y: c.z.cross(x), z: c.z }
}

/// How a slot's `across` goes into the joint: a value held by a limit, or an expression naming
/// parameters, which drives it instead.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Across {
    Held(f64),
    Expression(String),
}

/// A distance as the joint gets it: to a micrometre's millionth, no `-0`.
fn tidy(v: f64) -> f64 {
    (v * 1e6).round() / 1e6 + 0.0
}

/// `joint.create`'s values and limits for a planar joint onto a slot: `along` as its expression
/// (it follows the parameters), free; `across` held, or driven by its expression; the turn held.
pub(crate) fn joint_values(along_x: bool, along: &str, across: &Across, turn: f64) -> (Vec<Value>, Vec<Value>) {
    let along = (json!(along), Value::Null);
    let across = match across {
        Across::Held(v) => (json!(tidy(*v)), json!([tidy(*v), tidy(*v)])),
        Across::Expression(e) => (json!(e), Value::Null),
    };
    let (first, second) = if along_x { (along, across) } else { (across, along) };
    (vec![first.0, second.0, json!(turn)], vec![first.1, second.1, json!([turn, turn])])
}

/// A slot's corner frame and the geometry around it now, in the world.
struct Around {
    corner: Corner,
    /// The picked edge's other end.
    edge_end: Vec3,
    /// A point inside the floor face.
    floor_point: Vec3,
    /// The floor face across, from the corner: the least and the most.
    span: (f64, f64),
}

/// Points along a path exactly: each segment's ends and seven points between.
fn samples(path: &[solidcraft_kernel::PathSeg]) -> Vec<Vec3> {
    let mut points = Vec::with_capacity(path.len() * 9);
    for seg in path {
        points.push(seg.start());
        points.extend((1..8).filter_map(|k| seg.along(f64::from(k) / 8.0).map(|(p, _)| p)));
        points.push(seg.end());
    }
    points
}

fn column(m: &Mat, i: usize) -> Vec3 {
    m.get(i).map_or(Vec3::ZERO, |c| Vec3::new(c[0], c[1], c[2]))
}

/// A slot's corner frame now and the geometry around it, found by the names its origin keeps: the
/// picked edge (which must be straight), its floor face and the corner.
fn around(s: &Session, origin: &JointOrigin) -> std::result::Result<Around, String> {
    let Snap::Edge { edge: Some(edge), face: Some(face), corner: Some(_), axis: None, .. } = &origin.snap else {
        return Err("the slot does not start at a corner of a floor face's edge".into());
    };
    let doc = &s.doc;
    let st = s.model.state();
    let frame = joints::resolve_origin(doc, &st, origin).map_err(|e| e.to_string())?;
    let placed = if origin.occurrence == 0 { IDENTITY } else { joints::occurrence_world(doc, origin.occurrence) };
    let world = mat_mul(&placed, &frame);
    // The part's own coordinates, those its bodies' edges and faces are in, to the world.
    let on_part =
        if origin.inside.is_empty() { frame } else { joints::resolve_origin(doc, &st, &JointOrigin::new(origin.part(), origin.snap.clone())).map_err(|e| e.to_string())? };
    let to_world = mat_mul(&world, &mat_inverse(&on_part).ok_or("the slot's frame has no inverse")?);
    let corner_on_part = column(&on_part, 3);
    let component = super::joints::component_of(s, origin.part());
    let found = st.bodies.iter().filter(|b| doc.model_body_component(b) == component).find_map(|b| {
        let i = solidcraft_doc::naming::edge_names(b).iter().position(|n| n == edge)?;
        let f = solidcraft_doc::naming::face_names(b).iter().position(|n| n == face)?;
        let offset = doc.model_body_offset(b).copied().unwrap_or(IDENTITY);
        let path = solidcraft_kernel::edge_path(&b.body, i).ok()?;
        let points: Vec<Vec3> = samples(&path).into_iter().map(|p| apply_point(&offset, p)).collect();
        let (first, last) = (*points.first()?, *points.last()?);
        let tol = (b.body.size() * 2e-3).max(1e-3);
        let (near, far) = if first.dist(corner_on_part) <= tol {
            (first, last)
        } else if last.dist(corner_on_part) <= tol {
            (last, first)
        } else {
            return None;
        };
        Some((b, f, offset, near, far, points, tol))
    });
    let (body, face_index, offset, near, far, points, tol) = found.ok_or("the slot's edge or floor face is no longer there")?;
    let run = (far - near).normalized().ok_or("the slot's edge has no length")?;
    if points.iter().any(|p| (*p - near).cross(run).len() > tol) {
        return Err("the slot's edge is not straight".into());
    }
    let unit = |v: Vec3| v.normalized().ok_or("the slot's frame is degenerate");
    let (origin_w, x, y, z) = (column(&world, 3), unit(column(&world, 0))?, unit(column(&world, 1))?, unit(column(&world, 2))?);
    let edge_end = apply_point(&to_world, far);
    let de = unit(edge_end - origin_w)?;
    let along_x = x.dot(de).abs() >= y.dot(de).abs();
    let across = if along_x { y } else { x };
    let mut span: Option<(f64, f64)> = None;
    for e in solidcraft_kernel::face_edge_indices(&body.body, face_index) {
        let Ok(path) = solidcraft_kernel::edge_path(&body.body, e) else { continue };
        for p in samples(&path) {
            let d = (apply_point(&to_world, apply_point(&offset, p)) - origin_w).dot(across);
            span = Some(span.map_or((d, d), |(lo, hi)| (lo.min(d), hi.max(d))));
        }
    }
    let span = span.ok_or("the slot's floor face has no edges")?;
    let middle = origin_w + de * (edge_end.dist(origin_w) / 2.0) + across * ((span.0 + span.1) / 2.0);
    let floor_point = solidcraft_doc::naming::face_spot(body, face_index)
        .map(|spot| apply_point(&to_world, apply_point(&offset, spot.point)))
        .filter(|p| p.is_finite())
        .unwrap_or(middle);
    Ok(Around { corner: Corner { origin: origin_w, x, y, z, along_x }, edge_end, floor_point, span })
}

/// `v` as JSON coordinates, to a nanometre, no `-0`.
fn coordinates(v: Vec3) -> [f64; 3] {
    [v.x, v.y, v.z].map(|c| (c * 1e9).round() / 1e9 + 0.0)
}

/// A length expression's value now (mm), or why it has none.
fn length(s: &Session, text: &str) -> std::result::Result<f64, String> {
    let v = s.doc.eval(text, Kind::Length).map_err(|e| e.to_string())?;
    if !v.is_finite() || v.abs() >= MAX_DISTANCE {
        return Err(format!("`{text}` is too far (at most 1e6 mm)"));
    }
    Ok(v)
}

/// Whether a condition holds now (it is not 0), or why it can't be worked out.
fn condition(s: &Session, text: &str) -> std::result::Result<bool, String> {
    s.doc.eval(text, Kind::Unitless).map(|v| v != 0.0).map_err(|e| e.to_string())
}

fn is_middle(text: &str) -> bool {
    text.trim().eq_ignore_ascii_case(MIDDLE)
}

/// What a slot comes to now: its placement, the joint that puts a part there, and whether it is
/// in use.
struct Worked {
    frame: HardwareSlotFrame,
    joint_a: Value,
    values: Vec<Value>,
    limits: Vec<Value>,
}

/// A slot's frame and joint now, or why it can't be placed.
fn work_out(s: &Session, slot: &HardwareSlot) -> std::result::Result<Worked, String> {
    if !ANGLES.contains(&slot.angle) {
        return Err(format!("angle {} is not one of 0, 90, 180 and 270", slot.angle));
    }
    let around = around(s, &slot.origin)?;
    let along = length(s, &slot.along).map_err(|e| format!("along: {e}"))?;
    let across = if is_middle(&slot.across) {
        Across::Held((around.span.0 + around.span.1) / 2.0)
    } else {
        let v = length(s, &slot.across).map_err(|e| format!("across: {e}"))?;
        if solidcraft_doc::expr::references(&slot.across).is_empty() { Across::Held(v) } else { Across::Expression(slot.across.trim().to_owned()) }
    };
    let across_now = match &across {
        Across::Held(v) => tidy(*v),
        Across::Expression(_) => length(s, &slot.across).map_err(|e| format!("across: {e}"))?,
    };
    let p = place(&around.corner, along, across_now, slot.angle);
    let (values, limits) = joint_values(around.corner.along_x, slot.along.trim(), &across, p.values[2]);
    let c = around.corner.origin;
    let mut joint_a = json!({
        "occurrence": slot.origin.occurrence,
        "edge": coordinates(c + (around.edge_end - c) * 0.5),
        "corner": coordinates(c),
        "on": coordinates(around.floor_point),
    });
    if !slot.origin.inside.is_empty()
        && let Some(a) = joint_a.as_object_mut()
    {
        let path: Vec<u64> = std::iter::once(slot.origin.occurrence).chain(slot.origin.inside.iter().copied()).collect();
        a.insert("path".into(), json!(path));
    }
    let frame = HardwareSlotFrame { origin: coordinates(p.origin), x: coordinates(p.x), y: coordinates(p.y), z: coordinates(p.z) };
    Ok(Worked { frame, joint_a, values, limits })
}

/// A slot as `hardware.slot.list` lists it: whatever is wrong with it is its `error`.
fn listed(s: &Session, slot: HardwareSlot) -> HardwareSlotListed {
    let worked = work_out(s, &slot);
    let held = slot.when.as_deref().map_or(Ok(true), |w| condition(s, w).map_err(|e| format!("when: {e}")));
    let (frame, joint_a, values, limits, mut error) = match worked {
        Ok(w) => (Some(w.frame), Some(w.joint_a), Some(w.values), Some(w.limits), None),
        Err(e) => (None, None, None, None, Some(e)),
    };
    let active = match held {
        Ok(active) => active && error.is_none(),
        Err(e) => {
            if error.is_none() {
                error = Some(e);
            }
            false
        }
    };
    HardwareSlotListed { slot, active, frame, error, joint_a, values, limits }
}

/// The slot list kept in the design, with its other keys (`placed`) as they are: a new one when
/// the design has none. Refused when it is not one this version can change.
fn store(s: &Session, cmd: &str) -> Result<Map<String, Value>> {
    match s.doc.metadata.get(SLOTS_KEY) {
        None => {
            let mut store = Map::new();
            store.insert("version".into(), json!(SLOTS_VERSION));
            store.insert("slots".into(), json!([]));
            store.insert("placed".into(), json!([]));
            Ok(store)
        }
        Some(Value::Object(store)) => {
            if let Some(version) = store.get("version")
                && version.as_u64() != Some(SLOTS_VERSION)
            {
                return Err(bad(cmd, format!("the design's placement slots are in another format ({version}); this version reads format 1")));
            }
            if store.get("slots").is_some_and(|v| !v.is_array()) {
                return Err(bad(cmd, "the design's placement slots (`slots`) are not a list"));
            }
            Ok(store.clone())
        }
        Some(_) => Err(bad(cmd, format!("the design's `{SLOTS_KEY}` is not an object"))),
    }
}

fn raw_slots(store: &Map<String, Value>) -> Vec<Value> {
    store.get("slots").and_then(Value::as_array).cloned().unwrap_or_default()
}

fn raw_id(v: &Value) -> Option<&str> {
    v.get("id").and_then(Value::as_str)
}

/// Keep `slots` in the design: one undo step, and none when nothing changed.
fn save_slots(s: &mut Session, mut store: Map<String, Value>, slots: Vec<Value>, cmd: &str) -> Result<()> {
    store.insert("version".into(), json!(SLOTS_VERSION));
    store.insert("slots".into(), Value::Array(slots));
    store.entry("placed").or_insert_with(|| json!([]));
    let mut next = s.doc.metadata.clone();
    next.set(Owner::Design, SLOTS_KEY, Value::Object(store)).map_err(|e| bad(cmd, e.to_string()))?;
    if next != s.doc.metadata {
        s.doc_mut().metadata = next;
    }
    Ok(())
}

/// A slot's label or group: trimmed, at most 256 characters, no control characters.
fn slot_text(cmd: &str, what: &str, text: &str, required: bool) -> Result<String> {
    let text = text.trim();
    if required && text.is_empty() {
        return Err(bad(cmd, format!("`{what}` must not be empty")));
    }
    if text.chars().count() > MAX_TEXT || text.chars().any(char::is_control) {
        return Err(bad(cmd, format!("`{what}` must be at most {MAX_TEXT} characters, without control characters")));
    }
    Ok(text.to_owned())
}

/// A slot's condition: `None` for none (`""` too).
fn condition_text(cmd: &str, text: Option<&str>) -> Result<Option<String>> {
    let Some(text) = text.map(str::trim).filter(|t| !t.is_empty()) else { return Ok(None) };
    if text.len() > MAX_EXPRESSION || text.chars().any(char::is_control) {
        return Err(bad(cmd, format!("`when` must be at most {MAX_EXPRESSION} bytes, without control characters")));
    }
    Ok(Some(text.to_owned()))
}

/// Whether `id` is a slot id: `a-z`, `0-9`, `-` and `_`, at most 64 of them.
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_ID && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

/// A slot id made from its label: lower case ASCII (Turkish letters without their marks), words
/// joined by `-`.
pub(crate) fn slug(label: &str) -> String {
    let mut out = String::new();
    for c in label.chars().flat_map(char::to_lowercase) {
        let c = match c {
            'ç' => 'c',
            'ğ' => 'g',
            'ı' => 'i',
            'ö' => 'o',
            'ş' => 's',
            'ü' => 'u',
            'â' | 'à' | 'á' | 'ä' => 'a',
            'î' | 'ì' | 'í' | 'ï' => 'i',
            'û' | 'ù' | 'ú' => 'u',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'ô' | 'ò' | 'ó' => 'o',
            // The dot of a lower-cased `İ`.
            '\u{307}' => continue,
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            if out.len() >= MAX_ID - 8 {
                break;
            }
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-');
    if out.is_empty() { "slot".to_owned() } else { out.to_owned() }
}

/// `base`, or `base-2`, `base-3`… when it is taken.
fn unique_id(base: &str, taken: &[&str]) -> String {
    (1..=MAX_SLOTS + 1)
        .map(|n| if n == 1 { base.to_owned() } else { format!("{base}-{n}") })
        .find(|id| !taken.contains(&id.as_str()))
        .unwrap_or_else(|| format!("{base}-{}", MAX_SLOTS + 2))
}

/// The slot's origin: the edge-corner joint origin at the picks, as `joint.origin` makes it, with
/// the names it is found again by.
fn pick_origin(s: &Session, edge: [f64; 3], corner: [f64; 3], on: Option<[f64; 3]>, occurrence: Option<u64>, cmd: &str) -> Result<JointOrigin> {
    if [Some(edge), Some(corner), on].iter().flatten().flatten().any(|v| !v.is_finite() || v.abs() >= 1e9) {
        return Err(bad(cmd, "`edge`, `corner` and `on` must be points of finite numbers under 1e9 in size"));
    }
    let mut v = Map::new();
    v.insert("edge".into(), json!(edge));
    v.insert("corner".into(), json!(corner));
    if let Some(on) = on {
        v.insert("on".into(), json!(on));
    }
    if let Some(occurrence) = occurrence {
        v.insert("occurrence".into(), json!(occurrence));
    }
    let origin = super::joints::origin_param(s, Some(&Value::Object(v)), cmd)?;
    match &origin.snap {
        Snap::Edge { edge: Some(_), face: Some(_), corner: Some(_), axis: None, .. } => Ok(origin),
        _ => Err(bad(cmd, "pick a straight edge of a planar floor face, and the corner it starts from: the edge, the face and the corner need names")),
    }
}

/// A slot checked and worked out, as `hardware.slot.add` and `hardware.slot.edit` reply.
fn slot_set(s: &Session, slot: HardwareSlot, preview: bool, cmd: &str) -> Result<HardwareSlotSet> {
    if !ANGLES.contains(&slot.angle) {
        return Err(bad(cmd, "`angle` must be 0, 90, 180 or 270"));
    }
    for (what, text) in [("along", Some(slot.along.as_str())), ("across", Some(slot.across.as_str()).filter(|t| !is_middle(t)))] {
        if let Some(text) = text {
            length(s, text).map_err(|e| bad(cmd, format!("`{what}`: {e}")))?;
        }
    }
    let active = match slot.when.as_deref() {
        Some(w) => condition(s, w).map_err(|e| bad(cmd, format!("`when` must be a condition, such as FFH > 1200 mm: {e}")))?,
        None => true,
    };
    let w = work_out(s, &slot).map_err(|e| bad(cmd, e))?;
    Ok(HardwareSlotSet { slot, frame: w.frame, active, joint_a: w.joint_a, values: w.values, limits: w.limits, preview, param_warnings: Vec::new() })
}

/// An expression parameter's text, or `default`.
fn expression(e: Option<crate::schema::LengthExpr>, default: &str) -> String {
    e.map_or_else(|| default.to_owned(), |e| e.0.as_str().trim().to_owned())
}

fn slot_add(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "hardware.slot.add";
    let r: HardwareSlotAddParams = serde_json::from_value(p.clone()).map_err(|e| bad(cmd, e.to_string()))?;
    let store = store(s, cmd)?;
    let mut slots = raw_slots(&store);
    if slots.len() >= MAX_SLOTS && !r.preview {
        return Err(bad(cmd, format!("a design keeps at most {MAX_SLOTS} placement slots")));
    }
    let label = slot_text(cmd, "label", &r.label, true)?;
    let group = slot_text(cmd, "group", &r.group, false)?;
    let taken: Vec<&str> = slots.iter().filter_map(raw_id).collect();
    let id = match r.id.as_deref().map(str::trim) {
        Some(id) if !valid_id(id) => return Err(bad(cmd, "`id` must be 1 to 64 of a-z, 0-9, - and _")),
        Some(id) if taken.contains(&id) => return Err(bad(cmd, format!("there is already a placement slot `{id}`"))),
        Some(id) => id.to_owned(),
        None => unique_id(&slug(&label), &taken),
    };
    let origin = pick_origin(s, r.edge, r.corner, r.on, r.occurrence, cmd)?;
    let slot = HardwareSlot {
        id,
        label,
        group,
        origin,
        along: expression(r.along, "0"),
        across: expression(r.across, MIDDLE),
        angle: r.angle.unwrap_or(0),
        flip: r.flip.unwrap_or(false),
        when: condition_text(cmd, r.when.as_deref())?,
    };
    let set = slot_set(s, slot, r.preview, cmd)?;
    if !r.preview {
        slots.push(serde_json::to_value(&set.slot).map_err(|e| bad(cmd, e.to_string()))?);
        save_slots(s, store, slots, cmd)?;
    }
    reply(cmd, set)
}

fn slot_edit(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "hardware.slot.edit";
    let r: HardwareSlotEditParams = serde_json::from_value(p.clone()).map_err(|e| bad(cmd, e.to_string()))?;
    let store = store(s, cmd)?;
    let mut slots = raw_slots(&store);
    let index = slots.iter().position(|v| raw_id(v) == Some(r.id.as_str())).ok_or_else(|| bad(cmd, format!("no placement slot `{}`", r.id)))?;
    let kept = slots.get(index).cloned().unwrap_or_default();
    let mut slot: HardwareSlot = serde_json::from_value(kept).map_err(|e| bad(cmd, format!("the placement slot `{}` can't be read: {e}", r.id)))?;
    if let Some(label) = &r.label {
        slot.label = slot_text(cmd, "label", label, true)?;
    }
    if let Some(group) = &r.group {
        slot.group = slot_text(cmd, "group", group, false)?;
    }
    if r.along.is_some() {
        slot.along = expression(r.along, "0");
    }
    if r.across.is_some() {
        slot.across = expression(r.across, MIDDLE);
    }
    if let Some(angle) = r.angle {
        slot.angle = angle;
    }
    if let Some(flip) = r.flip {
        slot.flip = flip;
    }
    if let Some(when) = &r.when {
        slot.when = condition_text(cmd, when.as_deref())?;
    }
    match (r.edge, r.corner) {
        (Some(edge), Some(corner)) => slot.origin = pick_origin(s, edge, corner, r.on, r.occurrence, cmd)?,
        (None, None) if r.on.is_none() && r.occurrence.is_none() => {}
        _ => return Err(bad(cmd, "pick the corner again with both `edge` and `corner` (and `on?`, `occurrence?`)")),
    }
    let set = slot_set(s, slot, r.preview, cmd)?;
    if !r.preview {
        let value = serde_json::to_value(&set.slot).map_err(|e| bad(cmd, e.to_string()))?;
        if let Some(entry) = slots.get_mut(index) {
            *entry = value;
        }
        save_slots(s, store, slots, cmd)?;
    }
    reply(cmd, set)
}

fn slot_delete(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "hardware.slot.delete";
    let r: HardwareSlotDeleteParams = serde_json::from_value(p.clone()).map_err(|e| bad(cmd, e.to_string()))?;
    let store = store(s, cmd)?;
    let mut slots = raw_slots(&store);
    let index = slots.iter().position(|v| raw_id(v) == Some(r.id.as_str())).ok_or_else(|| bad(cmd, format!("no placement slot `{}`", r.id)))?;
    slots.remove(index);
    let left = slots.len();
    save_slots(s, store, slots, cmd)?;
    reply(cmd, HardwareSlotDeleted { id: r.id, slots: left, param_warnings: Vec::new() })
}

fn slot_list(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "hardware.slot.list";
    let r: HardwareSlotListParams = serde_json::from_value(p.clone()).map_err(|e| bad(cmd, e.to_string()))?;
    let store = store(s, cmd)?;
    let mut slots = Vec::new();
    let mut unreadable = Vec::new();
    for raw in raw_slots(&store) {
        match serde_json::from_value::<HardwareSlot>(raw.clone()) {
            Ok(slot) if r.group.as_deref().is_some_and(|g| g != slot.group) => {}
            Ok(slot) => slots.push(listed(s, slot)),
            Err(e) => unreadable.push(format!("{}: {e}", raw_id(&raw).unwrap_or("?"))),
        }
    }
    reply(cmd, HardwareSlotList { slots, unreadable, param_warnings: Vec::new() })
}

#[cfg(test)]
#[path = "hardware_tests.rs"]
mod tests;
