use std::collections::HashMap;

use eframe::App as _;
use eframe::egui::Vec2;

use super::*;

const DESKTOP: Vec2 = egui::vec2(1220.0, 960.0);

#[derive(Default)]
struct MemoryStorage(HashMap<String, String>);

impl eframe::Storage for MemoryStorage {
    fn get_string(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }

    fn set_string(&mut self, key: &str, value: String) {
        self.0.insert(key.into(), value);
    }

    fn remove_string(&mut self, key: &str) {
        self.0.remove(key);
    }

    fn flush(&mut self) {}
}

fn new_app() -> (egui::Context, GearApp) {
    let ctx = egui::Context::default();
    let cc = eframe::CreationContext::_new_kittest(ctx.clone());
    let app = GearApp::new(&cc);
    (ctx, app)
}

fn render(
    ctx: &egui::Context,
    app: &mut GearApp,
    size: Vec2,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            focused: true,
            events,
            ..Default::default()
        },
        |ui| app.ui(ui, &mut eframe::Frame::_new_kittest()),
    );
    // The headless harness inspects geometry and commands without a GPU texture manager.
    output.textures_delta.clear();
    output
}

fn shape_text_rect(shape: &egui::Shape, label: &str) -> Option<egui::Rect> {
    match shape {
        egui::Shape::Text(text) if text.galley.text() == label => Some(text.visual_bounding_rect()),
        egui::Shape::Vec(shapes) => shapes
            .iter()
            .find_map(|shape| shape_text_rect(shape, label)),
        _ => None,
    }
}

fn text_rect(output: &egui::FullOutput, label: &str) -> egui::Rect {
    visible_text_rect(output, label).unwrap_or_else(|| panic!("No visible text for {label:?}"))
}

fn visible_text_rect(output: &egui::FullOutput, label: &str) -> Option<egui::Rect> {
    output.shapes.iter().find_map(|clipped| {
        shape_text_rect(&clipped.shape, label)
            .filter(|rect| clipped.clip_rect.contains(rect.center()))
    })
}

fn click(ctx: &egui::Context, app: &mut GearApp, size: Vec2, label: &str) -> egui::FullOutput {
    // A full press/release sequence uses the same hit testing as the native app.
    // Newly opened modal areas need a measurement frame before their final layout.
    render(ctx, app, size, Vec::new());
    let output = render(ctx, app, size, Vec::new());
    let pos = text_rect(&output, label).center();
    click_position(ctx, app, size, pos)
}

fn click_id(ctx: &egui::Context, app: &mut GearApp, size: Vec2, id: egui::Id) -> egui::FullOutput {
    render(ctx, app, size, Vec::new());
    render(ctx, app, size, Vec::new());
    let response = ctx.read_response(id).expect("The control must be rendered");
    assert!(response.enabled(), "The control must be enabled");
    click_position(ctx, app, size, response.rect.center())
}

fn toggle_theme(ctx: &egui::Context, app: &mut GearApp, size: Vec2) -> egui::FullOutput {
    let was_dark = app.saved.dark_mode;
    let output = click_id(ctx, app, size, egui::Id::new(theme::THEME_TOGGLE_ID));
    assert_eq!(app.saved.dark_mode, !was_dark);
    output
}

fn enter_setup_name(ctx: &egui::Context, app: &mut GearApp, size: Vec2, name: &str) {
    click_id(ctx, app, size, egui::Id::new("saved_setup_name"));
    render(
        ctx,
        app,
        size,
        vec![
            key_press(egui::Key::A, egui::Modifiers::COMMAND),
            egui::Event::Text(name.to_owned()),
        ],
    );
}

fn save_setup_from_dialog(ctx: &egui::Context, app: &mut GearApp, size: Vec2, name: &str) {
    let label = if app.saved.active_setup.is_some() {
        "Save as…"
    } else {
        "Save setup…"
    };
    click(ctx, app, size, label);
    enter_setup_name(ctx, app, size, name);
    click(ctx, app, size, "Save");
    assert!(
        app.setup_dialog.is_none(),
        "A valid named setup should save"
    );
}

fn click_position(
    ctx: &egui::Context,
    app: &mut GearApp,
    size: Vec2,
    pos: egui::Pos2,
) -> egui::FullOutput {
    render(
        ctx,
        app,
        size,
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    render(
        ctx,
        app,
        size,
        vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    )
}

fn key_press(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn edit_cadence(
    ctx: &egui::Context,
    app: &mut GearApp,
    size: Vec2,
    cadence: u16,
) -> egui::FullOutput {
    let output = render(ctx, app, size, Vec::new());
    let value = format!("{:.1}", app.saved.settings.cadence);
    // DragValue paints the number and suffix as separate atoms in egui 0.36.
    let value_rect = visible_text_rect(&output, &value)
        .or_else(|| visible_text_rect(&output, &format!("{value} rpm")))
        .expect("The cadence slider must expose an editable numeric value");
    click_position(ctx, app, size, value_rect.center());
    render(ctx, app, size, Vec::new());
    assert!(
        ctx.text_edit_focused(),
        "Clicking cadence must allow typing"
    );
    render(
        ctx,
        app,
        size,
        vec![
            key_press(egui::Key::A, egui::Modifiers::COMMAND),
            egui::Event::Text(cadence.to_string()),
        ],
    );
    render(
        ctx,
        app,
        size,
        vec![key_press(egui::Key::Enter, egui::Modifiers::NONE)],
    );
    render(ctx, app, size, Vec::new())
}

fn assert_finite_geometry(ctx: &egui::Context, output: &egui::FullOutput) {
    let primitives = ctx.tessellate(output.shapes.clone(), output.pixels_per_point);
    let mut vertices = 0;
    for primitive in primitives {
        assert!(primitive.clip_rect.is_finite());
        if let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive {
            vertices += mesh.vertices.len();
            assert!(mesh.vertices.iter().all(|vertex| vertex.pos.is_finite()));
            assert!(mesh.is_valid());
        }
    }
    assert!(
        vertices > 100,
        "The app should produce real renderable content"
    );
}

fn current_palette(ctx: &egui::Context) -> theme::Palette {
    let mut palette = None;
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        palette = Some(theme::palette(ui));
    });
    output.textures_delta.clear();
    palette.unwrap()
}

