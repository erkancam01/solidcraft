//! The Hardware Library panel, docked on the right: the parts and profiles in the library folder
//! (`hardware_library_dir`), as `hardware.library.list` lists them, one section per group folder.
//! Each row shows the part's picture, its name and part number, and whether its data card defines
//! a connection point. Clicking a row selects it; Insert into design (or a double click) brings a
//! hardware STEP part in through `file.insert_step`, and Connection point… defines where it
//! attaches (`hardware_connection`). Profiles and DXF sections wait for a later step. The
//! ribbon's Hardware Library (`ui.start` of `hardware.library.list`) opens it. Below the parts,
//! the Placement slots section lists the design's placement slots and adds, edits and deletes
//! them (`hardware_slot`); while the panel is open the view shows them.
//!
//! Pictures are made the way Home makes a recent design's (`home::file_picture`, cached on disk by
//! the file's path, size and time), off the window thread and one at a time, only for rows on
//! screen; closing the panel stops them.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::rc::Rc;

use egui::{RichText, vec2};
use serde_json::{Value, json};
use solidcraft_theme::Tokens;
use solidcraft_ui_text::tr;

use crate::SolveApp;
use crate::ui_kit::BackingTooltipExt as _;

/// The command that lists the library; started from the ribbon, it opens this panel.
pub(crate) const COMMAND: &str = "hardware.library.list";
pub(crate) const TITLE_KEY: &str = "cmd.hardware.library.list";
/// A row's picture as shown: Home's 240 × 150 pictures, smaller.
const THUMB: egui::Vec2 = egui::vec2(64.0, 40.0);
/// The longest folder path kept.
const MAX_PATH: usize = 4096;
/// The extensions the folder picker offers: a file picked in the library or one of its groups.
const PICKED: [&str; 4] = ["step", "stp", "dxf", "sc"];

/// The panel's state. The parts come from the engine; the panel keeps their last listing.
#[derive(Default)]
pub struct HardwareLibrary {
    focus_pending: bool,
    /// The folder listed, and its listing or why it could not be listed.
    listing: Option<(String, Result<Rc<Value>, String>)>,
    /// The folder field as edited; it becomes the preference on Enter or when it loses focus.
    pub folder: String,
    /// Filters the rows by name, part number or description.
    pub search: String,
    /// The selected part's path.
    pub selected: Option<String>,
    /// The last refusal (an insert that failed).
    pub error: Option<String>,
    pictures: Pictures,
    /// The design's placement slots (`hardware_slot`), listed again as the design changes.
    pub(crate) slots: crate::hardware_slot::Listed,
}

impl Drop for HardwareLibrary {
    fn drop(&mut self) {
        self.pictures.stop();
    }
}

/// Open the panel (again), showing the library folder as it is now.
pub fn open(app: &mut SolveApp) {
    let folder = clean(&app.preferences.hardware_library_dir);
    let d = app.hardware_library.get_or_insert_with(HardwareLibrary::default);
    d.focus_pending = true;
    d.listing = None;
    d.folder = folder;
}

pub(crate) fn available(app: &SolveApp) -> bool {
    app.hardware_library.is_some()
}

pub(crate) fn focus_pending(app: &SolveApp) -> bool {
    app.hardware_library.as_ref().is_some_and(|d| d.focus_pending)
}

pub(crate) fn set_focus_pending(app: &mut SolveApp, pending: bool) {
    if let Some(d) = &mut app.hardware_library {
        d.focus_pending = pending;
    }
}

/// Draw the panel inside the shared panel's body.
pub(crate) fn shared_content(app: &mut SolveApp, ui: &mut egui::Ui) {
    let Some(mut d) = app.hardware_library.take() else { return };
    body(app, ui, &mut d);
    app.hardware_library = Some(d);
}

/// A folder path as typed or saved: without surrounding spaces or quotes (a copied Windows path
/// has them) and control characters, and at most [`MAX_PATH`] characters.
fn clean(path: &str) -> String {
    let path = path.trim();
    let path = path.strip_prefix('"').and_then(|p| p.strip_suffix('"')).unwrap_or(path).trim();
    path.chars().filter(|c| !c.is_control()).take(MAX_PATH).collect()
}

