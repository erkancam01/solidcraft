//! Placement slots in the app ("yerleşim noktaları", `hardware.slot.*`): named places on the sash
//! where a part of a library group goes. The Hardware Library lists them in its Placement slots
//! section, grouped by library group, with Edit and Delete; Add slot… and Edit start a picking
//! mode like the connection point's (`hardware_connection`): a click picks faces and edges only,
//! a face is the floor face (the profile face the hardware sits on) and an edge a straight edge of
//! it, whose end nearest the click is the corner the slot is measured from. A small card asks for
//! the name, the group, how far along the edge and across the floor, the turn, the flip and an
//! optional condition, and shows where the slot comes to (`hardware.slot.add`/`edit` with
//! `preview`) as a triad in the view. Save adds or changes the slot in one undo step; Cancel and
//! Esc change nothing.
//!
//! While the library is open every slot is drawn in the view as a dot (the theme's accent, dimmed
//! while its condition does not hold), its name shown on hover; the slot being edited or picked
//! in the list is ringed. [`markers`] and [`marker_at`] give the dots' screen positions and the
//! one under a point; `hardware_place` draws the slots a part goes in instead while it is placed.
//! Slots at the same spot (a corner's corner drive and its hinge) share one name tag on hover.

use egui::{RichText, Stroke, vec2};
use serde_json::{Map, Value, json};
use solidcore_record::Vec3;
use solidcraft_engine::Sel;
use solidcraft_engine::geom::prelude::*;
use solidcraft_theme::Tokens;
use solidcraft_ui_text::tr;

use crate::SolveApp;
use crate::hardware_connection::{Frame, Picking};
use crate::ui_kit;
use crate::ui_kit::BackingTooltipExt as _;
use crate::viewport::Proj;

/// The turns a slot takes, as the Angle list offers them.
const ANGLES: [u16; 4] = [0, 90, 180, 270];
const ANGLE_LABELS: [&str; 4] = ["0°", "90°", "180°", "270°"];
/// How near a marker the pointer must be (points) to be over it.
pub(crate) const MARKER_REACH: f32 = 8.0;
/// A marker's radius, and the ring round the one being edited or picked in the list.
const MARKER_RADIUS: f32 = 4.0;
const RING_RADIUS: f32 = 8.0;
/// Markers this near each other (points) are at one spot: one name tag names them all.
const SAME_SPOT: f32 = 3.0;

/// A slot as the panel and the view show it, from `hardware.slot.list`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Slot {
    pub id: String,
    pub label: String,
    pub group: String,
    pub along: String,
    pub across: String,
    pub angle: u16,
    pub flip: bool,
    pub when: Option<String>,
    /// Its condition holds now, and nothing is wrong with it.
    pub active: bool,
    /// Why it can't be placed now.
    pub error: Option<String>,
    /// Where the part's connection frame goes (world): `None` with `error`.
    pub frame: Option<Frame>,
    /// The part `hardware.place` put there: its occurrence and its joint.
    pub placed: Option<(u64, u64)>,
}

/// The design's slots, listed again whenever the design changes.
#[derive(Default)]
pub(crate) struct Listed {
    /// The design they were listed for: its session, revision and document.
    key: Option<(u64, u64, usize)>,
    pub slots: Vec<Slot>,
    /// Why they could not be listed.
    pub error: Option<String>,
    /// The slot picked in the list (its id), ringed in the view.
    pub selected: Option<String>,
}

fn vec3(v: &Value) -> Option<Vec3> {
    let a = v.as_array()?;
    match a.as_slice() {
        [x, y, z] => Some(Vec3::new(x.as_f64()?, y.as_f64()?, z.as_f64()?)),
        _ => None,
    }
}

/// A frame as the slot commands give it: `{origin, x, y, z}`.
fn frame_of(v: &Value) -> Option<Frame> {
    Some(Frame { origin: vec3(&v["origin"])?, z: vec3(&v["z"])?.normalized()?, x: vec3(&v["x"])?.normalized()? })
}

fn slot_of(v: &Value) -> Option<Slot> {
    let text = |key: &str| v[key].as_str().map(str::to_owned);
    Some(Slot {
        id: text("id")?,
        label: text("label").unwrap_or_default(),
        group: text("group").unwrap_or_default(),
        along: text("along").unwrap_or_default(),
        across: text("across").unwrap_or_default(),
        angle: v["angle"].as_u64().and_then(|a| u16::try_from(a).ok()).unwrap_or(0),
        flip: v["flip"].as_bool().unwrap_or(false),
        when: text("when").filter(|w| !w.trim().is_empty()),
        active: v["active"].as_bool().unwrap_or(false),
        error: text("error"),
        frame: frame_of(&v["frame"]),
        placed: v["placed"]["occurrence"].as_u64().zip(v["placed"]["joint"].as_u64()),
    })
}