fn shape_has_fill(shape: &egui::Shape, fill: egui::Color32) -> bool {
    match shape {
        egui::Shape::Rect(rect) => rect.fill == fill,
        egui::Shape::Vec(shapes) => shapes.iter().any(|shape| shape_has_fill(shape, fill)),
        _ => false,
    }
}

fn assert_palette_is_painted(output: &egui::FullOutput, palette: theme::Palette) {
    for fill in [palette.background, palette.surface] {
        assert!(
            output
                .shapes
                .iter()
                .any(|clipped| shape_has_fill(&clipped.shape, fill)),
            "The rendered interface must use its selected theme's background and surface colors"
        );
    }
}

#[test]
fn workspace_renders_at_default_and_minimum_window_sizes() {
    for size in [DESKTOP, egui::vec2(820.0, 600.0)] {
        let (ctx, mut app) = new_app();
        // Let egui measure the first frame, then inspect the stable layout.
        render(&ctx, &mut app, size, Vec::new());
        let output = render(&ctx, &mut app, size, Vec::new());
        assert_eq!(app.rows.len(), 22);
        assert!(app.error.is_none());
        for label in [APP_NAME, "1. Bike setup", "2. Gear results", "Help"] {
            let rect = text_rect(&output, label);
            assert!(
                rect.left() >= 0.0 && rect.right() <= size.x,
                "{label} overflowed"
            );
        }
        let toggle = ctx
            .read_response(egui::Id::new(theme::THEME_TOGGLE_ID))
            .expect("The theme icon must be available at every window size");
        assert!(toggle.rect.left() >= 0.0 && toggle.rect.right() <= size.x);
        assert!(toggle.rect.top() >= 0.0 && toggle.rect.bottom() <= size.y);
        assert_finite_geometry(&ctx, &output);
    }
}

#[test]
fn incomplete_or_invalid_edits_clear_stale_results_and_recover() {
    let (ctx, mut app) = new_app();
    let mut invalid = vec![
        Settings {
            chainrings: String::new(),
            ..Settings::default()
        },
        Settings {
            sprockets: "11, banana, 32".into(),
            ..Settings::default()
        },
    ];
    invalid.extend(
        ["", "NaN", "inf", "-20", "0", "not a number"].map(|circumference| Settings {
            wheel: WheelPreset::ALL.len(),
            circumference: circumference.into(),
            ..Settings::default()
        }),
    );
    for settings in invalid {
        app.saved.settings = settings;
        let output = render(&ctx, &mut app, DESKTOP, Vec::new());
        assert!(app.error.is_some());
        assert!(
            app.rows.is_empty(),
            "An invalid edit must not leave old calculations visible"
        );
        assert!(app.selected.is_none());
        text_rect(&output, "Complete the bike setup");

        app.saved.settings = Settings::default();
        render(&ctx, &mut app, DESKTOP, Vec::new());
        assert!(app.error.is_none());
        assert_eq!(app.rows.len(), 22);
        assert!(app.selected.is_some());
    }
}

#[test]
fn preset_unit_and_cadence_controls_update_the_live_workspace() {
    let (ctx, mut app) = new_app();
    click(&ctx, &mut app, DESKTOP, "Gravel");
    assert_eq!(app.rows.len(), 11);
    assert!(app.rows.iter().all(|row| row.front_teeth == 40));
    assert_eq!(app.rows.iter().map(|row| row.rear_teeth).max(), Some(42));
    assert!(
        WheelPreset::ALL[app.saved.settings.wheel]
            .name
            .contains("40")
    );

    click(&ctx, &mut app, DESKTOP, "mph");
    assert!(app.saved.settings.imperial);
    let output = edit_cadence(&ctx, &mut app, DESKTOP, 100);
    assert_eq!(app.saved.settings.cadence, 100.0);
    text_rect(&output, "Speed at 100 rpm");

    click(&ctx, &mut app, DESKTOP, "MTB");
    assert_eq!(app.rows.len(), 12);
    assert!(app.rows.iter().all(|row| row.front_teeth == 32));
    assert_eq!(app.rows.iter().map(|row| row.rear_teeth).max(), Some(51));
    assert_eq!(app.saved.settings.cadence, 100.0);
    assert!(app.saved.settings.imperial);

    click(&ctx, &mut app, DESKTOP, "Road");
    assert_eq!(app.rows.len(), 22);
    assert_eq!(app.saved.settings.cadence, 100.0);
    assert!(app.saved.settings.imperial);
}

