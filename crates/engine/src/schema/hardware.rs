//! Contracts and result types for the commands in `cmd/hardware.rs`: the hardware library, a
//! folder of window-hardware parts (STEP files) and profiles (DXF sections or SolidCraft designs),
//! one group per subfolder, each part with an optional data card (`<file stem>.json`) beside it.
//! The handlers read these same types; like them, a contract refuses unknown keys.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// List the parts in a hardware library folder, without changing the design.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HardwareLibraryListParams {
    /// The library folder. Each subfolder is a group (one level deep); files directly in the
    /// folder belong to no group (`""`). STEP (`.step`, `.stp`) and DXF (`.dxf`) files are parts
    /// everywhere; SolidCraft designs (`.sc`, and `.json` that is no other part's card) only as
    /// profiles: in a group folder named `Profil` or `Profile`, or (`.sc`) with a card that says so.
    pub folder: String,
}

/// What a part's file holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum HardwareKind {
    /// A STEP model (`.step`, `.stp`).
    Step,
    /// A DXF drawing (`.dxf`), such as a profile's cross-section.
    Dxf,
    /// A SolidCraft design (`.sc`, an older design extension, or `.json`).
    Sc,
}

/// What a part is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum HardwareRole {
    /// A fitting placed on a frame or sash: a hinge, a corner drive, a lock.
    Hardware,
    /// A profile whose section builds a frame or sash.
    Profile,
}

/// Where a part connects, in the part's own coordinates: a point and two directions (each
/// coordinate finite and under 1e6 in size). `z` and `x` are not zero and not parallel; saved,
/// `z` is a unit vector and `x` the unit vector square to it in the plane of the two.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HardwareConnection {
    /// The connection point.
    pub origin: [f64; 3],
    /// The direction the part's connection faces.
    pub z: [f64; 3],
    /// The direction along the part at the connection.
    pub x: [f64; 3],
}

/// A part's data card, as its `<file stem>.json` holds it. Keys other than these are kept as they
/// are when the card is changed.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HardwareCard {
    /// The part number (at most 256 characters, no control characters).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part_no: Option<String>,
    /// What the part is (at most 256 characters, no control characters).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// What the part is for. Without it, a file in a group folder named `Profil` or `Profile` is a
    /// profile, any other a hardware part.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<HardwareRole>,
    /// Where the part connects, in its own coordinates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection: Option<HardwareConnection>,
}

/// What `hardware.library.list` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct HardwareLibrary {
    /// The folder listed, as given.
    pub folder: String,
    /// The group folders, sorted by name, each with how many parts it holds; the ungrouped files
    /// (`""`) come first when there are any.
    pub groups: Vec<HardwareGroup>,
    /// The parts: by group, then by name.
    pub parts: Vec<HardwarePart>,
    /// More than 64 group folders or 2000 parts: the rest were left out.
    pub truncated: bool,
    /// In the warn parameter-check mode: what the call's parameters got wrong.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub param_warnings: Vec<String>,
}

/// A group: a subfolder of the library.
#[derive(Debug, Serialize, JsonSchema)]
pub struct HardwareGroup {
    /// The folder's name (`""`: the files directly in the library folder).
    pub name: String,
    /// How many parts it holds.
    pub parts: usize,
}

/// A part of the library: one STEP or DXF file.
#[derive(Debug, Serialize, JsonSchema)]
pub struct HardwarePart {
    /// The file's name without its extension.
    pub name: String,
    /// The file's name.
    pub file: String,
    /// The file's path, as `file.insert_step` takes it.
    pub path: String,
    /// Its group's name (`""`: none).
    pub group: String,
    /// What the file holds.
    pub kind: HardwareKind,
    /// What the part is for: the card's `role`, else from its group's name.
    pub role: HardwareRole,
    /// Its data card; `null` when it has none or it could not be read.
    pub card: Option<HardwareCard>,
    /// Why its data card could not be read (the other parts are listed all the same).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_error: Option<String>,
    /// Where its data card is, or would be: `<file stem>.json` beside it; `null` for a `.json`
    /// design, which can have none.
    pub card_path: Option<String>,
}