/// List the design's slots again when it changed since they were listed.
pub(crate) fn refresh(app: &mut SolveApp, listed: &mut Listed) {
    let key = |app: &SolveApp| (app.session.uid, app.session.revision, std::sync::Arc::as_ptr(&app.session.doc) as usize);
    if listed.key == Some(key(app)) {
        return;
    }
    match app.session.execute("hardware.slot.list", &json!({})) {
        Ok(r) => {
            listed.slots = r["slots"].as_array().map(|a| a.iter().filter_map(slot_of).collect()).unwrap_or_default();
            listed.error = None;
        }
        Err(e) => {
            listed.slots.clear();
            listed.error = Some(crate::friendly_error("hardware.slot.list", &e.to_string()));
        }
    }
    if listed.selected.as_ref().is_some_and(|id| !listed.slots.iter().any(|s| s.id == *id)) {
        listed.selected = None;
    }
    // Listing evaluates a stale model first: the key is the design as listed.
    listed.key = Some(key(app));
}

/// A slot's dot in the view.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Marker {
    pub id: String,
    pub at: egui::Pos2,
    pub active: bool,
}

/// The slots' dots where they are on screen now (a slot that can't be placed, or lies behind the
/// eye, has none).
pub(crate) fn markers(listed: &Listed, proj: &Proj) -> Vec<Marker> {
    listed
        .slots
        .iter()
        .filter_map(|s| {
            let frame = s.frame?;
            proj.to_screen(frame.origin).map(|at| Marker { id: s.id.clone(), at, active: s.active })
        })
        .collect()
}

/// The marker under `p`: the nearest one within [`MARKER_REACH`].
pub(crate) fn marker_at(markers: &[Marker], p: egui::Pos2) -> Option<&Marker> {
    markers.iter().map(|m| (m, m.at.distance(p))).filter(|(_, d)| *d <= MARKER_REACH).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(m, _)| m)
}

/// The floor face as picked (world): the click on it and its normal out of the material.
#[derive(Clone, Debug)]
struct Floor {
    sel: Sel,
    body: String,
    on: Vec3,
    plane: Vec3,
    normal: Vec3,
}

/// The edge as picked (world): its end nearest the click (the corner) and its other end.
#[derive(Clone, Debug)]
struct EdgePick {
    sel: Sel,
    body: String,
    near: Vec3,
    far: Vec3,
}

/// The card's fields as typed.
#[derive(Clone, Debug, PartialEq)]
struct Fields {
    label: String,
    group: String,
    along: String,
    middle: bool,
    across: String,
    angle: usize,
    flip: bool,
    when: String,
}

/// What the slot comes to as the card stands: its frame and whether its condition holds.
type Preview = Result<(Frame, bool), String>;

/// The card's state while a slot is being added or edited.
pub(crate) struct Pick {
    /// The slot being edited (its id); `None` adds one.
    pub(crate) editing: Option<String>,
    session: u64,
    document: String,
    floor: Option<Floor>,
    edge: Option<EdgePick>,
    fields: Fields,
    /// The library's groups, for the Group list.
    groups: Vec<String>,
    /// Why the last pick was not taken, or Save was refused.
    refused: Option<String>,
    picking: Picking,
    /// The preview, with the parameters it was worked out for.
    preview: Option<(String, Preview)>,
}

impl Pick {
    /// The floor face and the edge, when they make a corner: the edge in the face's plane, on the
    /// same body.
    fn corner(&self) -> Option<(&Floor, &EdgePick)> {
        let (floor, edge) = (self.floor.as_ref()?, self.edge.as_ref()?);
        (floor.body == edge.body && crate::hardware_connection::frame_of(floor.plane, floor.normal, edge.near, edge.far, false).is_some())
            .then_some((floor, edge))
    }