#[test]
fn comparison_and_preferences_survive_a_storage_roundtrip() {
    let (ctx, mut app) = new_app();
    let size = egui::vec2(1220.0, 1280.0);
    click(&ctx, &mut app, size, "Compare setups");
    assert_eq!(app.tab, ResultsTab::Compare);
    click(&ctx, &mut app, size, "Use current as reference");
    assert!(app.saved.baseline.is_some());
    click(&ctx, &mut app, size, "Gravel");
    click(&ctx, &mut app, size, "mph");
    edit_cadence(&ctx, &mut app, size, 100);
    let mut storage = MemoryStorage::default();
    app.save(&mut storage);

    let saved: SavedState = eframe::get_value(&storage, STORAGE_KEY).unwrap();
    assert_eq!(saved.settings.chainrings, "40");
    assert_eq!(saved.baseline.unwrap().chainrings, "50, 34");
    let restored_ctx = egui::Context::default();
    let mut cc = eframe::CreationContext::_new_kittest(restored_ctx.clone());
    cc.storage = Some(&storage);
    let mut restored = GearApp::new(&cc);
    assert_eq!(restored.rows.len(), 11);
    assert_eq!(restored.saved.settings.cadence, 100.0);
    assert!(restored.saved.settings.imperial);
    assert_eq!(restored.tab, ResultsTab::Gears);
    let output = click(&restored_ctx, &mut restored, size, "Compare setups");
    text_rect(&output, "Reference compared with current setup");
    assert!(
        restored
            .rows
            .iter()
            .any(|row| { restored.selected == Some((row.front_teeth, row.rear_teeth)) })
    );
}

#[test]
fn dark_mode_and_technical_columns_survive_presets_reset_and_restart() {
    let (ctx, mut app) = new_app();
    assert!(!app.saved.dark_mode);
    assert!(!ctx.global_style().visuals.dark_mode);
    let light = current_palette(&ctx);

    toggle_theme(&ctx, &mut app, DESKTOP);
    assert!(app.saved.dark_mode);
    assert!(ctx.global_style().visuals.dark_mode);
    let dark = current_palette(&ctx);
    assert_ne!(light.background, dark.background);
    assert_ne!(light.surface, dark.surface);
    assert_ne!(light.text, dark.text);
    assert_ne!(light.chart_colors, dark.chart_colors);
    assert_eq!(ctx.global_style().visuals.panel_fill, dark.background);
    assert_eq!(
        ctx.global_style().visuals.override_text_color,
        Some(dark.text)
    );
    let output = render(&ctx, &mut app, DESKTOP, Vec::new());
    assert_palette_is_painted(&output, dark);

    click(&ctx, &mut app, DESKTOP, "Show gear inches");
    assert!(app.saved.technical_columns);
    for preset in ["Gravel", "MTB", "Road"] {
        click(&ctx, &mut app, DESKTOP, preset);
        assert!(app.saved.dark_mode);
        assert!(ctx.global_style().visuals.dark_mode);
        assert!(app.saved.technical_columns);
    }
    edit_cadence(&ctx, &mut app, DESKTOP, 100);
    click(&ctx, &mut app, DESKTOP, "mph");
    click(&ctx, &mut app, DESKTOP, "Reset setup");
    assert_eq!(app.saved.settings.cadence, 90.0);
    assert!(!app.saved.settings.imperial);
    assert!(app.saved.dark_mode);
    assert!(app.saved.technical_columns);

    let mut storage = MemoryStorage::default();
    app.save(&mut storage);
    let restored_ctx = egui::Context::default();
    let mut cc = eframe::CreationContext::_new_kittest(restored_ctx.clone());
    cc.storage = Some(&storage);
    let mut restored = GearApp::new(&cc);
    assert!(restored.saved.dark_mode);
    assert!(restored.saved.technical_columns);
    assert!(restored_ctx.global_style().visuals.dark_mode);
    assert_eq!(current_palette(&restored_ctx), dark);
    render(&restored_ctx, &mut restored, DESKTOP, Vec::new());
    let output = render(&restored_ctx, &mut restored, DESKTOP, Vec::new());
    text_rect(&output, "Gear inches");
    assert_palette_is_painted(&output, dark);

    toggle_theme(&restored_ctx, &mut restored, DESKTOP);
    assert!(!restored.saved.dark_mode);
    assert!(!restored_ctx.global_style().visuals.dark_mode);
    assert_eq!(current_palette(&restored_ctx), light);
}

#[test]
fn theme_icon_can_be_reached_and_activated_from_the_keyboard() {
    let (ctx, mut app) = new_app();
    let before = app.saved.settings.clone();
    let toggle_id = egui::Id::new(theme::THEME_TOGGLE_ID);
    render(&ctx, &mut app, DESKTOP, Vec::new());
    for _ in 0..5 {
        render(
            &ctx,
            &mut app,
            DESKTOP,
            vec![key_press(egui::Key::Tab, egui::Modifiers::NONE)],
        );
        if ctx.memory(|memory| memory.focused()) == Some(toggle_id) {
            break;
        }
    }
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(toggle_id));
    render(
        &ctx,
        &mut app,
        DESKTOP,
        vec![key_press(egui::Key::Space, egui::Modifiers::NONE)],
    );
    assert!(app.saved.dark_mode);
    assert!(ctx.global_style().visuals.dark_mode);
    assert!(app.saved.settings == before);
    assert_eq!(app.rows.len(), 22);
}

