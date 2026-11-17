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

#[cfg(test)]
#[path = "hardware_tests.rs"]
mod tests;