/// The library folder for a file picked in it: the file's folder, or the one above it when the
/// file's folder holds no folders of its own (it is a group folder), unless that is a drive's root.
fn folder_for(file: &str) -> Option<String> {
    let parent = Path::new(file).parent()?;
    let has_folders =
        std::fs::read_dir(parent).is_ok_and(|entries| entries.take(10_000).filter_map(Result::ok).any(|e| e.file_type().is_ok_and(|t| t.is_dir())));
    let folder = match parent.parent() {
        Some(up) if !has_folders && up.parent().is_some() => up,
        _ => parent,
    };
    folder.to_str().map(str::to_owned).filter(|f| !f.is_empty())
}

/// List the folder again on the next frame: a part's data card changed.
pub(crate) fn refresh(app: &mut SolveApp) {
    if let Some(d) = &mut app.hardware_library {
        d.listing = None;
    }
}

fn set_folder(app: &mut SolveApp, folder: String) {
    let folder = clean(&folder);
    app.preferences.hardware_library_dir = folder.clone();
    if let Some(d) = &mut app.hardware_library {
        d.folder = folder;
        d.selected = None;
    }
}

fn choose_folder(app: &mut SolveApp) {
    app.pick_open_for(&PICKED, |app, path| {
        if let Some(folder) = path.as_deref().and_then(folder_for) {
            set_folder(app, folder);
        }
    });
}

/// List `folder` with the engine.
fn list(app: &mut SolveApp, d: &mut HardwareLibrary, folder: String) {
    let result = app.session.execute(COMMAND, &json!({ "folder": folder })).map(Rc::new).map_err(|e| crate::friendly_error(COMMAND, &e.to_string()));
    d.listing = Some((folder, result));
}

/// The library's group folders, by name, as last listed.
fn groups(d: &HardwareLibrary) -> Vec<String> {
    let listed = d.listing.as_ref().and_then(|(_, result)| result.as_ref().ok());
    listed
        .and_then(|l| l["groups"].as_array())
        .map(|groups| groups.iter().filter_map(|g| g["name"].as_str()).filter(|n| !n.is_empty()).map(str::to_owned).collect())
        .unwrap_or_default()
}

/// What the Placement slots section asked for.
fn slot_action(app: &mut SolveApp, d: &mut HardwareLibrary, action: crate::hardware_slot::SlotAction) {
    use crate::hardware_slot::SlotAction;
    match action {
        SlotAction::Add => d.error = crate::hardware_slot::start(app, None, groups(d)).err(),
        SlotAction::Edit(id) => {
            if let Some(slot) = d.slots.slots.iter().find(|s| s.id == id).cloned() {
                d.error = crate::hardware_slot::start(app, Some(&slot), groups(d)).err();
            }
        }
        SlotAction::Delete(id) => d.error = app.run("hardware.slot.delete", json!({ "id": id })).err(),
        SlotAction::Select(id) => {
            d.slots.selected = if d.slots.selected.as_deref() == Some(id.as_str()) { None } else { Some(id) };
            d.error = None;
        }
    }
}

fn text<'a>(part: &'a Value, key: &str) -> &'a str {
    part[key].as_str().unwrap_or_default()
}

/// The card's part number, when it has one.
fn part_no(part: &Value) -> Option<&str> {
    part["card"]["part_no"].as_str().filter(|n| !n.trim().is_empty())
}

/// A hardware STEP part, which Insert into design brings in. Profiles and DXF sections wait for
/// the profile tools.
fn insertable(part: &Value) -> bool {
    text(part, "kind") == "step" && text(part, "role") != "profile"
}

/// Whether a part matches the search (`needle` lower case; empty matches all).
fn found_by(part: &Value, needle: &str) -> bool {
    needle.is_empty()
        || [&part["name"], &part["file"], &part["card"]["part_no"], &part["card"]["description"]]
            .iter()
            .any(|v| v.as_str().is_some_and(|s| s.to_lowercase().contains(needle)))
}