#[test]
fn named_setups_save_load_update_and_reload_without_mutating_other_snapshots() {
    let (ctx, mut app) = new_app();
    let size = egui::vec2(1220.0, 1280.0);
    app.saved.settings = Settings {
        chainrings: "46, 30".into(),
        sprockets: "11, 17, 34".into(),
        wheel: WheelPreset::ALL.len(),
        circumference: "2198".into(),
        cadence: 82.0,
        imperial: true,
    };
    let commuter = app.saved.settings.clone();
    save_setup_from_dialog(&ctx, &mut app, size, "  Daily commuter  ");
    assert_eq!(app.saved.named_setups.len(), 1);
    assert_eq!(app.saved.active_setup.as_deref(), Some("Daily commuter"));
    assert_eq!(app.saved.named_setups[0].name, "Daily commuter");
    assert!(app.saved.named_setups[0].settings == commuter);

    click(&ctx, &mut app, size, "Gravel");
    edit_cadence(&ctx, &mut app, size, 100);
    save_setup_from_dialog(&ctx, &mut app, size, "Gravel weekends");
    let gravel = app.saved.named_setups[1].settings.clone();
    assert_eq!(app.saved.named_setups.len(), 2);
    assert!(app.saved.named_setups[0].settings == commuter);

    click(&ctx, &mut app, size, "Gravel weekends");
    click(&ctx, &mut app, size, "Daily commuter");
    assert!(app.saved.settings == commuter);
    assert_eq!(app.rows.len(), 6);
    assert_eq!(app.saved.active_setup.as_deref(), Some("Daily commuter"));
    assert!(app.error.is_none());

    edit_cadence(&ctx, &mut app, size, 100);
    assert!(app.saved.named_setups[0].settings == commuter);
    let output = render(&ctx, &mut app, size, Vec::new());
    text_rect(&output, "Edited. Use Update to save these changes.");
    click(&ctx, &mut app, size, "Update");
    assert_eq!(app.saved.named_setups[0].settings.cadence, 100.0);
    assert!(app.saved.named_setups[1].settings == gravel);

    edit_cadence(&ctx, &mut app, size, 80);
    click(&ctx, &mut app, size, "Daily commuter");
    let output = render(&ctx, &mut app, size, Vec::new());
    // The current name also appears on the picker; choose its menu entry,
    // painted above the main interface, to explicitly reload the same setup.
    let menu_entry = output
        .shapes
        .iter()
        .rev()
        .find_map(|clipped| {
            shape_text_rect(&clipped.shape, "Daily commuter")
                .filter(|rect| clipped.clip_rect.contains(rect.center()))
        })
        .expect("The loaded setup must remain selectable in the picker");
    click_position(&ctx, &mut app, size, menu_entry.center());
    assert_eq!(app.saved.settings.cadence, 100.0);
    assert!(app.saved.named_setups[1].settings == gravel);

    click(&ctx, &mut app, size, "Compare setups");
    click(&ctx, &mut app, size, "Use current as reference");
    toggle_theme(&ctx, &mut app, size);
    click(&ctx, &mut app, size, "Reset setup");
    assert_eq!(app.saved.named_setups.len(), 2);
    assert!(app.saved.dark_mode);
    let mut storage = MemoryStorage::default();
    app.save(&mut storage);
    let restored_ctx = egui::Context::default();
    let mut cc = eframe::CreationContext::_new_kittest(restored_ctx.clone());
    cc.storage = Some(&storage);
    let mut restored = GearApp::new(&cc);
    assert_eq!(restored.saved.named_setups.len(), 2);
    assert!(restored.saved.settings == Settings::default());
    assert!(restored.saved.dark_mode);
    assert!(restored_ctx.global_style().visuals.dark_mode);
    assert_eq!(restored.saved.baseline.as_ref().unwrap().cadence, 100.0);
    restored.load_named_setup("Gravel weekends");
    assert!(restored.saved.settings == gravel);
    assert_eq!(restored.rows.len(), 11);
}

#[test]
fn invalid_or_duplicate_names_and_invalid_drafts_preserve_the_saved_library() {
    let (ctx, mut app) = new_app();
    let size = egui::vec2(1220.0, 1280.0);
    click(&ctx, &mut app, size, "Save setup…");
    click(&ctx, &mut app, size, "Save");
    let output = render(&ctx, &mut app, size, Vec::new());
    text_rect(&output, "Enter a name for this setup.");
    assert!(app.saved.named_setups.is_empty());
    assert!(app.setup_dialog.is_some());
    click(&ctx, &mut app, size, "Cancel");
    save_setup_from_dialog(&ctx, &mut app, size, "Commuter");
    let original = app.saved.named_setups[0].settings.clone();

    click(&ctx, &mut app, size, "Save as…");
    enter_setup_name(&ctx, &mut app, size, " commuter ");
    click(&ctx, &mut app, size, "Save");
    assert!(matches!(
        &app.setup_dialog,
        Some(SetupDialog::Save { error: Some(message), .. }) if message.contains("already in use")
    ));
    assert_eq!(app.saved.named_setups.len(), 1);
    assert!(app.saved.named_setups[0].settings == original);
    click(&ctx, &mut app, size, "Cancel");

    for invalid in ["   ".to_owned(), "a\nb".to_owned(), "車".repeat(81)] {
        assert!(app.save_named_setup(&invalid).is_err());
        assert_eq!(app.saved.named_setups.len(), 1);
        assert_eq!(app.saved.active_setup.as_deref(), Some("Commuter"));
    }
    app.saved.settings.chainrings = "not a gear".into();
    assert!(app.save_named_setup("Broken draft").is_err());
    assert!(app.update_named_setup().is_err());
    assert_eq!(app.saved.named_setups.len(), 1);
    assert!(app.saved.named_setups[0].settings == original);
    app.load_named_setup("Commuter");
    assert!(app.saved.settings == original);
    assert!(app.error.is_none());
}