    fn command(&self) -> &'static str {
        if self.editing.is_some() { "hardware.slot.edit" } else { "hardware.slot.add" }
    }

    /// The command's parameters for the card as it stands; `None` while a new slot has no corner.
    fn params(&self, preview: bool) -> Option<Value> {
        let f = &self.fields;
        let text = |t: &str, default: &str| if t.trim().is_empty() { default.to_owned() } else { t.trim().to_owned() };
        let mut p = Map::new();
        let label = f.label.trim();
        // A preview needs no name yet.
        p.insert("label".into(), json!(if label.is_empty() && preview { "?" } else { label }));
        p.insert("group".into(), json!(f.group));
        p.insert("along".into(), json!(text(&f.along, "0")));
        p.insert("across".into(), json!(if f.middle { "middle".to_owned() } else { text(&f.across, "0") }));
        p.insert("angle".into(), json!(ANGLES.get(f.angle).copied().unwrap_or(0)));
        p.insert("flip".into(), json!(f.flip));
        p.insert("when".into(), json!(f.when.trim()));
        match self.corner() {
            Some((floor, edge)) => {
                let mid = edge.near.lerp(edge.far, 0.5);
                p.insert("edge".into(), json!([mid.x, mid.y, mid.z]));
                p.insert("corner".into(), json!([edge.near.x, edge.near.y, edge.near.z]));
                p.insert("on".into(), json!([floor.on.x, floor.on.y, floor.on.z]));
            }
            None if self.editing.is_none() => return None,
            None => {}
        }
        if let Some(id) = &self.editing {
            p.insert("id".into(), json!(id));
        }
        if preview {
            p.insert("preview".into(), json!(true));
        }
        Some(Value::Object(p))
    }

    /// What is wrong with the picks or the card as they stand.
    fn problem(&self) -> Option<String> {
        if self.floor.is_some() && self.edge.is_some() && self.corner().is_none() {
            return Some(tr("hardware_slot.not_in_face").to_owned());
        }
        if let Some(refused) = &self.refused {
            return Some(refused.clone());
        }
        match &self.preview {
            Some((_, Err(e))) => Some(e.clone()),
            _ => None,
        }
    }

    /// Whether the preview's refusal is about the field the engine calls `key`.
    fn flags(&self, key: &str) -> bool {
        matches!(&self.preview, Some((_, Err(e))) if e.contains(&format!("`{key}`")) || e.contains(&format!("{key}:")))
    }
}

/// Whether a slot is being added or edited.
pub(crate) fn active(app: &SolveApp) -> bool {
    app.hardware_slot.is_some()
}

/// Start adding a slot (`slot` `None`, its group `group` when that is one of them) or editing
/// `slot`, its Group list from the library's `groups`. Refused while a sketch, an edit in place or
/// a connection point is open; an open dialog or tool is closed first.
pub(crate) fn start(app: &mut SolveApp, slot: Option<&Slot>, mut groups: Vec<String>, group: Option<&str>) -> Result<(), String> {
    if active(app) {
        return Ok(());
    }
    if crate::hardware_connection::active(app) {
        return Err(tr("hardware_connection.busy").to_owned());
    }
    if app.session.active_sketch.is_some() || app.edit_in_place.active.is_some() {
        return Err(tr("hardware_connection.finish_first").to_owned());
    }
    if app.dialog.is_some() {
        crate::dialogs::cancel(app);
    }
    app.tool = None;
    app.home.open = false;
    groups.retain(|g| !g.is_empty());
    if let Some(group) = slot.map(|s| s.group.clone()).filter(|g| !g.is_empty() && !groups.contains(g)) {
        groups.push(group);
    }
    let fields = match slot {
        Some(s) => Fields {
            label: s.label.clone(),
            group: s.group.clone(),
            along: s.along.clone(),
            middle: s.across.trim().eq_ignore_ascii_case("middle"),
            across: if s.across.trim().eq_ignore_ascii_case("middle") { "0".to_owned() } else { s.across.clone() },
            angle: ANGLES.iter().position(|a| *a == s.angle).unwrap_or(0),
            flip: s.flip,
            when: s.when.clone().unwrap_or_default(),
        },
        None => Fields {
            label: String::new(),
            group: group.filter(|g| groups.iter().any(|x| x == g)).map(str::to_owned).or_else(|| groups.first().cloned()).unwrap_or_default(),
            along: "0".to_owned(),
            middle: true,
            across: "0".to_owned(),
            angle: 0,
            flip: false,
            when: String::new(),
        },
    };
    let picking = crate::hardware_connection::pick_faces_and_edges(app);
    app.session.selection.clear();
    app.hardware_slot = Some(Pick {
        editing: slot.map(|s| s.id.clone()),
        session: app.session.uid,
        document: app.session.document_id.clone(),
        floor: None,
        edge: None,
        fields,
        groups,
        refused: None,
        picking,
        preview: None,
    });
    Ok(())
}

/// The floor face a face pick stands for, or the message id saying why it is none.
fn floor(app: &SolveApp, sel: &Sel) -> Result<Floor, &'static str> {
    let Sel::Face { body, index, point } = sel else { return Err("hardware_slot.not_flat") };
    let (plane, normal) = crate::dialogs::planar_face(&app.session, body, *index).ok_or("hardware_slot.not_flat")?;
    let normal = normal.normalized().ok_or("hardware_slot.not_flat")?;
    Ok(Floor { sel: sel.clone(), body: body.clone(), on: *point, plane, normal })
}