/// The name a part comes into the design as: its part number, else its file's name.
fn insert_name(part: &Value) -> &str {
    part_no(part).unwrap_or(text(part, "name"))
}

fn insert(app: &mut SolveApp, d: &mut HardwareLibrary, part: &Value) {
    // While a connection point is picked, the design goes back afterwards: nothing comes in.
    if !insertable(part) || crate::hardware_connection::active(app) || crate::hardware_slot::active(app) {
        return;
    }
    d.error = app.run("file.insert_step", json!({ "path": text(part, "path"), "name": insert_name(part) })).err();
}

/// Connection point…: pick where the part attaches, on the part brought in for a moment.
fn define_connection(app: &mut SolveApp, d: &mut HardwareLibrary, part: &Value) {
    if !insertable(part) {
        return;
    }
    d.error = crate::hardware_connection::start(app, text(part, "path"), insert_name(part)).err();
}

/// What a row asked for.
enum RowAction {
    Select(String),
    Insert(Value),
}

/// What a footer button asked for.
enum FooterAction {
    Insert,
    Connection,
}

fn body(app: &mut SolveApp, ui: &mut egui::Ui, d: &mut HardwareLibrary) {
    let t = Tokens::get();
    let top = ui.cursor().top();
    let wanted = clean(&app.preferences.hardware_library_dir);
    if wanted.is_empty() {
        d.listing = None;
    } else if d.listing.as_ref().is_none_or(|(folder, _)| *folder != wanted) {
        list(app, d, wanted);
    }
    d.pictures.poll(ui.ctx());
    crate::hardware_slot::refresh(app, &mut d.slots);
    let library = d.listing.as_ref().and_then(|(_, result)| result.as_ref().ok()).cloned();
    let parts = library.as_ref().and_then(|l| l["parts"].as_array()).map_or(&[][..], Vec::as_slice);
    let selected = d.selected.as_deref().and_then(|path| parts.iter().find(|p| p["path"].as_str() == Some(path))).cloned();
    if selected.is_none() && library.is_some() {
        d.selected = None;
    }

    let failure = d.listing.as_ref().and_then(|(_, result)| result.as_ref().err()).cloned();
    let busy = crate::hardware_connection::active(app) || crate::hardware_slot::active(app);
    let footer = crate::ui_kit::Panel::pinned_footer(ui, "hardware-library-actions", 2, |ui| footer_buttons(ui, selected.as_ref(), busy));
    folder_row(app, ui, d);
    let search_id = ui.make_persistent_id("hardware-library-search");
    let response = crate::ui_kit::search_field(ui, search_id, &mut d.search, tr("hardware_library.search"));
    crate::panel_runtime::register_content_focus(ui.ctx(), &response);
    ui.separator();
    let before = ui.cursor().top();
    let needle = d.search.trim().to_lowercase();
    let scroll = egui::ScrollArea::vertical()
        .id_salt("hardware-library-parts")
        .min_scrolled_height(0.0)
        .max_height(ui.available_height().max(1.0))
        .show(ui, |ui| {
            let row = match (&library, &failure) {
                (Some(library), _) => parts_list(ui, d, library, &needle),
                (None, Some(error)) => {
                    ui.label(RichText::new(error.as_str()).color(t.error));
                    None
                }
                (None, None) => {
                    ui.label(RichText::new(tr("hardware_library.no_folder")).color(t.text_dim));
                    None
                }
            };
            ui.separator();
            let slot = crate::hardware_slot::section(ui, &d.slots, busy);
            (row, slot)
        });
    let after = scroll.inner_rect.bottom();
    if let Some(error) = &d.error {
        ui.label(RichText::new(error.as_str()).color(t.error));
    }
    let (row, slot) = scroll.inner;
    if let Some(action) = slot {
        slot_action(app, d, action);
    }
    match row {
        Some(RowAction::Select(path)) => {
            d.selected = Some(path);
            d.error = None;
        }
        Some(RowAction::Insert(part)) => insert(app, d, &part),
        None => {}
    }
    match (&footer.inner, &selected) {
        (Some(FooterAction::Insert), Some(part)) => insert(app, d, part),
        (Some(FooterAction::Connection), Some(part)) => define_connection(app, d, part),
        _ => {}
    }
    let natural =
        (before - top) + scroll.content_size.y + (ui.cursor().top() - after).max(0.0) + footer.response.rect.height() + ui.spacing().item_spacing.y;
    crate::panel_runtime::report_content_extent(ui, top, natural, false);
}