#[test]
fn deleting_a_named_setup_requires_confirmation_and_keeps_current_inputs() {
    let (ctx, mut app) = new_app();
    let size = egui::vec2(1220.0, 1280.0);
    save_setup_from_dialog(&ctx, &mut app, size, "Commuter");
    let current = app.saved.settings.clone();
    click(&ctx, &mut app, size, "Delete");
    assert!(matches!(app.setup_dialog, Some(SetupDialog::Delete { .. })));
    assert_eq!(app.saved.named_setups.len(), 1);
    click(&ctx, &mut app, size, "Cancel");
    assert!(app.setup_dialog.is_none());
    assert_eq!(app.saved.named_setups.len(), 1);
    assert!(app.saved.settings == current);
    click(&ctx, &mut app, size, "Delete");
    click(&ctx, &mut app, size, "Delete setup");
    assert!(app.saved.named_setups.is_empty());
    assert!(app.saved.active_setup.is_none());
    assert!(app.saved.settings == current);
    assert_eq!(app.rows.len(), 22);
    let mut storage = MemoryStorage::default();
    app.save(&mut storage);
    let saved: SavedState = eframe::get_value(&storage, STORAGE_KEY).unwrap();
    assert!(saved.named_setups.is_empty());
    assert!(saved.settings == current);
}

#[test]
fn compact_saved_setup_picker_contains_and_loads_an_eighty_character_name() {
    let size = egui::vec2(820.0, 600.0);
    let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
    let name = "W".repeat(80);
    for dark_mode in [false, true] {
        let (ctx, mut app) = new_app();
        if dark_mode {
            toggle_theme(&ctx, &mut app, size);
        }
        app.save_named_setup(&name).unwrap();
        app.apply_preset(1);
        app.refresh();
        assert_eq!(app.rows.len(), 11);
        click(&ctx, &mut app, size, "Choose a saved setup");
        render(&ctx, &mut app, size, Vec::new());
        let output = render(&ctx, &mut app, size, Vec::new());
        assert!(egui::Popup::is_any_open(&ctx));
        let popup_rects: Vec<_> = ctx.memory(|memory| {
            memory
                .layer_ids()
                .filter(|layer| {
                    layer.order == egui::Order::Foreground && memory.areas().is_visible(layer)
                })
                .filter_map(|layer| memory.area_rect(layer.id))
                .collect()
        });
        assert!(!popup_rects.is_empty(), "The saved setup menu must open");
        for rect in popup_rects {
            assert!(
                viewport.contains_rect(rect),
                "The saved setup popup overflows 820×600 (dark mode: {dark_mode}): {rect:?}"
            );
        }
        let entry = text_rect(&output, &name);
        assert!(
            viewport.contains_rect(entry),
            "The long setup label must truncate within the viewport: {entry:?}"
        );
        click_position(&ctx, &mut app, size, entry.center());
        assert_eq!(app.saved.active_setup.as_deref(), Some(name.as_str()));
        assert_eq!(app.rows.len(), 22);
        assert_eq!(app.saved.settings.chainrings, "50, 34");
    }
}

#[test]
fn compact_saved_setup_modals_keep_fields_errors_and_actions_inside_the_viewport() {
    let size = egui::vec2(820.0, 600.0);
    let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
    for dark_mode in [false, true] {
        let (ctx, mut app) = new_app();
        if dark_mode {
            toggle_theme(&ctx, &mut app, size);
        }
        click(&ctx, &mut app, size, "Save setup…");
        click(&ctx, &mut app, size, "Save");
        render(&ctx, &mut app, size, Vec::new());
        let output = render(&ctx, &mut app, size, Vec::new());
        let modal = ctx
            .memory(|memory| memory.area_rect(egui::Id::new("saved_setup_dialog")))
            .expect("The Save modal must be visible");
        assert!(
            viewport.contains_rect(modal),
            "Save modal overflows: {modal:?}"
        );
        let field = ctx
            .read_response(egui::Id::new("saved_setup_name"))
            .unwrap();
        assert!(viewport.contains_rect(field.rect));
        for label in [
            "Save bike setup",
            "Enter a name for this setup.",
            "Save",
            "Cancel",
        ] {
            assert!(viewport.contains_rect(text_rect(&output, label)));
        }

        enter_setup_name(&ctx, &mut app, size, &"W".repeat(80));
        click(&ctx, &mut app, size, "Save");
        assert!(app.setup_dialog.is_none());
        click(&ctx, &mut app, size, "Delete");
        render(&ctx, &mut app, size, Vec::new());
        let output = render(&ctx, &mut app, size, Vec::new());
        let modal = ctx
            .memory(|memory| memory.area_rect(egui::Id::new("saved_setup_dialog")))
            .expect("The Delete modal must be visible");
        assert!(
            viewport.contains_rect(modal),
            "Delete modal overflows: {modal:?}"
        );
        for label in ["Delete saved setup?", "Delete setup", "Cancel"] {
            assert!(viewport.contains_rect(text_rect(&output, label)));
        }
        click(&ctx, &mut app, size, "Cancel");
        assert_eq!(app.saved.named_setups.len(), 1);
    }
}