/// The edge an edge pick stands for, or the message id saying why it is none.
fn edge(app: &SolveApp, sel: &Sel) -> Result<EdgePick, &'static str> {
    let Sel::Edge { body, index, point } = sel else { return Err("hardware_slot.not_in_face") };
    let world = crate::open_meshing::session_world(&app.session);
    let b = world.body(body).ok_or("hardware_slot.not_in_face")?;
    let (a, z) = crate::hardware_connection::straight_ends(b, *index).ok_or("hardware_slot.not_in_face")?;
    let (near, far) = if a.dist(*point) <= z.dist(*point) { (a, z) } else { (z, a) };
    Ok(EdgePick { sel: sel.clone(), body: body.clone(), near, far })
}

/// Take the faces and edges clicked since the last frame: a face is the floor face, an edge the
/// slot's edge. The selection then shows the two picks.
fn take_picks(app: &mut SolveApp) {
    let Some(mut pick) = app.hardware_slot.take() else { return };
    let picked: Vec<Sel> = pick.floor.iter().map(|f| f.sel.clone()).chain(pick.edge.iter().map(|e| e.sel.clone())).collect();
    let clicked: Vec<Sel> = app.session.selection.iter().filter(|sel| !picked.contains(sel)).cloned().collect();
    for sel in &clicked {
        match sel {
            Sel::Face { .. } => match floor(app, sel) {
                Ok(f) => {
                    pick.floor = Some(f);
                    pick.refused = None;
                }
                Err(id) => pick.refused = Some(tr(id).to_owned()),
            },
            Sel::Edge { .. } => match edge(app, sel) {
                Ok(e) => {
                    pick.edge = Some(e);
                    pick.refused = None;
                }
                Err(id) => pick.refused = Some(tr(id).to_owned()),
            },
            _ => {}
        }
    }
    let shown: Vec<Sel> = pick.floor.iter().map(|f| f.sel.clone()).chain(pick.edge.iter().map(|e| e.sel.clone())).collect();
    if app.session.selection != shown {
        app.session.selection = shown;
    }
    app.hardware_slot = Some(pick);
}

/// Work the preview out again when the card or the design changed since.
fn update_preview(app: &mut SolveApp, pick: &mut Pick) {
    let Some(params) = pick.params(true) else {
        pick.preview = None;
        return;
    };
    let key = format!("{}:{}:{params}", app.session.uid, app.session.revision);
    if pick.preview.as_ref().is_some_and(|(k, _)| *k == key) {
        return;
    }
    let cmd = pick.command();
    let preview = match app.session.execute(cmd, &params) {
        Ok(r) => frame_of(&r["frame"]).map(|f| (f, r["active"].as_bool().unwrap_or(true))).ok_or_else(|| format!("{cmd} gave no frame")),
        Err(e) => Err(crate::friendly_error(cmd, &e.to_string())),
    };
    pick.preview = Some((key, preview));
}

/// Stop picking: the pick filter back, the picks out of the selection.
fn finish(app: &mut SolveApp, pick: &Pick) {
    crate::hardware_connection::restore_picking(app, &pick.picking);
    app.session.selection.clear();
}

/// Save: the slot added or changed in one undo step. Refused (the card stays up, saying why)
/// when the engine refuses it.
fn save(app: &mut SolveApp) -> Result<(), String> {
    let Some(mut pick) = app.hardware_slot.take() else { return Ok(()) };
    let saved = match pick.params(false) {
        Some(params) => app.run(pick.command(), params),
        None => Err(tr("hardware_slot.edge_hint").to_owned()),
    };
    match saved {
        Ok(r) => {
            finish(app, &pick);
            if let (Some(d), Some(id)) = (app.hardware_library.as_mut(), r["slot"]["id"].as_str()) {
                d.slots.selected = Some(id.to_owned());
                // The next new slot starts in this group.
                d.last_group = r["slot"]["group"].as_str().filter(|g| !g.is_empty()).map(str::to_owned).or(d.last_group.take());
            }
            app.set_status(tr("hardware_slot.saved"), false);
            Ok(())
        }
        Err(e) => {
            pick.refused = Some(e.clone());
            app.hardware_slot = Some(pick);
            Err(e)
        }
    }
}

/// Cancel: nothing changes.
fn cancel(app: &mut SolveApp) {
    if let Some(pick) = app.hardware_slot.take() {
        finish(app, &pick);
    }
}

