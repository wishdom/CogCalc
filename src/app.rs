use std::time::{Duration, Instant};

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;

use cogcalc::gearing::{
    GearRow, GearSetup, SPEED_THRESHOLDS_KMH, SpeedUnit, WheelPreset, export_csv, export_tsv,
};
use eframe::egui::{self, Align, Button, Layout, RichText, Sense};
use egui_extras::{Column, TableBuilder};
use serde::{Deserialize, Serialize};

use crate::{chart, drivetrain::DrivetrainView, theme};

pub const APP_NAME: &str = "CogCalc";
const STORAGE_KEY: &str = "cogcalc.settings.v1";
const CASSETTES: &[(&str, &str)] = &[
    ("Road · 11–32", "11, 12, 13, 14, 16, 18, 20, 22, 25, 28, 32"),
    ("Road · 11–34", "11, 13, 15, 17, 19, 21, 23, 25, 27, 30, 34"),
    (
        "Gravel · 11–42",
        "11, 13, 15, 17, 19, 21, 24, 28, 32, 36, 42",
    ),
    (
        "MTB · 10–51",
        "10, 12, 14, 16, 18, 21, 24, 28, 33, 39, 45, 51",
    ),
    ("Every size · 9–60", "9-60"),
];

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    chainrings: String,
    sprockets: String,
    wheel: usize,
    circumference: String,
    cadence: f64,
    imperial: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            chainrings: "50, 34".into(),
            sprockets: CASSETTES[0].1.into(),
            wheel: 0,
            circumference: "2130".into(),
            cadence: 90.0,
            imperial: false,
        }
    }
}

impl Settings {
    fn unit(&self) -> SpeedUnit {
        if self.imperial {
            SpeedUnit::Imperial
        } else {
            SpeedUnit::Metric
        }
    }

    fn wheel_mm(&self) -> Result<f64, String> {
        if let Some(wheel) = WheelPreset::ALL.get(self.wheel) {
            Ok(wheel.circumference_mm())
        } else {
            self.circumference
                .trim()
                .parse::<f64>()
                .map_err(|_| "Enter a wheel circumference in millimeters, for example 2130.".into())
        }
    }

    fn setup(&self) -> Result<GearSetup, String> {
        GearSetup::parse(&self.chainrings, &self.sprockets, self.wheel_mm()?)
            .map_err(|error| error.to_string())
    }

    fn sanitize(&mut self) {
        self.wheel = self.wheel.min(WheelPreset::ALL.len());
        self.cadence = if self.cadence.is_finite() {
            self.cadence.clamp(30.0, 150.0)
        } else {
            90.0
        };
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct NamedSetup {
    name: String,
    settings: Settings,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct SavedState {
    settings: Settings,
    baseline: Option<Settings>,
    dark_mode: bool,
    technical_columns: bool,
    named_setups: Vec<NamedSetup>,
    active_setup: Option<String>,
}

enum SetupDialog {
    Save {
        name: String,
        error: Option<String>,
        focus: bool,
    },
    Delete {
        name: String,
    },
}

fn valid_setup_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        Err("Enter a name for this setup.".into())
    } else if name.chars().count() > 80 {
        Err("Use a name of 80 characters or fewer.".into())
    } else if name.chars().any(char::is_control) {
        Err("Use a single line of text for the name.".into())
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, Debug)]
enum ResultsTab {
    #[default]
    Gears,
    Cadence,
    Chart,
    Drivetrain,
    Compare,
}

fn choice_button(
    ui: &mut egui::Ui,
    label: impl Into<String>,
    selected: bool,
    width: f32,
) -> egui::Response {
    let p = theme::palette(ui);
    ui.add(
        Button::new(RichText::new(label.into()).color(if selected {
            p.accent_text
        } else {
            p.text
        }))
        .fill(if selected { p.accent } else { p.surface })
        .selected(selected)
        .wrap_mode(egui::TextWrapMode::Extend)
        .min_size(egui::vec2(width, 34.0)),
    )
}

fn gear_choices(ui: &mut egui::Ui, label: &'static str, teeth: &[u16], selected: &mut u16) {
    let button_width = if teeth.iter().any(|teeth| *teeth >= 100) {
        52.0
    } else {
        44.0
    };
    let remembered = ui.make_persistent_id(("gear_choice_visible", label));
    let key = (
        *selected,
        ui.available_width().round() as u32,
        teeth.to_vec(),
    );
    let reveal = ui
        .ctx()
        .data(|data| data.get_temp::<(u16, u32, Vec<u16>)>(remembered))
        .as_ref()
        != Some(&key);
    ui.horizontal_top(|ui| {
        ui.add_sized(
            [44.0, 34.0],
            egui::Label::new(RichText::new(label).strong()),
        );
        let mut choices = |ui: &mut egui::Ui, row: &[u16]| {
            for &tooth in row {
                let response = ui
                    .push_id((label, tooth), |ui| {
                        choice_button(ui, tooth.to_string(), tooth == *selected, button_width)
                    })
                    .inner
                    .on_hover_text(format!("{label} gear: {tooth} teeth"));
                if teeth.len() > 16 && reveal && tooth == *selected {
                    response.scroll_to_me(Some(Align::Center));
                }
                if response.clicked() {
                    *selected = tooth;
                }
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::Button,
                        ui.is_enabled(),
                        tooth == *selected,
                        format!("{label} gear: {tooth} teeth"),
                    )
                });
            }
        };
        if teeth.len() <= 16 {
            let gap = ui.spacing().item_spacing.x;
            let columns = ((ui.available_width() + gap) / (button_width + gap))
                .floor()
                .max(1.0) as usize;
            ui.vertical(|ui| {
                for row in teeth.chunks(columns) {
                    ui.horizontal(|ui| choices(ui, row));
                }
            });
        } else {
            egui::ScrollArea::horizontal()
                .id_salt(("gear_choices", label))
                .animated(false)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.horizontal(|ui| choices(ui, teeth));
                });
        }
    });
    ui.ctx()
        .data_mut(|data| data.insert_temp(remembered, (*selected, key.1, key.2)));
}