#[test]
fn malformed_saved_entries_are_filtered_without_losing_valid_bikes_or_preferences() {
    let saved = SavedState {
        settings: Settings {
            chainrings: "42".into(),
            ..Settings::default()
        },
        baseline: Some(Settings::default()),
        dark_mode: true,
        technical_columns: true,
        named_setups: vec![
            NamedSetup {
                name: "  Road  ".into(),
                settings: Settings::default(),
            },
            NamedSetup {
                name: "ROAD".into(),
                settings: Settings::default(),
            },
            NamedSetup {
                name: " ".into(),
                settings: Settings::default(),
            },
            NamedSetup {
                name: "Broken".into(),
                settings: Settings {
                    chainrings: String::new(),
                    ..Settings::default()
                },
            },
            NamedSetup {
                name: "Gravel".into(),
                settings: Settings {
                    chainrings: "40".into(),
                    wheel: usize::MAX,
                    circumference: "2200".into(),
                    cadence: 999.0,
                    ..Settings::default()
                },
            },
        ],
        active_setup: Some("Missing bike".into()),
    };
    let mut storage = MemoryStorage::default();
    eframe::set_value(&mut storage, STORAGE_KEY, &saved);
    let ctx = egui::Context::default();
    let mut cc = eframe::CreationContext::_new_kittest(ctx.clone());
    cc.storage = Some(&storage);
    let mut app = GearApp::new(&cc);
    assert_eq!(app.saved.named_setups.len(), 2);
    assert_eq!(app.saved.named_setups[0].name, "Road");
    assert_eq!(app.saved.named_setups[1].name, "Gravel");
    assert!(app.saved.active_setup.is_none());
    assert_eq!(app.saved.settings.chainrings, "42");
    assert!(app.saved.baseline.is_some());
    assert!(app.saved.dark_mode);
    assert!(app.saved.technical_columns);
    app.load_named_setup("Gravel");
    assert_eq!(app.saved.settings.wheel, WheelPreset::ALL.len());
    assert_eq!(app.saved.settings.wheel_mm().unwrap(), 2200.0);
    assert_eq!(app.saved.settings.cadence, 150.0);
    assert!(app.error.is_none());
    assert_eq!(app.rows.len(), 11);
}

#[test]
fn legacy_saved_state_keeps_drivetrain_and_defaults_to_light_mode() {
    // This is the actual pre-theme storage shape, without either new preference.
    #[derive(Serialize)]
    struct LegacySavedState {
        settings: Settings,
        baseline: Option<Settings>,
    }
    let legacy = LegacySavedState {
        settings: Settings {
            chainrings: "46, 30".into(),
            sprockets: "11, 13, 34".into(),
            wheel: WheelPreset::ALL.len(),
            circumference: "2180".into(),
            cadence: 82.0,
            imperial: true,
        },
        baseline: Some(Settings::default()),
    };
    let mut storage = MemoryStorage::default();
    eframe::set_value(&mut storage, STORAGE_KEY, &legacy);
    let ctx = egui::Context::default();
    let mut cc = eframe::CreationContext::_new_kittest(ctx.clone());
    cc.storage = Some(&storage);
    let mut app = GearApp::new(&cc);
    assert_eq!(app.saved.settings.chainrings, "46, 30");
    assert_eq!(app.saved.settings.sprockets, "11, 13, 34");
    assert_eq!(app.saved.settings.wheel_mm().unwrap(), 2180.0);
    assert_eq!(app.saved.settings.cadence, 82.0);
    assert!(app.saved.settings.imperial);
    assert_eq!(app.rows.len(), 6);
    assert_eq!(app.saved.baseline.as_ref().unwrap().chainrings, "50, 34");
    assert!(!app.saved.dark_mode);
    assert!(!app.saved.technical_columns);
    assert!(app.saved.named_setups.is_empty());
    assert!(app.saved.active_setup.is_none());
    assert!(!ctx.global_style().visuals.dark_mode);
    assert_eq!(app.tab, ResultsTab::Gears);
    let output = render(&ctx, &mut app, DESKTOP, Vec::new());
    assert_finite_geometry(&ctx, &output);
}