/// Write a part's data card (`<file stem>.json` beside the part's file) atomically, merged with
/// the card already there. A key left out keeps its value; `null` or `""` removes it.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HardwareCardSetParams {
    /// The part's file: a `.step`, `.stp`, `.dxf` or `.sc` file that exists.
    pub path: String,
    /// The part number (at most 256 characters, no control characters).
    #[serde(default, deserialize_with = "super::present")]
    #[schemars(with = "Option<String>")]
    pub part_no: Option<Option<String>>,
    /// What the part is (at most 256 characters, no control characters).
    #[serde(default, deserialize_with = "super::present")]
    #[schemars(with = "Option<String>")]
    pub description: Option<Option<String>>,
    /// What the part is for.
    #[serde(default, deserialize_with = "super::present")]
    #[schemars(with = "Option<HardwareRole>")]
    pub role: Option<Option<HardwareRole>>,
    /// Where the part connects, in its own coordinates.
    #[serde(default, deserialize_with = "super::present")]
    #[schemars(with = "Option<HardwareConnection>")]
    pub connection: Option<Option<HardwareConnection>>,
}

/// What `hardware.card.set` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct HardwareCardSet {
    /// The part's file.
    pub path: String,
    /// The data card written.
    pub card_path: String,
    /// The card as written.
    pub card: HardwareCard,
    /// In the warn parameter-check mode: what the call's parameters got wrong.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub param_warnings: Vec<String>,
}

/// A placement slot ("yerleşim noktası"): a named place on the sash where a part of a library
/// group goes, kept in the design's metadata under `tr.aksesuar.yerlesim`. Its frame starts at the
/// corner of an edge of the floor face (the profile face the hardware sits on), found again by name
/// as the design's parameters change; the part's connection point goes `along` the edge from the
/// corner and `across` the floor, turned `angle` about the floor's normal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HardwareSlot {
    /// A unique slug (`a-z`, `0-9`, `-`, `_`; at most 64 characters).
    pub id: String,
    /// The slot's name, as the user reads it.
    pub label: String,
    /// The library group whose parts go here (a group folder's name).
    pub group: String,
    /// The edge-corner joint origin the slot starts from: on the floor face, at the corner where
    /// the picked edge starts, with the face's and the edges' names.
    #[schemars(with = "serde_json::Value")]
    pub origin: solidcraft_doc::joints::JointOrigin,
    /// How far along the picked edge from the corner (mm): a number or an expression.
    pub along: String,
    /// How far across the floor face from the edge (mm): an expression, or `middle` for the
    /// middle of the floor face's width.
    pub across: String,
    /// The part's turn about the floor face's normal (deg): 0, 90, 180 or 270; at 0 the part's
    /// connection x runs along the edge, away from the corner.
    pub angle: u16,
    /// The part's connection z along the floor's normal instead of against it (the joint's flip).
    #[serde(default)]
    pub flip: bool,
    /// The condition the slot is in use under (in use while it is not 0, such as `FFH > 1200 mm`);
    /// `null`: always.
    #[serde(default)]
    pub when: Option<String>,
}

/// A frame in the world: its origin and its unit axes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HardwareSlotFrame {
    /// The origin (mm).
    pub origin: [f64; 3],
    /// The unit x axis.
    pub x: [f64; 3],
    /// The unit y axis (z × x).
    pub y: [f64; 3],
    /// The unit z axis.
    pub z: [f64; 3],
}

/// Add a placement slot to the design (one undo step), or, with `preview`, work out what it would
/// be without adding it.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HardwareSlotAddParams {
    /// The slot's name (1 to 256 characters, no control characters).
    pub label: String,
    /// The library group whose parts go here (at most 256 characters).
    pub group: String,
    /// A point on the picked edge (world): a straight edge of the floor face.
    pub edge: [f64; 3],
    /// The corner the slot is measured from (world): the edge's end nearest it.
    pub corner: [f64; 3],
    /// A point on the floor face (world). Default: the larger planar face beside the edge.
    #[serde(default)]
    pub on: Option<[f64; 3]>,
    /// The occurrence holding the floor face (0: the design's own bodies). Default: the one at
    /// `edge`.
    #[serde(default)]
    pub occurrence: Option<u64>,
    /// Along the edge from the corner (default `0`).
    #[serde(default)]
    pub along: Option<super::types::LengthExpr>,
    /// Across the floor face (default `middle`).
    #[serde(default)]
    pub across: Option<super::types::LengthExpr>,
    /// 0, 90, 180 or 270 (default 0).
    #[serde(default)]
    pub angle: Option<u16>,
    /// The joint's flip (default false).
    #[serde(default)]
    pub flip: Option<bool>,
    /// The condition (default: always in use; `""` too).
    #[serde(default)]
    pub when: Option<String>,
    /// The slot's id (default: made from the label).
    #[serde(default)]
    pub id: Option<String>,
    /// Work it out without changing the design.
    #[serde(default)]
    pub preview: bool,
}