pub struct GearApp {
    saved: SavedState,
    rows: Vec<GearRow>,
    setup: Option<GearSetup>,
    error: Option<String>,
    fingerprint: String,
    selected: Option<(u16, u16)>,
    tab: ResultsTab,
    notice: Option<(String, bool, Instant)>,
    show_about: bool,
    setup_dialog: Option<SetupDialog>,
    persist_library: bool,
    drivetrain: DrivetrainView,
}

impl GearApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut saved: SavedState = cc
            .storage
            .and_then(|storage| eframe::get_value(storage, STORAGE_KEY))
            .unwrap_or_default();
        saved.settings.sanitize();
        let mut names = std::collections::HashSet::new();
        saved.named_setups.retain_mut(|entry| {
            entry.settings.sanitize();
            entry.name = entry.name.trim().to_owned();
            valid_setup_name(&entry.name).is_ok()
                && entry.settings.setup().is_ok()
                && names.insert(entry.name.to_lowercase())
        });
        if !saved
            .named_setups
            .iter()
            .any(|entry| Some(&entry.name) == saved.active_setup.as_ref())
        {
            saved.active_setup = None;
        }
        #[cfg(feature = "screenshot")]
        if std::env::var_os("EFRAME_SCREENSHOT_TO").is_some() {
            saved.dark_mode = std::env::var("COGCALC_PREVIEW_THEME").as_deref() == Ok("dark");
        }
        theme::install(&cc.egui_ctx, saved.dark_mode);
        if saved
            .baseline
            .as_ref()
            .is_some_and(|baseline| baseline.setup().is_err())
        {
            saved.baseline = None;
        }
        let mut app = Self {
            saved,
            rows: Vec::new(),
            setup: None,
            error: None,
            fingerprint: String::new(),
            selected: None,
            tab: ResultsTab::Gears,
            notice: None,
            show_about: false,
            setup_dialog: None,
            persist_library: false,
            drivetrain: DrivetrainView::default(),
        };
        app.refresh();
        #[cfg(feature = "screenshot")]
        if std::env::var_os("EFRAME_SCREENSHOT_TO").is_some() {
            app.tab = match std::env::var("COGCALC_PREVIEW_TAB").as_deref() {
                Ok("chart") => ResultsTab::Chart,
                Ok("drivetrain") => ResultsTab::Drivetrain,
                _ => ResultsTab::Gears,
            };
            if let Ok(name) = std::env::var("COGCALC_PREVIEW_SETUP_NAME") {
                let _ = app.save_named_setup(&name);
                app.notice = None;
                app.persist_library = false;
            }
        }
        app
    }

    fn refresh(&mut self) {
        let s = &self.saved.settings;
        let fingerprint = format!(
            "{}|{}|{}|{}",
            s.chainrings, s.sprockets, s.wheel, s.circumference
        );
        if fingerprint == self.fingerprint {
            return;
        }
        self.fingerprint = fingerprint;
        match s.setup() {
            Ok(setup) => {
                self.rows = setup.rows();
                self.rows.sort_by(|a, b| a.ratio.total_cmp(&b.ratio));
                self.error = None;
                if !self
                    .rows
                    .iter()
                    .any(|r| Some((r.front_teeth, r.rear_teeth)) == self.selected)
                {
                    self.selected = self
                        .rows
                        .iter()
                        .min_by(|a, b| a.ratio.total_cmp(&b.ratio))
                        .map(|r| (r.front_teeth, r.rear_teeth));
                }
                self.setup = Some(setup);
            }
            Err(error) => {
                self.error = Some(error);
                self.rows.clear();
                self.selected = None;
                self.setup = None;
            }
        }
    }

    fn notify(&mut self, message: impl Into<String>, error: bool) {
        self.notice = Some((message.into(), error, Instant::now()));
    }

    fn copy_table(&mut self, ctx: &egui::Context) {
        if self.rows.is_empty() {
            return;
        }
        ctx.copy_text(export_tsv(
            &self.rows,
            self.saved.settings.cadence,
            self.saved.settings.unit(),
        ));
        self.notify(
            format!(
                "Copied {} gear combinations. Paste into any spreadsheet.",
                self.rows.len()
            ),
            false,
        );
    }

    fn export(&mut self) {
        if self.rows.is_empty() {
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Export gearing table")
            .set_file_name("cogcalc-gearing.csv")
            .add_filter("CSV spreadsheet", &["csv"])
            .save_file()
        {
            let csv = export_csv(
                &self.rows,
                self.saved.settings.cadence,
                self.saved.settings.unit(),
            );
            match std::fs::write(&path, csv) {
                Ok(()) => self.notify(format!("Exported to {}", path.display()), false),
                Err(error) => self.notify(format!("Could not save the CSV: {error}"), true),
            }
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let p = theme::palette(ui);
        egui::Panel::top("header")
            .exact_size(62.0)
            .frame(
                egui::Frame::new()
                    .fill(p.surface)
                    .inner_margin(egui::Margin::symmetric(18, 12)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    theme::brand(ui);
                    ui.label(RichText::new(APP_NAME).size(22.0).strong());
                    ui.add_space(8.0);
                    theme::muted(ui, "Bicycle gear calculator");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Help").clicked() {
                            self.show_about = true;
                        }
                        if theme::theme_toggle(ui, &mut self.saved.dark_mode).changed() {
                            theme::install(ui.ctx(), self.saved.dark_mode);
                            ui.ctx().request_repaint();
                        }
                    });
                });
            });
    }

    fn save_named_setup(&mut self, name: &str) -> Result<(), String> {
        let name = name.trim();
        valid_setup_name(name)?;
        self.saved.settings.setup()?;
        if self
            .saved
            .named_setups
            .iter()
            .any(|entry| entry.name.to_lowercase() == name.to_lowercase())
        {
            return Err("That name is already in use. Choose another name, or cancel and use Update on the loaded setup.".into());
        }
        self.saved.named_setups.push(NamedSetup {
            name: name.to_owned(),
            settings: self.saved.settings.clone(),
        });
        self.saved.active_setup = Some(name.to_owned());
        self.persist_library = true;
        self.notify(format!("Saved setup \"{name}\" on this device."), false);
        Ok(())
    }

    fn load_named_setup(&mut self, name: &str) {
        if let Some(entry) = self
            .saved
            .named_setups
            .iter()
            .find(|entry| entry.name == name)
        {
            self.saved.settings = entry.settings.clone();
            self.saved.active_setup = Some(entry.name.clone());
            self.refresh();
            self.persist_library = true;
            self.notify(format!("Loaded setup \"{name}\"."), false);
        }
    }

    fn update_named_setup(&mut self) -> Result<(), String> {
        self.saved.settings.setup()?;
        let entry = self
            .saved
            .named_setups
            .iter_mut()
            .find(|entry| Some(&entry.name) == self.saved.active_setup.as_ref())
            .ok_or_else(|| "Load a saved setup before updating it.".to_owned())?;
        entry.settings = self.saved.settings.clone();
        let name = entry.name.clone();
        self.persist_library = true;
        self.notify(format!("Updated setup \"{name}\"."), false);
        Ok(())
    }

    fn delete_named_setup(&mut self, name: &str) {
        self.saved.named_setups.retain(|entry| entry.name != name);
        if self.saved.active_setup.as_deref() == Some(name) {
            self.saved.active_setup = None;
        }
        self.persist_library = true;
        self.notify(
            format!("Deleted saved setup \"{name}\". Current bike inputs are unchanged."),
            false,
        );
    }

    fn saved_setups(&mut self, ui: &mut egui::Ui) {
        ui.strong("Saved setups");
        let active = self
            .saved
            .named_setups
            .iter()
            .find(|entry| Some(&entry.name) == self.saved.active_setup.as_ref());
        let edited = active.is_some_and(|entry| entry.settings != self.saved.settings);
        let has_active = active.is_some();
        if self.saved.named_setups.is_empty() {
            theme::muted(ui, "Save your bike's setup to use it again.");
        } else {
            let label = active
                .map(|entry| entry.name.clone())
                .unwrap_or_else(|| "Choose a saved setup".to_owned());
            let mut load = None;
            ui.scope(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                egui::ComboBox::from_id_salt("saved_setup_picker")
                    .selected_text(label)
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        for entry in &self.saved.named_setups {
                            if ui
                                .selectable_label(
                                    Some(&entry.name) == self.saved.active_setup.as_ref(),
                                    &entry.name,
                                )
                                .on_hover_text(&entry.name)
                                .clicked()
                            {
                                load = Some(entry.name.clone());
                            }
                        }
                    })
                    .response
                    .on_hover_text(
                        "Load a saved bike, including wheel size, cadence, and speed units",
                    );
            });
            if let Some(name) = load {
                self.load_named_setup(&name);
            }
            if edited {
                theme::muted(ui, "Edited. Use Update to save these changes.");
            }
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    self.error.is_none(),
                    Button::new(if has_active {
                        "Save as…"
                    } else {
                        "Save setup…"
                    }),
                )
                .on_hover_text(
                    "Save the current bike, wheel size, cadence, and units under a new name",
                )
                .clicked()
            {
                self.setup_dialog = Some(SetupDialog::Save {
                    name: String::new(),
                    error: None,
                    focus: true,
                });
            }
            if has_active {
                if ui
                    .add_enabled(edited && self.error.is_none(), Button::new("Update"))
                    .on_hover_text("Replace the loaded saved setup with your current inputs")
                    .clicked()
                    && let Err(error) = self.update_named_setup()
                {
                    self.notify(error, true);
                }
                if ui
                    .button("Delete")
                    .on_hover_text("Remove this setup from the saved list")
                    .clicked()
                {
                    self.setup_dialog = self
                        .saved
                        .active_setup
                        .clone()
                        .map(|name| SetupDialog::Delete { name });
                }
            }
        });
    }

    fn setup_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.setup_dialog.take() else {
            return;
        };
        let mut finished = false;
        let response = egui::Modal::new(egui::Id::new("saved_setup_dialog")).show(ctx, |ui| {
            ui.set_width(370.0);
            match &mut dialog {
                SetupDialog::Save { name, error, focus } => {
                    ui.heading("Save bike setup");
                    ui.label("Store the gears, wheel size, cadence, and units on this device.");
                    ui.label("Setup name");
                    let field = ui.add(egui::TextEdit::singleline(name)
                        .id(egui::Id::new("saved_setup_name"))
                        .hint_text("e.g. Commuter bike").desired_width(f32::INFINITY));
                    field.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Setup name"));
                    if *focus { field.request_focus(); *focus = false; }
                    if field.changed() { *error = None; }
                    if let Some(message) = error.as_ref() { ui.colored_label(theme::palette(ui).error, message); }
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked()
                            || (field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter))) {
                            match self.save_named_setup(name) {
                                Ok(()) => finished = true,
                                Err(message) => *error = Some(message),
                            }
                        }
                        if ui.button("Cancel").clicked() { finished = true; }
                    });
                }
                SetupDialog::Delete { name } => {
                    ui.heading("Delete saved setup?");
                    ui.label(format!("Remove \"{name}\" from your saved setups? The current bike inputs will stay as they are."));
                    ui.horizontal(|ui| {
                        if ui.button("Delete setup").clicked() {
                            self.delete_named_setup(name);
                            finished = true;
                        }
                        if ui.button("Cancel").clicked() { finished = true; }
                    });
                }
            }
        });
        if !finished && !response.should_close() {
            self.setup_dialog = Some(dialog);
        }
    }

    fn export_drivetrain(&mut self) {
        if let Some(svg) = self.drivetrain.svg()
            && let Some(path) = rfd::FileDialog::new()
                .set_title("Export drivetrain diagram")
                .set_file_name("cogcalc-drivetrain.svg")
                .add_filter("SVG vector image", &["svg"])
                .save_file()
        {
            match std::fs::write(&path, svg) {
                Ok(()) => self.notify(format!("Exported to {}", path.display()), false),
                Err(error) => self.notify(format!("Could not save the SVG: {error}"), true),
            }
        }
    }

    fn apply_preset(&mut self, index: usize) {
        self.saved.active_setup = None;
        let cadence = self.saved.settings.cadence;
        let imperial = self.saved.settings.imperial;
        self.saved.settings = Settings {
            cadence,
            imperial,
            ..Settings::default()
        };
        match index {
            1 => {
                self.saved.settings.chainrings = "40".into();
                self.saved.settings.sprockets = CASSETTES[2].1.into();
                self.select_wheel_containing("40");
            }
            2 => {
                self.saved.settings.chainrings = "32".into();
                self.saved.settings.sprockets = CASSETTES[3].1.into();
                self.select_wheel_containing("29");
            }
            _ => {}
        }
    }

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        let p = theme::palette(ui);
        egui::Panel::left("setup").exact_size(280.0).resizable(false)
            .frame(egui::Frame::new().fill(p.surface).inner_margin(18))
            .show(ui, |ui| {
                egui::ScrollArea::vertical().id_salt("setup_scroll").show(ui, |ui| {
                    ui.heading("1. Bike setup");
                    self.saved_setups(ui);
                    ui.separator();
                    theme::muted(ui, "Start with your kind of bike.");
                    ui.horizontal(|ui| {
                        for (index, name) in ["Road", "Gravel", "MTB"].iter().enumerate() {
                            let matches = match index {
                                0 => self.saved.settings.chainrings == "50, 34" && self.saved.settings.sprockets == CASSETTES[0].1,
                                1 => self.saved.settings.chainrings == "40" && self.saved.settings.sprockets == CASSETTES[2].1,
                                _ => self.saved.settings.chainrings == "32" && self.saved.settings.sprockets == CASSETTES[3].1,
                            };
                            if choice_button(ui, *name, matches, 72.0).clicked() { self.apply_preset(index); }
                        }
                    });
                    ui.add_space(8.0);
                    ui.separator();
                    ui.strong("Front gears (chainrings)");
                    let response = ui.add(egui::TextEdit::singleline(&mut self.saved.settings.chainrings)
                        .hint_text("50, 34").desired_width(f32::INFINITY).margin(egui::vec2(8.0, 7.0)));
                    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Chainring teeth"));
                    theme::muted(ui, "Tooth counts, separated by commas.");
                    ui.add_space(8.0);
                    ui.strong("Rear gears (cassette)");
                    let current = CASSETTES.iter().find(|(_, teeth)| *teeth == self.saved.settings.sprockets)
                        .map(|(name, _)| *name).unwrap_or("Custom cassette");
                    egui::ComboBox::from_id_salt("cassette_preset").selected_text(current).width(ui.available_width())
                        .show_ui(ui, |ui| {
                            for (name, teeth) in CASSETTES {
                                ui.selectable_value(&mut self.saved.settings.sprockets, teeth.to_string(), *name);
                            }
                        }).response.on_hover_text("Choose an example cassette, or edit its individual tooth counts below.");
                    let response = ui.add(egui::TextEdit::multiline(&mut self.saved.settings.sprockets)
                        .desired_rows(2).desired_width(f32::INFINITY).margin(egui::vec2(8.0, 7.0))
                        .hint_text("11, 13, 15, 17, 19, 21"));
                    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Cassette teeth"));
                    theme::muted(ui, "Edit individual cogs, or enter a range: 9-60.");
                    ui.add_space(8.0);
                    ui.strong("Wheel & tire size");
                    theme::muted(ui, "Choose the size printed on your tire.");
                    let wheel_name = WheelPreset::ALL.get(self.saved.settings.wheel)
                        .map(|p| p.name).unwrap_or("Measured circumference");
                    egui::ComboBox::from_id_salt("wheel_preset").selected_text(wheel_name).width(ui.available_width())
                        .show_ui(ui, |ui| {
                            for (index, wheel) in WheelPreset::ALL.iter().enumerate() {
                                ui.selectable_value(&mut self.saved.settings.wheel, index, wheel.name);
                            }
                            ui.selectable_value(&mut self.saved.settings.wheel, WheelPreset::ALL.len(), "Measured circumference");
                        });
                    if self.saved.settings.wheel >= WheelPreset::ALL.len() {
                        ui.horizontal(|ui| {
                            let response = ui.add(egui::TextEdit::singleline(&mut self.saved.settings.circumference)
                                .desired_width(155.0).hint_text("2130").margin(egui::vec2(8.0, 7.0)));
                            response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Wheel circumference in millimeters"));
                            ui.label("mm");
                        });
                        theme::muted(ui, "Distance rolled in one full wheel turn. Measure with the tire inflated.");
                    } else if let Ok(mm) = self.saved.settings.wheel_mm() {
                        theme::muted(ui, format!("Estimated circumference: {mm:.0} mm per wheel turn."));
                    }
                    self.refresh();
                    if let Some(error) = &self.error {
                        egui::Frame::new().fill(p.error_background).corner_radius(6).inner_margin(10)
                            .show(ui, |ui| { ui.colored_label(p.error, error); });
                    }
                    ui.add_space(10.0);
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Reset setup").on_hover_text("Restore the road example and default cadence and units").clicked() {
                            self.saved.settings = Settings::default();
                            self.saved.active_setup = None;
                            self.refresh();
                        }
                        if ui.button("Clear").on_hover_text("Clear front and rear tooth counts").clicked() {
                            self.saved.settings.chainrings.clear();
                            self.saved.settings.sprockets.clear();
                            self.saved.active_setup = None;
                            self.refresh();
                        }
                    });
                });
            });
    }

    fn select_wheel_containing(&mut self, text: &str) {
        if let Some(index) = WheelPreset::ALL
            .iter()
            .position(|wheel| wheel.name.contains(text))
        {
            self.saved.settings.wheel = index;
        }
    }

    fn workspace(&mut self, ui: &mut egui::Ui) {
        let p = theme::palette(ui);
        egui::CentralPanel::default().frame(egui::Frame::new().fill(p.background).inner_margin(18))
            .show(ui, |ui| {
                ui.heading("2. Gear results");
                // Keep navigation visible while scrolling through a long table or diagram.
                ui.horizontal_wrapped(|ui| {
                    for (tab, label) in [
                        (ResultsTab::Gears, "Gears"),
                        (ResultsTab::Cadence, "Cadence"),
                        (ResultsTab::Chart, "Chart"),
                        (ResultsTab::Drivetrain, "Drivetrain"),
                        (ResultsTab::Compare, "Compare"),
                    ] {
                        if choice_button(ui, label, self.tab == tab, 0.0).clicked() {
                            self.tab = tab;
                        }
                    }
                });
                ui.add_space(8.0);
                egui::ScrollArea::vertical().id_salt(("workspace_scroll", self.tab)).auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.riding_controls(ui);
                        if self.rows.is_empty() {
                            theme::card(ui).show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                ui.strong("Complete the bike setup");
                                ui.label("Choose an example or enter the front and rear tooth counts on the left.");
                                if let Some(error) = &self.error { ui.colored_label(p.error, error); }
                            });
                            return;
                        }
                        self.selected_gear(ui);
                        ui.add_space(12.0);
                        match self.tab {
                            ResultsTab::Gears | ResultsTab::Cadence => self.results_table(ui),
                            ResultsTab::Chart => {
                                theme::card(ui).show(ui, |ui| {
                                    ui.strong(format!("Speed at {:.0} rpm", self.saved.settings.cadence));
                                    theme::muted(ui, "Each line is a front gear. Select a point to inspect a front/rear combination.");
                                    chart::speed_chart(ui, &self.rows, self.saved.settings.cadence, self.saved.settings.unit(), &mut self.selected);
                                });
                            }
                            ResultsTab::Compare => self.comparison(ui),
                            ResultsTab::Drivetrain => {
                                theme::card(ui).show(ui, |ui| {
                                    let mut export = false;
                                    ui.horizontal(|ui| {
                                        ui.strong("Chainrings & cassette");
                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                            export = ui.button("Export SVG").on_hover_text("Save this drivetrain diagram as a scalable vector image").clicked();
                                        });
                                    });
                                    theme::muted(ui, "Select a cog to inspect that gear combination. Labels show tooth counts.");
                                    if let Some(setup) = &self.setup {
                                        self.drivetrain.show(ui, setup, &mut self.selected);
                                    }
                                    if export {
                                        self.export_drivetrain();
                                    }
                                });
                            }
                        }
                    });
            });
    }

    fn riding_controls(&mut self, ui: &mut egui::Ui) {
        if ui.available_width() < 650.0 {
            ui.horizontal(|ui| self.cadence_control(ui));
            ui.horizontal(|ui| self.unit_controls(ui));
        } else {
            ui.horizontal(|ui| {
                self.cadence_control(ui);
                ui.add_space(12.0);
                self.unit_controls(ui);
            });
        }
        ui.add_space(8.0);
    }

    fn cadence_control(&mut self, ui: &mut egui::Ui) {
        let p = theme::palette(ui);
        ui.label("Pedaling rate (cadence)");
        ui.scope(|ui| {
            ui.visuals_mut().widgets.inactive.bg_fill = p.border;
            ui.visuals_mut().selection.bg_fill = p.accent;
            ui.add(
                egui::Slider::new(&mut self.saved.settings.cadence, 30.0..=150.0)
                    .step_by(1.0)
                    .suffix(" rpm")
                    .trailing_fill(true)
                    .handle_shape(egui::style::HandleShape::Circle),
            );
        });
    }

    fn unit_controls(&mut self, ui: &mut egui::Ui) {
        ui.label("Speed units:");
        ui.selectable_value(&mut self.saved.settings.imperial, false, "km/h");
        ui.selectable_value(&mut self.saved.settings.imperial, true, "mph");
    }

    fn selected_gear(&mut self, ui: &mut egui::Ui) {
        let Some((mut front, mut rear)) = self.selected else {
            return;
        };
        let p = theme::palette(ui);
        let unit = self.saved.settings.unit();
        let setup = self.setup.clone().expect("Only displayed for valid input");
        theme::card(ui).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.strong("Inspect a gear");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let ratio = f64::from(front) / f64::from(rear);
                    let harder = self.rows.iter().find(|row| row.ratio > ratio);
                    let easier = self.rows.iter().rfind(|row| row.ratio < ratio);
                    if ui.add_enabled(harder.is_some(), Button::new("Harder")).on_hover_text("Next higher gear ratio").clicked() {
                        let next = harder.unwrap();
                        (front, rear) = (next.front_teeth, next.rear_teeth);
                    }
                    if ui.add_enabled(easier.is_some(), Button::new("Easier")).on_hover_text("Next lower gear ratio").clicked() {
                        let next = easier.unwrap();
                        (front, rear) = (next.front_teeth, next.rear_teeth);
                    }
                });
            });
            ui.add_space(3.0);
            gear_choices(ui, "Front", &setup.chainrings, &mut front);
            gear_choices(ui, "Rear", &setup.sprockets, &mut rear);
            self.selected = Some((front, rear));
            let row = self.rows.iter().find(|r| r.front_teeth == front && r.rear_teeth == rear).copied().unwrap();
            let compact = self.tab == ResultsTab::Drivetrain;
            ui.add_space(6.0);
            ui.columns(2, |columns| {
                columns[0].label(format!("Speed at {:.0} rpm", self.saved.settings.cadence));
                columns[0].label(RichText::new(format!("{:.1} {}", unit.from_kmh(row.speed_kmh(self.saved.settings.cadence)), unit.label()))
                    .size(30.0).strong().color(p.accent));
                columns[1].label("Distance per pedal turn");
                columns[1].label(RichText::new(format!("{:.2} m", row.development_m)).size(30.0).strong());
            });
            if compact { return; }
            ui.label(format!("Ratio {:.2}: one pedal turn rotates the rear wheel {:.2} times.", row.ratio, row.ratio));
            egui::CollapsingHeader::new("Technical details").show(ui, |ui| {
                let summary = setup.summary().unwrap();
                ui.label(format!("Selected gear: {:.1} gear inches. Equivalent wheel diameter used to compare gearing.", row.gear_inches));
                ui.label(format!("Overall range: {:.0}% (highest ratio ÷ lowest ratio × 100).", summary.range_percent));
                ui.label(format!("Lowest ratio: {:.2} ({} / {}). Highest ratio: {:.2} ({} / {}).", summary.lowest.ratio,
                    summary.lowest.front_teeth, summary.lowest.rear_teeth, summary.highest.ratio, summary.highest.front_teeth, summary.highest.rear_teeth));
                if ui.small_button("Copy gear").clicked() {
                    ui.ctx().copy_text(export_tsv(&[row], self.saved.settings.cadence, unit));
                    self.notify("Selected gear copied.", false);
                }
            });
        });
    }

    fn results_table(&mut self, ui: &mut egui::Ui) {
        let p = theme::palette(ui);
        let cadence_tab = self.tab == ResultsTab::Cadence;
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("{} gear combinations", self.rows.len()));
            if !cadence_tab {
                ui.checkbox(&mut self.saved.technical_columns, "Show gear inches");
            }
            if ui
                .button("Copy table")
                .on_hover_text("Copy all gear metrics and reference cadences · Ctrl/Cmd+C")
                .clicked()
            {
                self.copy_table(ui.ctx());
            }
            if ui
                .add(Button::new(RichText::new("Export CSV").color(p.accent_text)).fill(p.accent))
                .clicked()
            {
                self.export();
            }
        });
        if cadence_tab {
            theme::muted(
                ui,
                "Required pedal turns per minute (rpm) to ride at each column's speed.",
            );
        } else {
            theme::muted(
                ui,
                "Easiest to hardest. Select a row to inspect it. Front / rear are tooth counts; m / pedal turn is distance traveled.",
            );
        }
        let cadence = self.saved.settings.cadence;
        let unit = self.saved.settings.unit();
        let mut headings: Vec<String> = if cadence_tab {
            std::iter::once("Front / rear".into())
                .chain(
                    SPEED_THRESHOLDS_KMH
                        .iter()
                        .map(|s| format!("{:.1} {}", unit.from_kmh(*s), unit.label())),
                )
                .collect()
        } else {
            vec![
                "Front / rear".into(),
                "Ratio".into(),
                format!("Speed ({})", unit.label()),
                "m / pedal turn".into(),
            ]
        };
        if !cadence_tab && self.saved.technical_columns {
            headings.push("Gear inches".into());
        }
        let table_height = ui.available_height().max(220.0);
        egui::ScrollArea::horizontal()
            .id_salt("table_horizontal")
            .show(ui, |ui| {
                ui.visuals_mut().faint_bg_color = p.surface;
                ui.set_min_width(if cadence_tab {
                    590.0
                } else if self.saved.technical_columns {
                    550.0
                } else {
                    410.0
                });
                TableBuilder::new(ui)
                    .id_salt(("results_table", cadence_tab, self.saved.technical_columns))
                    .striped(true)
                    .sense(Sense::click())
                    .cell_layout(Layout::left_to_right(Align::Center))
                    .columns(Column::remainder().at_least(80.0), headings.len())
                    .max_scroll_height(table_height)
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .header(32.0, |mut header| {
                        for heading in &headings {
                            header.col(|ui| {
                                ui.label(RichText::new(heading).strong().color(p.muted));
                            });
                        }
                    })
                    .body(|body| {
                        body.rows(32.0, self.rows.len(), |mut table_row| {
                            let row = self.rows[table_row.index()];
                            let key = (row.front_teeth, row.rear_teeth);
                            table_row.set_selected(self.selected == Some(key));
                            table_row.col(|ui| {
                                if ui
                                    .selectable_label(
                                        self.selected == Some(key),
                                        format!("{} / {}", row.front_teeth, row.rear_teeth),
                                    )
                                    .clicked()
                                {
                                    self.selected = Some(key);
                                }
                            });
                            let mut values: Vec<String> = if cadence_tab {
                                SPEED_THRESHOLDS_KMH
                                    .iter()
                                    .map(|speed| format!("{:.1}", row.cadence_at(*speed)))
                                    .collect()
                            } else {
                                vec![
                                    format!("{:.2}", row.ratio),
                                    format!("{:.1}", unit.from_kmh(row.speed_kmh(cadence))),
                                    format!("{:.2}", row.development_m),
                                ]
                            };
                            if !cadence_tab && self.saved.technical_columns {
                                values.push(format!("{:.1}", row.gear_inches));
                            }
                            for value in &values {
                                table_row.col(|ui| {
                                    ui.label(RichText::new(value).monospace());
                                });
                            }
                            if table_row.response().clicked() {
                                self.selected = Some(key);
                            }
                        });
                    });
            });
        theme::muted(
            ui,
            "Calculated speed depends on gearing and cadence. It does not predict how fast you can ride.",
        );
    }

    fn comparison(&mut self, ui: &mut egui::Ui) {
        ui.label("Keep a reference to compare with. Then edit the bike or load one of your saved setups.");
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(if self.saved.baseline.is_some() {
                    "Replace reference"
                } else {
                    "Use current as reference"
                })
                .clicked()
            {
                self.saved.baseline = Some(self.saved.settings.clone());
                self.notify(
                    "Comparison reference set. Edit the bike or load a saved setup to see the difference.",
                    false,
                );
            }
            if self.saved.baseline.is_some() && ui.button("Clear reference").clicked() {
                self.saved.baseline = None;
            }
        });
        if let Some(baseline) = &self.saved.baseline
            && let Ok(setup) = baseline.setup()
        {
            let original = setup.summary().unwrap();
            let current = self.saved.settings.setup().unwrap().summary().unwrap();
            let unit = self.saved.settings.unit();
            let cadence = self.saved.settings.cadence;
            theme::card(ui).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.strong("Reference compared with current setup");
                ui.label(format!("Reference: {} teeth front · {}–{} teeth rear · {:.0} mm wheel circumference",
                    baseline.chainrings, setup.sprockets.first().unwrap(), setup.sprockets.last().unwrap(), setup.circumference_mm));
                ui.add_space(4.0);
                egui::ScrollArea::horizontal().id_salt("comparison_scroll").show(ui, |ui| {
                    egui::Grid::new("comparison_grid").striped(true).min_col_width(75.0).spacing([20.0, 12.0]).show(ui, |ui| {
                        for title in ["Metric", "Reference", "Current", "Change"] { ui.strong(title); }
                        ui.end_row();
                        for (label, saved, now) in [
                            ("Lowest gear (m / pedal turn)".to_owned(), original.lowest.development_m, current.lowest.development_m),
                            (format!("Highest gear ({} at {cadence:.0} rpm)", unit.label()), unit.from_kmh(original.highest.speed_kmh(cadence)), unit.from_kmh(current.highest.speed_kmh(cadence))),
                        ] {
                            ui.label(label); ui.label(format!("{saved:.2}")); ui.label(format!("{now:.2}"));
                            ui.label(format!("{:+.1}%", (now / saved - 1.0) * 100.0)); ui.end_row();
                        }
                    });
                });
                ui.add_space(4.0);
                ui.label("A shorter distance per pedal turn means lower gearing, useful for climbing. A higher speed in the highest gear means more distance at the same pedaling rate.");
                theme::muted(ui, "Both setups use the current cadence. This comparison includes changes in wheel size.");
            });
        }
    }

    fn about(&mut self, ctx: &egui::Context) {
        egui::Window::new("Help & definitions").open(&mut self.show_about).resizable(true)
            .collapsible(false).default_width(540.0).show(ctx, |ui| {
                egui::ScrollArea::vertical().max_height(560.0).show(ui, |ui| {
                    ui.heading("Using the calculator");
                    ui.label("1. Choose a bike example or enter your front gears, rear gears, and tire size.");
                    ui.label("2. Set your pedaling rate. Click Front and Rear tooth-count buttons to choose a gear, or use Easier / Harder to step through the ratios. Table rows are also selectable.");
                    ui.label("3. Use the tabs for all gear combinations, cadence at fixed speeds, a speed chart, the drivetrain diagram, or setup comparison.");
                    ui.label("4. Choose Save setup to give your bike a name. Load it later from Saved setups. Edits stay in your current inputs until you choose Update; Save as creates a separate setup.");
                    ui.label("The moon/sun button beside Help changes the theme. In Drivetrain, select a cog to inspect it, or export the diagram as SVG.");
                    ui.separator();
                    for (term, definition) in [
                        ("Teeth", "The points around a gear that engage the chain. A 50-tooth front gear and 25-tooth rear gear give a ratio of 2.00."),
                        ("Cadence (rpm)", "Complete pedal turns per minute. At 90 rpm, the pedals turn 90 times each minute."),
                        ("Gear ratio", "Front teeth divided by rear teeth. A ratio of 2.00 means two wheel turns per complete pedal turn."),
                        ("Distance per pedal turn", "How far the bike travels for one complete pedal turn. Also called development. Lower values mean lower gearing, useful for climbing."),
                        ("Gear inches", "A traditional gearing measure: wheel diameter in inches multiplied by gear ratio. It is an equivalent diameter, not distance traveled."),
                        ("Gear range", "Highest gear ratio divided by lowest gear ratio, multiplied by 100. A 10–50 cassette with one front gear has a 500% range."),
                        ("Wheel circumference", "Distance rolled in one full wheel turn, measured in millimeters. Tire presets estimate this; a measured rollout is more accurate."),
                    ] { ui.strong(term); ui.label(definition); }
                    ui.separator();
                    egui::CollapsingHeader::new("Formulas and assumptions").show(ui, |ui| {
                        ui.label("Ratio = front teeth ÷ rear teeth");
                        ui.label("Distance per pedal turn = wheel circumference × ratio");
                        ui.label("Speed (km/h) = distance per pedal turn (m) × cadence × 0.06");
                        ui.label("Wheel presets estimate circumference from rim diameter plus twice the tire width. Actual rollout varies with tire shape, pressure, and load.");
                        ui.label("All front/rear combinations are shown. This does not check chainline, derailleur capacity, or component compatibility. Calculated speeds do not account for power, gradient, wind, or drag.");
                    });
                    ui.label("Ctrl/Cmd+C copies all gears when you are not editing text. Ctrl/Cmd+S exports CSV. Tab moves between controls.");
                    theme::muted(ui, format!("CogCalc {} · GPL-3.0 · No warranty. You may modify and redistribute under the included license.", env!("CARGO_PKG_VERSION")));
                    egui::CollapsingHeader::new("License").show(ui, |ui| { ui.label(include_str!("../LICENSE")); });
                });
            });
    }
}