/// Every frame: keep the library's slot list current, and while a slot is being added or edited,
/// take the picks, work out the preview and show the card.
pub(crate) fn show(app: &mut SolveApp, ctx: &egui::Context) {
    if let Some(mut d) = app.hardware_library.take() {
        refresh(app, &mut d.slots);
        app.hardware_library = Some(d);
    }
    let Some(pick) = app.hardware_slot.as_ref() else { return };
    if app.session.uid != pick.session || app.session.document_id != pick.document {
        // Another design: its tab keeps its pick filter as it was.
        if let Some(pick) = app.hardware_slot.take() {
            crate::hardware_connection::restore_picking(app, &pick.picking);
        }
        return;
    }
    // A dialog, tool or sketch started meanwhile takes over the picks.
    if app.dialog.is_some() || app.tool.is_some() || app.session.active_sketch.is_some() {
        cancel(app);
        return;
    }
    take_picks(app);
    let Some(mut pick) = app.hardware_slot.take() else { return };
    update_preview(app, &mut pick);
    let named = !pick.fields.label.trim().is_empty();
    let valid = named && matches!(&pick.preview, Some((_, Ok(_)))) && pick.refused.is_none();
    let window = ui_kit::Window::new(egui::Id::unique("sc_hardware_slot_card"), "joint_origin", tr("hardware_slot.card_title"))
        .actions(tr("ui.save"), tr("ui.cancel"), None)
        .fields()
        .cards(1.25);
    let window = match crate::hardware_connection::card_corner(app) {
        Some(corner) => window.placed(egui::Align2::LEFT_TOP, corner),
        None => window.anchor(egui::Align2::CENTER_TOP, vec2(0.0, 140.0)),
    };
    let before = pick.fields.clone();
    let (closed, action) = window.show(ctx, valid, |ui| card(ui, &mut pick));
    if pick.fields != before {
        // A changed field: a refusal of the last Save no longer stands.
        pick.refused = None;
    }
    app.hardware_slot = Some(pick);
    match action {
        ui_kit::Action::Ok => {
            let _ = save(app);
        }
        ui_kit::Action::Cancel => cancel(app),
        _ if closed => cancel(app),
        _ => {}
    }
}

/// The card's body: the two picks and what each wants, the slot's fields, and what is wrong.
fn card(ui: &mut egui::Ui, pick: &mut Pick) {
    for (label, hint, picked) in [
        ("hardware_slot.floor", "hardware_slot.floor_hint", pick.floor.is_some()),
        ("hardware_slot.edge", "hardware_slot.edge_hint", pick.edge.is_some()),
    ] {
        ui_kit::text(ui, tr(label), ui_kit::Text::Heading(label));
        let note = if picked { format!("✓ {}", tr("hardware_connection.picked")) } else { tr(hint).to_owned() };
        ui_kit::text(ui, &note, ui_kit::Text::Note(hint));
    }
    if pick.editing.is_some() && pick.corner().is_none() {
        ui_kit::text(ui, tr("hardware_slot.keep_corner"), ui_kit::Text::Note("hardware_slot.keep_corner"));
    }
    let (flag_along, flag_across, flag_when) = (pick.flags("along"), pick.flags("across"), pick.flags("when"));
    let f = &mut pick.fields;
    ui_kit::row(ui, tr("hardware_slot.name"), |ui| ui_kit::text_field(ui, &mut f.label, ""));
    if pick.groups.is_empty() {
        ui_kit::row(ui, tr("hardware_slot.group"), |ui| ui_kit::text_field(ui, &mut f.group, ""));
    } else {
        let options: Vec<&str> = pick.groups.iter().map(String::as_str).collect();
        let mut index = options.iter().position(|g| *g == f.group).unwrap_or(0);
        // A folder's name is shown as written, never looked up as an English label.
        let verbatim = solidcraft_ui_text::resolved_labels();
        ui_kit::row(ui, tr("hardware_slot.group"), |ui| ui_kit::dropdown(ui, "hardware_slot_group", &mut index, &options));
        drop(verbatim);
        if let Some(group) = options.get(index) {
            f.group = (*group).to_owned();
        }
    }
    ui_kit::row(ui, tr("hardware_slot.along"), |ui| ui_kit::checked_field(ui, &mut f.along, "0", flag_along).0);
    ui_kit::row(ui, tr("hardware_slot.across"), |ui| ui_kit::checkbox(ui, tr("hardware_slot.middle"), &mut f.middle));
    if !f.middle {
        ui_kit::row(ui, "", |ui| ui_kit::checked_field(ui, &mut f.across, "0", flag_across).0);
    }
    ui_kit::row(ui, tr("hardware_slot.angle"), |ui| ui_kit::dropdown(ui, "hardware_slot_angle", &mut f.angle, &ANGLE_LABELS));
    ui_kit::checkbox(ui, tr("hardware_slot.flip"), &mut f.flip);
    ui_kit::row(ui, tr("hardware_slot.when"), |ui| ui_kit::checked_field(ui, &mut f.when, tr("hardware_slot.when_hint"), flag_when).0);
    if let Some(problem) = pick.problem() {
        ui_kit::status_line(ui, &problem, ui_kit::StatusTone::Error);
    } else if matches!(&pick.preview, Some((_, Ok((_, false))))) {
        ui_kit::status_line(ui, tr("hardware_slot.inactive"), ui_kit::StatusTone::Dim);
    }
}