/// Change a placement slot (one undo step), or, with `preview`, work out what it would be. A key
/// left out keeps its value; `edge` and `corner` (with `on?`, `occurrence?`) pick its corner again.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HardwareSlotEditParams {
    /// The slot's id.
    pub id: String,
    /// The slot's name.
    #[serde(default)]
    pub label: Option<String>,
    /// The library group.
    #[serde(default)]
    pub group: Option<String>,
    /// Along the edge from the corner.
    #[serde(default)]
    pub along: Option<super::types::LengthExpr>,
    /// Across the floor face, or `middle`.
    #[serde(default)]
    pub across: Option<super::types::LengthExpr>,
    /// 0, 90, 180 or 270.
    #[serde(default)]
    pub angle: Option<u16>,
    /// The joint's flip.
    #[serde(default)]
    pub flip: Option<bool>,
    /// The condition; `null` or `""`: always in use.
    #[serde(default, deserialize_with = "super::present")]
    #[schemars(with = "Option<String>")]
    pub when: Option<Option<String>>,
    /// A point on the edge picked again (world), with `corner`.
    #[serde(default)]
    pub edge: Option<[f64; 3]>,
    /// The corner picked again (world), with `edge`.
    #[serde(default)]
    pub corner: Option<[f64; 3]>,
    /// A point on the floor face picked again (world).
    #[serde(default)]
    pub on: Option<[f64; 3]>,
    /// The occurrence holding the floor face picked again.
    #[serde(default)]
    pub occurrence: Option<u64>,
    /// Work it out without changing the design.
    #[serde(default)]
    pub preview: bool,
}

/// Remove a placement slot (one undo step).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HardwareSlotDeleteParams {
    /// The slot's id.
    pub id: String,
}

/// List the design's placement slots, without changing it.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HardwareSlotListParams {
    /// Only the slots of this library group.
    #[serde(default)]
    pub group: Option<String>,
}

/// A slot as `hardware.slot.add` and `hardware.slot.edit` leave it, with where it is now.
#[derive(Debug, Serialize, JsonSchema)]
pub struct HardwareSlotSet {
    /// The slot as kept.
    pub slot: HardwareSlot,
    /// The slot's frame in the world (as `hardware.slot.list` gives it).
    pub frame: HardwareSlotFrame,
    /// Whether its condition holds now.
    pub active: bool,
    /// `joint.create`'s side `a` that puts a part there (as `hardware.slot.list` gives it).
    pub joint_a: serde_json::Value,
    /// The planar joint's values (as `hardware.slot.list` gives them).
    pub values: Vec<serde_json::Value>,
    /// The planar joint's limits (as `hardware.slot.list` gives them).
    pub limits: Vec<serde_json::Value>,
    /// Worked out only: the design is unchanged.
    pub preview: bool,
    /// In the warn parameter-check mode: what the call's parameters got wrong.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub param_warnings: Vec<String>,
}

/// What `hardware.slot.delete` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct HardwareSlotDeleted {
    /// The slot removed.
    pub id: String,
    /// How many slots are left.
    pub slots: usize,
    /// In the warn parameter-check mode: what the call's parameters got wrong.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub param_warnings: Vec<String>,
}