/// The folder: its path, Choose folder… and Refresh.
fn folder_row(app: &mut SolveApp, ui: &mut egui::Ui, d: &mut HardwareLibrary) {
    let t = Tokens::get();
    ui.label(RichText::new(tr("hardware_library.folder")).small().color(t.text_dim));
    let (enter, response) = crate::ui_kit::text_field_reserving(ui, &mut d.folder, tr("hardware_library.folder_hint"), 0.0);
    crate::panel_runtime::register_content_focus(ui.ctx(), &response);
    if enter || response.lost_focus() {
        let folder = clean(&d.folder);
        if folder != app.preferences.hardware_library_dir {
            app.preferences.hardware_library_dir = folder.clone();
            d.selected = None;
        }
        d.folder = folder;
    } else if !response.has_focus() && d.folder != app.preferences.hardware_library_dir {
        // Set from elsewhere (the folder picker, Preferences, the control channel).
        d.folder = clean(&app.preferences.hardware_library_dir);
    }
    ui.horizontal_wrapped(|ui| {
        let choose = crate::ui_kit::button(ui, tr("hardware_library.choose_folder"), crate::ui_kit::Button::Normal)
            .backing_on_hover_text(tr("hardware_library.choose_folder_help"));
        crate::panel_runtime::register_content_focus(ui.ctx(), &choose);
        if choose.clicked() {
            choose_folder(app);
        }
        let listed = d.listing.is_some();
        let refresh = crate::ui_kit::icon_action(ui, tr("hardware_library.refresh"), "update", listed, false);
        crate::panel_runtime::register_content_focus(ui.ctx(), &refresh);
        if refresh.clicked() {
            d.listing = None;
            d.pictures.forget();
        }
    });
}

/// Insert into design, and Connection point… (Change connection point… when the card has one).
/// Both wait while a connection point is picked (`busy`).
fn footer_buttons(ui: &mut egui::Ui, selected: Option<&Value>, busy: bool) -> Option<FooterAction> {
    let usable = selected.is_some_and(insertable);
    let enabled = usable && !busy;
    let defined = selected.is_some_and(|part| part["card"]["connection"].is_object());
    let connection = if defined { tr("hardware_library.change_connection") } else { tr("hardware_library.define_connection") };
    let mut action = None;
    for (label, wanted) in [(tr("hardware_library.insert"), FooterAction::Insert), (connection, FooterAction::Connection)] {
        let response = crate::ui_kit::button(ui, label, crate::ui_kit::Button::Enabled(enabled));
        let response =
            if selected.is_some() && !usable { response.backing_on_disabled_hover_text(tr("hardware_library.insert_later")) } else { response };
        crate::panel_runtime::register_content_focus(ui.ctx(), &response);
        if enabled && response.clicked() {
            action = Some(wanted);
        }
    }
    action
}