/// What a slot row or the section's button asked for.
pub(crate) enum SlotAction {
    Add,
    Edit(String),
    Delete(String),
    Select(String),
}

/// The library panel's Placement slots section: the slots by group, each with its edit and delete
/// buttons, and Add slot…. `busy` (a slot or a connection point being picked) waits them all.
pub(crate) fn section(ui: &mut egui::Ui, listed: &Listed, busy: bool) -> Option<SlotAction> {
    let t = Tokens::get();
    let mut action = None;
    crate::ui_kit::section_count_as_written(ui, "hardware-library-slots", tr("hardware_slot.title"), true, Some(listed.slots.len()), |ui| {
        // First, so it shows without scrolling past the slots.
        let add = crate::ui_kit::button(ui, tr("hardware_slot.add"), crate::ui_kit::Button::Enabled(!busy));
        crate::panel_runtime::register_content_focus(ui.ctx(), &add);
        if add.clicked() && !busy {
            action = Some(SlotAction::Add);
        }
        if let Some(error) = &listed.error {
            ui.label(RichText::new(error.as_str()).color(t.error));
        }
        let mut groups: Vec<&str> = listed.slots.iter().map(|s| s.group.as_str()).collect();
        groups.sort_unstable();
        groups.dedup();
        for group in groups {
            let title = if group.is_empty() { tr("hardware_library.ungrouped") } else { group };
            ui.label(RichText::new(title).small().strong().color(t.text_dim));
            for slot in listed.slots.iter().filter(|s| s.group == group) {
                if let Some(a) = slot_row(ui, slot, listed.selected.as_deref() == Some(slot.id.as_str()), busy) {
                    action = Some(a);
                }
            }
        }
    });
    action
}

/// One slot: its name and how far along the edge, whether its condition holds or what is wrong
/// with it, and its edit and delete buttons.
fn slot_row(ui: &mut egui::Ui, slot: &Slot, selected: bool, busy: bool) -> Option<SlotAction> {
    let t = Tokens::get();
    let mut action = None;
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let delete = crate::ui_kit::icon_action(ui, tr("hardware_slot.delete"), "delete", !busy, true);
            crate::panel_runtime::register_content_focus(ui.ctx(), &delete);
            if delete.clicked() && !busy {
                action = Some(SlotAction::Delete(slot.id.clone()));
            }
            let edit = crate::ui_kit::icon_action(ui, tr("hardware_slot.edit"), "rename", !busy, false);
            crate::panel_runtime::register_content_focus(ui.ctx(), &edit);
            if edit.clicked() && !busy {
                action = Some(SlotAction::Edit(slot.id.clone()));
            }
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                let ink = match (&slot.error, slot.active, selected) {
                    (Some(_), ..) => t.error,
                    (None, _, true) => t.accent,
                    (None, true, false) => t.text,
                    (None, false, false) => t.text_dim,
                };
                let text = format!("{} · {}", slot.label, slot.along);
                let label = ui.add(egui::Label::new(RichText::new(text).color(ink)).truncate().sense(egui::Sense::click()));
                let label = match &slot.error {
                    Some(error) => label.backing_on_hover_text(error),
                    None if !slot.active => label.backing_on_hover_text(tr("hardware_slot.inactive")),
                    None => label,
                };
                if label.clicked() {
                    action = Some(SlotAction::Select(slot.id.clone()));
                }
            });
        });
    });
    if slot.error.is_none() && !slot.active {
        ui.label(RichText::new(tr("hardware_slot.inactive")).small().color(t.text_dim));
    }
    action
}