/// One slot of `hardware.slot.list`: as kept, and where it is now.
#[derive(Debug, Serialize, JsonSchema)]
pub struct HardwareSlotListed {
    /// The slot as kept.
    #[serde(flatten)]
    pub slot: HardwareSlot,
    /// Its condition holds now (and nothing is wrong with it).
    pub active: bool,
    /// Its frame in the world after `along`, `across` and `angle`: the origin where the part's
    /// connection point goes, z out of the floor face, x the way the part's connection x runs (a
    /// part mated there has its connection z along -z, along z when `flip`). `null` with `error`.
    pub frame: Option<HardwareSlotFrame>,
    /// Why the slot can't be placed now (its edge or face gone, an expression that no longer
    /// evaluates); the other slots are listed all the same.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// `joint.create`'s side `a` for a planar joint that puts a part on the slot: the edge-corner
    /// origin as world points (`occurrence`, `edge`, `corner`, `on`).
    pub joint_a: Option<serde_json::Value>,
    /// The planar joint's values: the shift along a's x and y (one of them `along`, kept as an
    /// expression, the other `across`) and the turn (deg).
    pub values: Option<Vec<serde_json::Value>>,
    /// The planar joint's limits: `across` (unless an expression) and the turn held.
    pub limits: Option<Vec<serde_json::Value>>,
    /// The part `hardware.place` put there, while its occurrence is in the design.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placed: Option<HardwarePlacement>,
}

/// A part `hardware.place` put on a slot, as the design's slot list keeps it (`placed`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HardwarePlacement {
    /// The slot's id.
    pub slot: String,
    /// The part's STEP file.
    pub path: String,
    /// The component the file came in as (shared by every placement of that file).
    pub component: u64,
    /// The part's occurrence on this slot.
    pub occurrence: u64,
    /// The planar joint holding it there.
    pub joint: u64,
}

/// Put a library part on a placement slot (one undo step), or, with `preview`, work out where it
/// would go without changing the design.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HardwarePlaceParams {
    /// The slot's id: one in use now, with nothing on it.
    pub slot: String,
    /// The part's STEP file (`.step`, `.stp`); its data card must give `connection`.
    pub path: String,
    /// Work it out without changing the design.
    #[serde(default)]
    pub preview: bool,
}

/// What `hardware.place` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct HardwarePlaced {
    /// The slot.
    pub slot: String,
    /// The part's file.
    pub path: String,
    /// The part's name: its card's part number, else its file's name.
    pub name: String,
    /// Where the part's occurrence goes in the world: a row-major 4x4 placement.
    pub transform: [[f64; 4]; 4],
    /// The slot's frame in the world (as `hardware.slot.list` gives it): the part's connection
    /// point goes on its origin, its connection x along x and its connection z against z (along it
    /// when the slot flips).
    pub frame: HardwareSlotFrame,
    /// The part's component; `null` for a preview.
    pub component: Option<u64>,
    /// The part's new occurrence; `null` for a preview.
    pub occurrence: Option<u64>,
    /// The planar joint holding it on the slot; `null` for a preview.
    pub joint: Option<u64>,
    /// The file was placed before: this is another occurrence of the same component.
    pub reused: bool,
    /// Worked out only: the design is unchanged.
    pub preview: bool,
    /// What the joint reported, or that the part did not come to the slot.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    /// In the warn parameter-check mode: what the call's parameters got wrong.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub param_warnings: Vec<String>,
}

/// Take the part `hardware.place` put on a slot away again (one undo step).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HardwareUnplaceParams {
    /// The slot's id.
    pub slot: String,
}

/// What `hardware.unplace` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct HardwareUnplaced {
    /// The slot, now free.
    pub slot: String,
    /// The occurrence removed (already gone when the part was deleted by hand).
    pub occurrence: u64,
    /// The joint removed with it.
    pub joint: u64,
    /// The component went too: no other placement used it.
    pub component_removed: bool,
    /// In the warn parameter-check mode: what the call's parameters got wrong.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub param_warnings: Vec<String>,
}

/// What `hardware.slot.list` returns.
#[derive(Debug, Serialize, JsonSchema)]
pub struct HardwareSlotList {
    /// The slots, in the order they were added.
    pub slots: Vec<HardwareSlotListed>,
    /// Entries of the design's slot list this version can't read (left as they are).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unreadable: Vec<String>,
    /// In the warn parameter-check mode: what the call's parameters got wrong.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub param_warnings: Vec<String>,
}

#[cfg(test)]
#[path = "hardware_tests.rs"]
mod tests;