#[test]
fn all_result_tabs_render_in_both_themes_at_minimum_size_without_taglines() {
    for dark_mode in [false, true] {
        for (label, expected_tab) in [
            ("Gear table", ResultsTab::Gears),
            ("Cadence table", ResultsTab::Cadence),
            ("Speed chart", ResultsTab::Chart),
            ("Drivetrain", ResultsTab::Drivetrain),
            ("Compare setups", ResultsTab::Compare),
        ] {
            let (ctx, mut app) = new_app();
            if dark_mode {
                toggle_theme(&ctx, &mut app, DESKTOP);
            }
            click(&ctx, &mut app, DESKTOP, label);
            assert_eq!(app.tab, expected_tab);
            if expected_tab == ResultsTab::Compare {
                click(&ctx, &mut app, DESKTOP, "Use current as reference");
                click(&ctx, &mut app, DESKTOP, "Gravel");
            }
            let palette = current_palette(&ctx);
            for size in [DESKTOP, egui::vec2(820.0, 600.0)] {
                render(&ctx, &mut app, size, Vec::new());
                let output = render(&ctx, &mut app, size, Vec::new());
                assert_eq!(ctx.global_style().visuals.dark_mode, dark_mode);
                assert_eq!(app.saved.dark_mode, dark_mode);
                assert_finite_geometry(&ctx, &output);
                assert_palette_is_painted(&output, palette);
                for label in [
                    APP_NAME,
                    "1. Bike setup",
                    "2. Gear results",
                    "Speed units:",
                    "km/h",
                    "mph",
                    "Inspect a gear",
                    "Speed at 90 rpm",
                ] {
                    let rect = text_rect(&output, label);
                    assert!(
                        rect.left() >= 0.0
                            && rect.right() <= size.x
                            && rect.top() >= 0.0
                            && rect.bottom() <= size.y,
                        "{label:?} is clipped at {size:?} in {expected_tab:?} (dark mode: {dark_mode}): {rect:?}"
                    );
                }
                for tagline in [
                    "BICYCLE GEARING STUDIO",
                    "A LITTLE SCIENCE. A BETTER RIDE.",
                    "Find your rhythm.",
                    "See how your gears turn every pedal stroke into distance.",
                    "MADE FOR THE RIDE",
                ] {
                    assert!(
                        output
                            .shapes
                            .iter()
                            .all(|clipped| { shape_text_rect(&clipped.shape, tagline).is_none() }),
                        "Removed tagline {tagline:?} was rendered in {expected_tab:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn invalid_saved_preferences_are_sanitized_and_corrupt_storage_recovers() {
    let mut storage = MemoryStorage::default();
    let saved = SavedState {
        settings: Settings {
            wheel: usize::MAX,
            circumference: "2100".into(),
            cadence: 999.0,
            ..Settings::default()
        },
        baseline: Some(Settings {
            chainrings: String::new(),
            ..Settings::default()
        }),
        ..SavedState::default()
    };
    eframe::set_value(&mut storage, STORAGE_KEY, &saved);
    let mut cc = eframe::CreationContext::_new_kittest(egui::Context::default());
    cc.storage = Some(&storage);
    let app = GearApp::new(&cc);
    assert_eq!(app.saved.settings.wheel, WheelPreset::ALL.len());
    assert_eq!(app.saved.settings.cadence, 150.0);
    assert_eq!(app.rows.len(), 22);
    assert!(app.saved.baseline.is_none());

    storage
        .0
        .insert(STORAGE_KEY.into(), "not valid saved state".into());
    let mut cc = eframe::CreationContext::_new_kittest(egui::Context::default());
    cc.storage = Some(&storage);
    let app = GearApp::new(&cc);
    assert_eq!(app.rows.len(), 22);
    assert_eq!(app.saved.settings.cadence, 90.0);
    assert!(app.error.is_none());
}

#[test]
fn selection_survives_display_changes_and_reconciles_removed_gears() {
    let (ctx, mut app) = new_app();
    app.selected = Some((34, 32));
    click(&ctx, &mut app, DESKTOP, "mph");
    edit_cadence(&ctx, &mut app, DESKTOP, 100);
    assert_eq!(app.selected, Some((34, 32)));

    app.saved.settings.sprockets = "11, 13".into();
    render(&ctx, &mut app, DESKTOP, Vec::new());
    assert_ne!(app.selected, Some((34, 32)));
    assert!(
        app.rows
            .iter()
            .any(|row| app.selected == Some((row.front_teeth, row.rear_teeth)))
    );
}

#[test]
fn gear_inspector_dropdowns_change_selection_without_changing_the_drivetrain() {
    let (ctx, mut app) = new_app();
    assert_eq!(app.selected, Some((34, 32)));
    let chainrings = app.saved.settings.chainrings.clone();
    let sprockets = app.saved.settings.sprockets.clone();

    click(&ctx, &mut app, DESKTOP, "34 teeth");
    click(&ctx, &mut app, DESKTOP, "50 teeth");
    assert_eq!(app.selected, Some((50, 32)));

    click(&ctx, &mut app, DESKTOP, "32 teeth");
    let output = click(&ctx, &mut app, DESKTOP, "11 teeth");
    assert_eq!(app.selected, Some((50, 11)));
    assert_eq!(app.rows.len(), 22);
    assert_eq!(app.saved.settings.chainrings, chainrings);
    assert_eq!(app.saved.settings.sprockets, sprockets);
    assert_eq!(app.tab, ResultsTab::Gears);
    text_rect(
        &output,
        "Ratio 4.55: one pedal turn rotates the rear wheel 4.55 times.",
    );
}

#[test]
fn copy_table_button_emits_complete_current_data_to_the_clipboard() {
    let (ctx, mut app) = new_app();
    click(&ctx, &mut app, DESKTOP, "mph");
    edit_cadence(&ctx, &mut app, DESKTOP, 100);
    let output = click(&ctx, &mut app, DESKTOP, "Copy table");
    let copied = output
        .platform_output
        .commands
        .iter()
        .find_map(|command| {
            if let egui::OutputCommand::CopyText(text) = command {
                Some(text)
            } else {
                None
            }
        })
        .expect("Copy table must emit a native clipboard command");
    assert_eq!(copied.lines().count(), app.rows.len() + 1);
    assert!(
        copied
            .lines()
            .next()
            .unwrap()
            .contains("Speed at 100.0 rpm (mph)")
    );
    assert!(copied.lines().all(|line| line.split('\t').count() == 11));
    assert!(copied.lines().any(|line| line.starts_with("34\t32\t")));
    assert!(app.notice.as_ref().is_some_and(|(_, error, _)| !error));
}

#[test]
fn single_and_dense_extreme_setups_produce_valid_chart_geometry() {
    for (chainrings, sprockets, circumference) in
        [("1", "200", "100"), ("198,199,200", "1-64", "10000")]
    {
        let (ctx, mut app) = new_app();
        app.saved.settings = Settings {
            chainrings: chainrings.into(),
            sprockets: sprockets.into(),
            wheel: WheelPreset::ALL.len(),
            circumference: circumference.into(),
            cadence: 150.0,
            ..Settings::default()
        };
        click(&ctx, &mut app, DESKTOP, "Speed chart");
        assert_eq!(app.tab, ResultsTab::Chart);
        let output = render(&ctx, &mut app, DESKTOP, Vec::new());
        assert!(app.error.is_none());
        assert!(!app.rows.is_empty());
        assert_finite_geometry(&ctx, &output);
    }
}

#[test]
fn chart_points_and_table_rows_select_the_inspected_gear() {
    let (ctx, mut app) = new_app();
    let size = egui::vec2(1220.0, 1280.0);
    app.saved.settings.chainrings = "40".into();
    app.saved.settings.sprockets = "11, 22, 44".into();
    render(&ctx, &mut app, size, Vec::new());
    app.selected = Some((40, 11));
    click(&ctx, &mut app, size, "Speed chart");
    assert_eq!(app.selected, Some((40, 11)));
    let output = render(&ctx, &mut app, size, Vec::new());
    let chart_title = text_rect(&output, "Speed chart");
    let axis_label = text_rect(&output, "Rear gear size (teeth)");

    fn collect_markers(shape: &egui::Shape, markers: &mut Vec<egui::Pos2>) {
        match shape {
            egui::Shape::Circle(circle) if (3.0..=4.0).contains(&circle.radius) => {
                markers.push(circle.center);
            }
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect_markers(shape, markers);
                }
            }
            _ => {}
        }
    }
    let mut markers = Vec::new();
    for clipped in &output.shapes {
        collect_markers(&clipped.shape, &mut markers);
    }
    // Choose the rightmost painted point: the largest rear sprocket, 44T.
    let pos = markers
        .into_iter()
        .filter(|pos| pos.y > chart_title.bottom() + 60.0 && pos.y < axis_label.top())
        .max_by(|a, b| a.x.total_cmp(&b.x))
        .expect("The chart must expose gear markers");
    click_position(&ctx, &mut app, size, pos);
    assert_eq!(app.selected, Some((40, 44)));

    click(&ctx, &mut app, size, "Gear table");
    assert_eq!(app.tab, ResultsTab::Gears);
    click(&ctx, &mut app, size, "40 / 22");
    assert_eq!(app.selected, Some((40, 22)));
    click(&ctx, &mut app, size, "Technical details");
    // Let the expanding section finish exposing its clipboard button.
    for _ in 0..20 {
        render(&ctx, &mut app, size, Vec::new());
    }
    let output = click(&ctx, &mut app, size, "Copy gear");
    let copied = output
        .platform_output
        .commands
        .iter()
        .find_map(|command| match command {
            egui::OutputCommand::CopyText(text) => Some(text),
            _ => None,
        })
        .expect("The selected gear should be copyable");
    assert_eq!(copied.lines().count(), 2);
    assert!(copied.lines().nth(1).unwrap().starts_with("40\t22\t"));
}

#[test]
fn keyboard_copy_works_with_button_focus_and_preserves_text_field_copy() {
    let (ctx, mut app) = new_app();
    render(&ctx, &mut app, DESKTOP, Vec::new());
    render(
        &ctx,
        &mut app,
        DESKTOP,
        vec![key_press(egui::Key::Tab, egui::Modifiers::NONE)],
    );
    let output = render(&ctx, &mut app, DESKTOP, Vec::new());
    let focused = ctx
        .memory(|memory| memory.focused())
        .expect("Tab should focus a control");
    let response = ctx
        .read_response(focused)
        .expect("The focused control is still rendered");
    let help_button = text_rect(&output, "Help");
    assert!(response.rect.contains(help_button.center()));
    assert!(!ctx.text_edit_focused());

    // Native integrations translate Ctrl/Cmd+C into Event::Copy.
    let output = render(&ctx, &mut app, DESKTOP, vec![egui::Event::Copy]);
    let copies: Vec<_> = output
        .platform_output
        .commands
        .iter()
        .filter_map(|command| match command {
            egui::OutputCommand::CopyText(text) => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(copies.len(), 1);
    assert_eq!(copies[0].lines().count(), app.rows.len() + 1);

    click(&ctx, &mut app, DESKTOP, "50, 34");
    assert!(ctx.text_edit_focused());
    let output = render(
        &ctx,
        &mut app,
        DESKTOP,
        vec![
            key_press(egui::Key::A, egui::Modifiers::COMMAND),
            egui::Event::Copy,
        ],
    );
    let copies: Vec<_> = output
        .platform_output
        .commands
        .iter()
        .filter_map(|command| match command {
            egui::OutputCommand::CopyText(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        copies,
        ["50, 34"],
        "The app shortcut must not replace the text editor's clipboard content"
    );
}

#[test]
fn every_builtin_cassette_is_valid_including_the_legacy_full_range() {
    let expected = [
        (11, 11, 32),
        (11, 11, 34),
        (11, 11, 42),
        (12, 10, 51),
        (52, 9, 60),
    ];
    assert_eq!(CASSETTES.len(), expected.len());
    for ((name, teeth), (count, first, last)) in CASSETTES.iter().zip(expected) {
        let setup = GearSetup::parse("50, 34", teeth, 2130.0)
            .unwrap_or_else(|error| panic!("The {name} preset is invalid: {error}"));
        assert_eq!(setup.sprockets.len(), count, "{name}");
        assert_eq!(setup.sprockets.first(), Some(&first), "{name}");
        assert_eq!(setup.sprockets.last(), Some(&last), "{name}");
    }
}