/// Draw the slots while the library is open (a dot each, the one being edited or picked in the
/// list ringed, a name on hover), and the card's preview triad.
pub(crate) fn overlay(app: &SolveApp, painter: &egui::Painter, proj: &Proj) {
    let t = Tokens::get();
    // While a part is placed, `hardware_place` draws the slots it can go in.
    if let Some(d) = app.hardware_library.as_ref().filter(|d| d.placing.is_none()) {
        let listed = &d.slots;
        let marks = markers(listed, proj);
        let editing = app.hardware_slot.as_ref().and_then(|p| p.editing.as_deref());
        for m in &marks {
            let fill = if m.active { t.accent } else { t.accent.gamma_multiply(0.35) };
            painter.circle(m.at, MARKER_RADIUS, fill, Stroke::new(1.0, t.panel));
            if editing == Some(m.id.as_str()) || listed.selected.as_deref() == Some(m.id.as_str()) {
                painter.circle_stroke(m.at, RING_RADIUS, Stroke::new(1.5, t.accent));
            }
        }
        let hover = painter.ctx().pointer_hover_pos().filter(|p| !crate::shell::covered(app, *p));
        if let Some(m) = hover.and_then(|p| marker_at(&marks, p)) {
            // Slots at the same spot (a corner drive and a hinge at one corner) share the tag.
            let text: Vec<String> = marks
                .iter()
                .filter(|o| o.at.distance(m.at) <= SAME_SPOT)
                .filter_map(|o| listed.slots.iter().find(|s| s.id == o.id))
                .map(|slot| {
                    let mut text = format!("{} · {}", slot.label, slot.along);
                    if !slot.active {
                        text.push_str(" — ");
                        text.push_str(slot.error.as_deref().unwrap_or(tr("hardware_slot.inactive")));
                    }
                    text
                })
                .collect();
            if !text.is_empty() {
                name_tag(painter, m.at, &text.join(" / "));
            }
        }
    }
    if let Some(pick) = &app.hardware_slot {
        let len = crate::hardware_connection::triad_length(app);
        match &pick.preview {
            Some((_, Ok((frame, _)))) => crate::hardware_connection::paint_triad(painter, proj, frame, len),
            _ => {
                if let Some(floor) = &pick.floor
                    && let (Some(a), Some(b)) = (proj.to_screen(floor.on), proj.to_screen(floor.on + floor.normal * len))
                {
                    painter.line_segment([a, b], Stroke::new(2.5, t.axis_z));
                }
            }
        }
    }
}

/// A slot's name beside its dot, on the panel colour.
pub(crate) fn name_tag(painter: &egui::Painter, at: egui::Pos2, text: &str) {
    let t = Tokens::get();
    let galley = painter.layout_no_wrap(text.to_owned(), egui::FontId::proportional(t.font_small), t.text);
    let pad = vec2(t.space_4, t.space_2);
    let min = at + vec2(RING_RADIUS + t.space_4, -galley.size().y / 2.0 - pad.y);
    let rect = egui::Rect::from_min_size(min, galley.size() + pad * 2.0);
    painter.rect_filled(rect, t.radius_sm, t.panel);
    painter.rect_stroke(rect, t.radius_sm, Stroke::new(1.0, t.control_border), egui::StrokeKind::Inside);
    painter.galley(rect.min + pad, galley, t.text);
}

#[cfg(test)]
mod tests {
    use super::*;
    use solidcraft_engine::Session;

    fn listed(slots: &[(&str, Vec3, bool)]) -> Listed {
        Listed {
            slots: slots
                .iter()
                .map(|(id, at, active)| Slot {
                    id: (*id).to_owned(),
                    label: (*id).to_owned(),
                    group: "Köşe aktarma".to_owned(),
                    along: "0".to_owned(),
                    across: "middle".to_owned(),
                    angle: 0,
                    flip: false,
                    when: None,
                    active: *active,
                    error: None,
                    frame: Some(Frame { origin: *at, z: Vec3::X, x: Vec3::Z }),
                    placed: None,
                })
                .collect(),
            ..Listed::default()
        }
    }

    #[test]
    fn the_marker_under_a_point_is_the_nearest_within_reach() {
        let marks = [
            Marker { id: "a".into(), at: egui::pos2(100.0, 100.0), active: true },
            Marker { id: "b".into(), at: egui::pos2(106.0, 100.0), active: false },
        ];
        assert_eq!(marker_at(&marks, egui::pos2(104.0, 100.0)).map(|m| m.id.as_str()), Some("b"));
        assert_eq!(marker_at(&marks, egui::pos2(99.0, 101.0)).map(|m| m.id.as_str()), Some("a"));
        assert!(marker_at(&marks, egui::pos2(130.0, 100.0)).is_none());
        assert!(marker_at(&[], egui::pos2(0.0, 0.0)).is_none());
    }