/// The parts, one section per group; with a search, the matching parts in one list.
fn parts_list(ui: &mut egui::Ui, d: &mut HardwareLibrary, library: &Value, needle: &str) -> Option<RowAction> {
    let t = Tokens::get();
    let parts = library["parts"].as_array().map_or(&[][..], Vec::as_slice);
    let groups = library["groups"].as_array().map_or(&[][..], Vec::as_slice);
    let mut action = None;
    if parts.is_empty() {
        ui.label(RichText::new(tr("hardware_library.empty")).color(t.text_dim));
    } else if !needle.is_empty() {
        let mut any = false;
        for part in parts.iter().filter(|p| found_by(p, needle)) {
            any = true;
            action = row(ui, d, part, true).or(action);
        }
        if !any {
            ui.label(RichText::new(tr("hardware_library.no_match")).color(t.text_dim));
        }
    } else {
        for group in groups {
            let name = text(group, "name");
            let count = group["parts"].as_u64().unwrap_or(0);
            let title = if name.is_empty() { tr("hardware_library.ungrouped") } else { name };
            // A folder's name is shown as it is, never looked up as an English label.
            let verbatim = solidcraft_ui_text::resolved_labels();
            // A folder's name is shown as written, never in capitals the user's language may
            // spell otherwise (Turkish i → İ).
            crate::ui_kit::section_count_as_written(ui, ("hardware-library-group", name), title, true, Some(count), |ui| {
                let mut any = false;
                for part in parts.iter().filter(|p| text(p, "group") == name) {
                    any = true;
                    action = row(ui, d, part, false).or(action.take());
                }
                if !any {
                    ui.label(RichText::new(tr("hardware_library.empty_group")).small().color(t.text_dim));
                }
            });
            drop(verbatim);
        }
    }
    if library["truncated"].as_bool() == Some(true) {
        ui.label(RichText::new(tr("hardware_library.truncated")).small().color(t.warning));
    }
    action
}

/// One line of text cut to `width`.
fn line(ui: &egui::Ui, text: &str, font: egui::FontId, color: egui::Color32, width: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width.max(0.0));
    ui.painter().layout_job(job)
}

/// One part: its picture, name, part number (and group, in a search) and connection point.
fn row(ui: &mut egui::Ui, d: &mut HardwareLibrary, part: &Value, show_group: bool) -> Option<RowAction> {
    let t = Tokens::get();
    let path = text(part, "path");
    let name = text(part, "name");
    let selected = d.selected.as_deref() == Some(path);
    let height = THUMB.y + 2.0 * t.space_4;
    let (rect, response) = ui.allocate_exact_size(vec2(crate::ui_kit::dimension(ui.available_width()), height), egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, selected, name));
    crate::scenario::publish_target(&format!("hardware:part:{path}"), rect);
    if selected || response.has_focus() {
        crate::panel_runtime::register_content_focus(ui.ctx(), &response);
    }
    if !ui.is_rect_visible(rect) {
        return None;
    }
    let painter = ui.painter_at(rect);
    if selected {
        painter.rect_filled(rect, t.radius_sm, t.selected_bg);
    } else if response.hovered() {
        painter.rect_filled(rect, t.radius_sm, t.hover);
    }
    let thumb = egui::Rect::from_min_size(rect.min + vec2(t.space_4, t.space_4), THUMB);
    let kind = text(part, "kind");
    match d.pictures.texture(path) {
        Some(texture) => egui::Image::new(texture).paint_at(ui, thumb),
        None => {
            painter.rect_filled(thumb, t.radius_sm, t.control_fill);
            let icon = egui::Rect::from_center_size(thumb.center(), vec2(t.space_24, t.space_24));
            crate::ui_kit::icons::paint_ink(&painter, icon, if kind == "dxf" { "dxf" } else { "component" }, t.text_dim);
            // A DXF section has no picture of its own.
            if kind != "dxf" {
                d.pictures.want(path);
            }
        }
    }
    let ink = if selected { t.selected_ink } else { t.text };
    let dim = if selected { t.selected_ink } else { t.text_dim };
    let left = thumb.right() + t.space_8;
    let width = rect.right() - left - t.space_4;
    let title = line(ui, name, crate::ui_kit::type_face::semibold(t.font_label), ink, width);
    let mut meta: Vec<&str> = part_no(part).into_iter().collect();
    if text(part, "role") == "profile" {
        meta.push(tr("hardware_library.profile"));
    }
    if show_group && !text(part, "group").is_empty() {
        meta.push(text(part, "group"));
    }
    let meta = line(ui, &meta.join(" · "), egui::FontId::proportional(t.font_small), dim, width);
    let (status, colour) = if part.get("card_error").is_some_and(|e| !e.is_null()) {
        (tr("hardware_library.card_error"), t.error)
    } else if part["card"]["connection"].is_object() {
        (tr("hardware_library.connection"), if selected { t.selected_ink } else { t.accent })
    } else {
        (tr("hardware_library.no_connection"), dim)
    };
    let status = line(ui, status, egui::FontId::proportional(t.font_small), colour, width);
    let total = title.size().y + meta.size().y + status.size().y;
    let mut y = rect.center().y - total / 2.0;
    for galley in [title, meta, status] {
        let h = galley.size().y;
        painter.galley(egui::pos2(left, y), galley, ink);
        y += h;
    }
    let response = match part["card_error"].as_str() {
        Some(error) => response.backing_on_hover_text(error),
        None => response,
    };
    if response.double_clicked() && insertable(part) {
        Some(RowAction::Insert(part.clone()))
    } else if response.clicked() && !selected {
        Some(RowAction::Select(path.to_owned()))
    } else {
        None
    }
}