impl eframe::App for GearApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.refresh();
        // Header first so a theme change applies to the rest of the interface immediately.
        self.top_bar(ui);
        let p = theme::palette(ui);
        egui::Panel::bottom("status")
            .exact_size(28.0)
            .frame(
                egui::Frame::new()
                    .fill(p.surface)
                    .inner_margin(egui::Margin::symmetric(18, 5)),
            )
            .show(ui, |ui| {
                if let Some((message, error, started)) = &self.notice {
                    ui.label(RichText::new(message).size(12.0).color(if *error {
                        p.error
                    } else {
                        p.accent
                    }));
                    if !error {
                        let remaining = Duration::from_secs(8).saturating_sub(started.elapsed());
                        if remaining.is_zero() {
                            self.notice = None;
                        } else {
                            ctx.request_repaint_after(remaining);
                        }
                    }
                } else {
                    theme::muted(
                        ui,
                        if self.error.is_none() {
                            "Current inputs saved automatically on this device."
                        } else {
                            "Check the bike setup to calculate results."
                        },
                    );
                }
            });
        self.sidebar(ui);
        self.workspace(ui);
        self.about(&ctx);
        let dialog_open = self.setup_dialog.is_some();
        self.setup_dialog(&ctx);
        if self.persist_library
            && let Some(storage) = frame.storage_mut()
        {
            eframe::set_value(storage, STORAGE_KEY, &self.saved);
            storage.flush();
            self.persist_library = false;
        }
        if !dialog_open
            && !ctx.text_edit_focused()
            && ctx.input(|input| {
                input
                    .events
                    .iter()
                    .any(|event| matches!(event, egui::Event::Copy))
            })
        {
            self.copy_table(&ctx);
        }
        if !dialog_open
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::S))
        {
            self.export();
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, STORAGE_KEY, &self.saved);
    }
}