    #[test]
    fn a_listed_slot_reads_back_with_its_frame() {
        let v = json!({"id": "kol", "label": "Kol", "group": "Makas ve kol", "along": "KOL", "across": "middle", "angle": 90,
            "flip": true, "when": null, "active": true, "frame": {"origin": [900, 35, 600], "x": [0, 0, 1], "y": [0, -1, 0], "z": [1, 0, 0]}});
        let slot = slot_of(&v).unwrap();
        assert_eq!((slot.id.as_str(), slot.angle, slot.flip, slot.when.as_deref()), ("kol", 90, true, None));
        let frame = slot.frame.unwrap();
        assert!(frame.origin.dist(Vec3::new(900.0, 35.0, 600.0)) < 1e-9 && frame.x.dist(Vec3::Z) < 1e-9 && frame.z.dist(Vec3::X) < 1e-9);
        assert!(slot_of(&json!({"label": "no id"})).is_none());
        let broken = slot_of(&json!({"id": "x", "frame": null, "error": "gone", "active": false})).unwrap();
        assert!(broken.frame.is_none() && broken.error.as_deref() == Some("gone"));
        assert_eq!(listed(&[("a", Vec3::ZERO, true)]).slots.len(), 1);
    }

    /// A sash profile in a fresh app, the library panel open.
    fn app() -> SolveApp {
        let mut app = SolveApp::new(Session::default(), crate::Services::default());
        app.run("solid.box", json!({"length": 62, "width": 70, "height": 1200, "corner": [838, 0, 0]})).unwrap();
        app.hardware_library = Some(crate::hardware_library::HardwareLibrary::default());
        app
    }

    #[test]
    fn picking_a_floor_face_and_an_edge_adds_a_slot_in_one_undo_step() {
        let mut app = app();
        let world = crate::open_meshing::session_world(&app.session);
        let body = world.bodies.first().expect("the profile");
        // Its outer face (x = 900, facing +X) and the edge along Z at y = 0 on it.
        let face = (0..body.body.face_count())
            .find(|&i| crate::dialogs::planar_face(&app.session, &body.name, i).is_some_and(|(_, n)| n.x > 0.99))
            .expect("the outer face");
        let edge = (0..body.body.edge_count())
            .find(|&i| {
                crate::hardware_connection::straight_ends(body, i).is_some_and(|(a, b)| {
                    let (lo, hi) = if a.z < b.z { (a, b) } else { (b, a) };
                    lo.dist(Vec3::new(900.0, 0.0, 0.0)) < 1e-6 && hi.dist(Vec3::new(900.0, 0.0, 1200.0)) < 1e-6
                })
            })
            .expect("the edge up the outer face");
        let undo = app.session.undo.len();
        app.ui.pick_components = true;
        start(&mut app, None, vec!["Köşe aktarma".into(), "Makas ve kol".into()], None).unwrap();
        assert!(active(&app) && !app.ui.pick_components, "a click picks faces and edges while picking");
        app.session.selection = vec![Sel::Face { body: body.name.clone(), index: face, point: Vec3::new(900.0, 35.0, 600.0) }];
        take_picks(&mut app);
        app.session.selection.push(Sel::Edge { body: body.name.clone(), index: edge, point: Vec3::new(900.0, 0.0, 50.0) });
        take_picks(&mut app);
        let mut pick = app.hardware_slot.take().unwrap();
        pick.fields.label = "Alt menteşe".into();
        pick.fields.along = "100 mm".into();
        update_preview(&mut app, &mut pick);
        let frame = match &pick.preview {
            Some((_, Ok((frame, true)))) => *frame,
            other => panic!("{other:?}"),
        };
        assert!(frame.origin.dist(Vec3::new(900.0, 35.0, 100.0)) < 1e-6, "100 mm up from the bottom corner, centred across: {frame:?}");
        assert_eq!(app.session.undo.len(), undo, "a preview changes nothing");
        app.hardware_slot = Some(pick);
        save(&mut app).unwrap();
        assert!(!active(&app) && app.ui.pick_components, "the pick filter is put back");
        assert_eq!(app.session.undo.len(), undo + 1, "one undo step");
        let mut listed = Listed::default();
        refresh(&mut app, &mut listed);
        assert_eq!(listed.slots.len(), 1);
        assert_eq!((listed.slots[0].id.as_str(), listed.slots[0].group.as_str()), ("alt-mentese", "Köşe aktarma"));
    }

    #[test]
    fn cancel_changes_nothing() {
        let mut app = app();
        let (undo, doc) = (app.session.undo.len(), app.session.doc.clone());
        start(&mut app, None, Vec::new(), None).unwrap();
        cancel(&mut app);
        assert!(!active(&app));
        assert_eq!(app.session.undo.len(), undo);
        assert!(std::sync::Arc::ptr_eq(&app.session.doc, &doc) || *app.session.doc == *doc);
    }
}