/// The rows' pictures, made one at a time off the window thread.
#[derive(Default)]
struct Pictures {
    textures: HashMap<String, egui::TextureHandle>,
    /// Paths asked for, with or without a picture yet: each is asked for once.
    asked: HashSet<String>,
    queue: VecDeque<String>,
    #[cfg(not(target_arch = "wasm32"))]
    job: Option<std::sync::mpsc::Receiver<(String, Option<egui::ColorImage>)>>,
    /// Stops the picture being made when the panel closes.
    #[cfg(not(target_arch = "wasm32"))]
    stop: std::sync::Arc<solidcraft_engine::doc::progress::Progress>,
}

impl Pictures {
    fn texture(&self, path: &str) -> Option<&egui::TextureHandle> {
        self.textures.get(path)
    }

    fn want(&mut self, path: &str) {
        if self.asked.insert(path.to_owned()) {
            self.queue.push_back(path.to_owned());
        }
    }

    /// Ask for every picture again (Refresh: the files may have changed).
    fn forget(&mut self) {
        self.textures.clear();
        self.asked.clear();
        self.queue.clear();
    }

    fn stop(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.stop.cancel();
            self.job = None;
        }
        self.queue.clear();
    }

    /// Take a finished picture and start the next one.
    #[cfg(not(target_arch = "wasm32"))]
    fn poll(&mut self, ctx: &egui::Context) {
        use std::sync::mpsc::TryRecvError;
        let wait = std::time::Duration::from_millis(100);
        if let Some(rx) = &self.job {
            match rx.try_recv() {
                Ok((path, picture)) => {
                    if let Some(picture) = picture {
                        let texture = ctx.load_texture(format!("hardware:{path}"), picture, egui::TextureOptions::LINEAR);
                        self.textures.insert(path, texture);
                    }
                    self.job = None;
                }
                Err(TryRecvError::Disconnected) => self.job = None,
                Err(TryRecvError::Empty) => ctx.request_repaint_after(wait),
            }
        }
        if self.job.is_none()
            && !self.stop.is_cancelled()
            && let Some(path) = self.queue.pop_front()
        {
            let (tx, rx) = std::sync::mpsc::channel();
            let stop = self.stop.clone();
            let spawned = std::thread::Builder::new().name("hardware-picture".into()).spawn(move || {
                let picture = crate::home::file_picture(&path, &stop).map(|picture| half(&picture));
                let _ = tx.send((path, picture));
            });
            if spawned.is_ok() {
                self.job = Some(rx);
                ctx.request_repaint_after(wait);
            }
        }
    }

    /// The web build has no threads to make pictures on: rows keep their placeholder.
    #[cfg(target_arch = "wasm32")]
    fn poll(&mut self, _ctx: &egui::Context) {
        self.queue.clear();
    }
}

/// A picture at half its size, each pixel the mean of the four it covers: a row shows it at most
/// at twice its size, and it takes a quarter of the memory.
#[cfg(not(target_arch = "wasm32"))]
fn half(picture: &egui::ColorImage) -> egui::ColorImage {
    let [w, h] = picture.size;
    let (hw, hh) = ((w / 2).max(1), (h / 2).max(1));
    let at = |x: usize, y: usize| picture.pixels.get(y.min(h.saturating_sub(1)) * w + x.min(w.saturating_sub(1))).copied().unwrap_or_default();
    let mut rgba = Vec::with_capacity(hw * hh * 4);
    for y in 0..hh {
        for x in 0..hw {
            let quad = [at(2 * x, 2 * y), at(2 * x + 1, 2 * y), at(2 * x, 2 * y + 1), at(2 * x + 1, 2 * y + 1)].map(|c| c.to_array());
            for channel in 0..4 {
                let sum: u16 = quad.iter().map(|c| u16::from(c[channel])).sum();
                rgba.push((sum / 4) as u8);
            }
        }
    }
    egui::ColorImage::from_rgba_premultiplied([hw, hh], &rgba)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::scenario::Harness;

    /// A library folder in the temporary folder, removed when dropped.
    struct Folder(std::path::PathBuf);
    impl Folder {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("solidcraft-hardware-panel-{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            for (file, contents) in [
                ("Köşe aktarma/YMB.00146 - TIRNAKLI KÖŞE AKTARMA.STEP", "not a real STEP file"),
                (
                    "Köşe aktarma/YMB.00146 - TIRNAKLI KÖŞE AKTARMA.json",
                    r#"{"part_no": "YMB.00146", "connection": {"origin": [0, 0, 0], "z": [0, 0, 1], "x": [1, 0, 0]}}"#,
                ),
                ("Profil/sash-70.dxf", "0\nEOF\n"),
            ] {
                let path = dir.join(file);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, contents).unwrap();
            }
            std::fs::create_dir_all(dir.join("Menteşe grubu")).unwrap();
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
    fn the_panel_lists_the_folder_by_group_and_selects_a_part() {
        let folder = Folder::new("list");
        let mut h = Harness::new();
        h.call("ui.resize", json!({"width": 1600, "height": 1000}));
        h.app.preferences.hardware_library_dir = folder.path("");
        h.step(&json!({"start": "hardware.library.list"})).unwrap();
        h.frames(4);
        assert!(h.app.hardware_library.is_some(), "ui.start opens the panel");
        assert!(h.widget("YMB.00146 - TIRNAKLI KÖŞE AKTARMA").is_some(), "the STEP part is listed");
        assert!(h.widget("sash-70").is_some(), "the DXF profile is listed");
        h.step(&json!({"widget": "sash-70"})).unwrap();
        h.frames(2);
        let d = h.app.hardware_library.as_ref().unwrap();
        assert_eq!(d.selected.as_deref(), Some(folder.path("Profil/sash-70.dxf").as_str()));
        let undo = h.app.session.undo.len();
        // The button is disabled for it: a click (if the step finds it at all) does nothing.
        let _ = h.step(&json!({"widget": "Insert into design"}));
        assert_eq!(h.app.session.undo.len(), undo, "a profile is not inserted yet");
        // Closing the panel drops it, which stops its pictures.
        h.app.hardware_library = None;
        h.frames(2);
    }

    #[test]
    fn an_empty_folder_preference_asks_for_a_folder() {
        let mut h = Harness::new();
        h.call("ui.resize", json!({"width": 1600, "height": 1000}));
        h.app.preferences.hardware_library_dir.clear();
        h.step(&json!({"start": "hardware.library.list"})).unwrap();
        h.frames(4);
        assert!(h.widget("Choose folder…").is_some());
        assert!(h.app.hardware_library.as_ref().is_some_and(|d| d.listing.is_none()));
    }

    #[test]
    fn a_picked_file_names_its_library_folder() {
        let folder = Folder::new("pick");
        let library = folder.path("");
        let library = library.trim_end_matches(['/', '\\']);
        assert_eq!(
            super::folder_for(&folder.path("Köşe aktarma/YMB.00146 - TIRNAKLI KÖŞE AKTARMA.STEP")).as_deref(),
            Some(library),
            "a group's file"
        );
        std::fs::write(folder.path("loose.stp"), "").unwrap();
        assert_eq!(super::folder_for(&folder.path("loose.stp")).as_deref(), Some(library), "a file beside the group folders");
        assert_eq!(super::clean("  \"C:\\Lib\\Hardware\"  "), "C:\\Lib\\Hardware");
        assert_eq!(super::clean("a\u{7}b"), "ab");
    }
}
