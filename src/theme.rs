use eframe::egui::{self, Color32, CornerRadius, FontId, Stroke, TextStyle, Vec2};

/// The theme button has this stable global ID, including when its icon changes.
/// Tests can find it with `ctx.read_response(egui::Id::new(THEME_TOGGLE_ID))`.
pub const THEME_TOGGLE_ID: &str = "cogcalc.theme_toggle";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub background: Color32,
    pub surface: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub selection: Color32,
    pub border: Color32,
    pub error: Color32,
    pub error_background: Color32,
    pub accent_text: Color32,
    pub chart_colors: [Color32; 3],
}

pub fn palette(ui: &egui::Ui) -> Palette {
    palette_for(ui.visuals().dark_mode)
}

fn palette_for(dark: bool) -> Palette {
    if dark {
        Palette {
            background: Color32::from_rgb(24, 30, 35),
            surface: Color32::from_rgb(35, 43, 49),
            text: Color32::from_rgb(231, 236, 239),
            muted: Color32::from_rgb(165, 177, 183),
            accent: Color32::from_rgb(105, 203, 177),
            selection: Color32::from_rgb(32, 62, 55),
            border: Color32::from_rgb(57, 68, 75),
            error: Color32::from_rgb(255, 170, 160),
            error_background: Color32::from_rgb(72, 45, 44),
            accent_text: Color32::from_rgb(16, 36, 30),
            chart_colors: [
                Color32::from_rgb(105, 203, 177),
                Color32::from_rgb(238, 183, 121),
                Color32::from_rgb(137, 180, 239),
            ],
        }
    } else {
        Palette {
            background: Color32::from_rgb(243, 245, 246),
            surface: Color32::WHITE,
            text: Color32::from_rgb(30, 43, 50),
            muted: Color32::from_rgb(91, 107, 116),
            accent: Color32::from_rgb(18, 111, 92),
            selection: Color32::from_rgb(225, 240, 235),
            border: Color32::from_rgb(212, 220, 224),
            error: Color32::from_rgb(178, 63, 51),
            error_background: Color32::from_rgb(255, 240, 237),
            accent_text: Color32::WHITE,
            chart_colors: [
                Color32::from_rgb(18, 111, 92),
                Color32::from_rgb(169, 96, 43),
                Color32::from_rgb(66, 111, 165),
            ],
        }
    }
}

pub fn install(ctx: &egui::Context, dark: bool) {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        ctx.set_style_of(theme, style_for(theme == egui::Theme::Dark));
    }
    ctx.set_theme(if dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });
}

fn style_for(dark: bool) -> egui::Style {
    let colors = palette_for(dark);
    let mut style = egui::Style {
        text_styles: [
            (TextStyle::Heading, FontId::proportional(21.0)),
            (TextStyle::Body, FontId::proportional(14.0)),
            (TextStyle::Button, FontId::proportional(14.0)),
            (TextStyle::Small, FontId::proportional(12.0)),
            (TextStyle::Monospace, FontId::monospace(13.0)),
        ]
        .into(),
        ..Default::default()
    };
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    style.spacing.button_padding = Vec2::new(10.0, 6.0);
    style.spacing.interact_size.y = 30.0;
    style.spacing.slider_width = 164.0;
    style.visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    let visuals = &mut style.visuals;
    visuals.override_text_color = Some(colors.text);
    visuals.weak_text_color = Some(colors.muted);
    visuals.hyperlink_color = colors.accent;
    visuals.panel_fill = colors.background;
    visuals.window_fill = colors.surface;
    visuals.window_stroke = Stroke::new(1.0, colors.border);
    visuals.window_corner_radius = CornerRadius::same(6);
    visuals.menu_corner_radius = CornerRadius::same(6);
    visuals.extreme_bg_color = colors.background;
    visuals.text_edit_bg_color = Some(colors.background);
    visuals.faint_bg_color = colors.background;
    visuals.code_bg_color = colors.background;
    visuals.warn_fg_color = colors.chart_colors[1];
    visuals.error_fg_color = colors.error;
    visuals.selection.bg_fill = colors.selection;
    visuals.selection.stroke = Stroke::new(1.0, colors.accent);
    visuals.text_cursor.stroke = Stroke::new(1.5, colors.accent);
    visuals.ime_composition.active_underline_stroke = Stroke::new(1.5, colors.accent);
    visuals.ime_composition.inactive_underline_stroke = Stroke::new(1.0, colors.muted);
    visuals.slider_trailing_fill = true;

    for (widget, fill, border) in [
        (
            &mut visuals.widgets.noninteractive,
            colors.surface,
            colors.border,
        ),
        (&mut visuals.widgets.inactive, colors.surface, colors.border),
        (
            &mut visuals.widgets.hovered,
            colors.selection,
            colors.accent,
        ),
        (&mut visuals.widgets.active, colors.selection, colors.accent),
        (&mut visuals.widgets.open, colors.selection, colors.accent),
    ] {
        widget.bg_fill = fill;
        widget.weak_bg_fill = fill;
        widget.bg_stroke = Stroke::new(1.0, border);
        widget.fg_stroke = Stroke::new(1.0, colors.text);
        widget.corner_radius = CornerRadius::same(5);
        widget.expansion = 0.0;
    }
    style
}

pub fn card(ui: &egui::Ui) -> egui::Frame {
    let colors = palette(ui);
    egui::Frame::new()
        .fill(colors.surface)
        .stroke(Stroke::new(1.0, colors.border))
        .corner_radius(6)
        .inner_margin(14)
}

pub fn muted(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.label(
        egui::RichText::new(text.into())
            .size(12.0)
            .color(palette(ui).muted),
    );
}

/// Switches the preference on mouse click, Space, Enter, or an accessibility action.
/// The caller should call [`install`] with the new value when the response changed.
pub fn theme_toggle(ui: &mut egui::Ui, dark: &mut bool) -> egui::Response {
    let (_, rect) = ui.allocate_space(Vec2::splat(32.0));
    let mut response = ui.interact(rect, egui::Id::new(THEME_TOGGLE_ID), egui::Sense::click());
    if response.clicked() {
        *dark = !*dark;
        response.mark_changed();
    }
    let action = if *dark {
        "Switch to light mode"
    } else {
        "Switch to dark mode"
    };
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), action)
    });

    if ui.is_rect_visible(rect) {
        let visuals = *ui.style().interact(&response);
        let painter = ui.painter();
        painter.rect(
            rect,
            5,
            visuals.weak_bg_fill,
            visuals.bg_stroke,
            egui::StrokeKind::Inside,
        );
        let center = rect.center();
        let stroke = Stroke::new(1.7, visuals.fg_stroke.color);
        if *dark {
            painter.circle_stroke(center, 4.0, stroke);
            for ray in 0..8 {
                let angle = ray as f32 * std::f32::consts::TAU / 8.0;
                let direction = egui::vec2(angle.cos(), angle.sin());
                painter.line_segment(
                    [center + direction * 7.0, center + direction * 10.0],
                    stroke,
                );
            }
        } else {
            // The offset cutout makes a crescent without depending on a symbol font.
            painter.circle_filled(center + egui::vec2(-1.0, 1.0), 8.5, visuals.fg_stroke.color);
            painter.circle_filled(center + egui::vec2(3.0, -2.0), 7.5, visuals.weak_bg_fill);
        }
    }
    response
        .on_hover_text(action)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

const COG_TEETH: usize = 10;

/// The same proportions define both the vector UI mark and native window icon.
fn cog_tooth(center: egui::Pos2, radius: f32, index: usize) -> [egui::Pos2; 4] {
    let angle =
        index as f32 * std::f32::consts::TAU / COG_TEETH as f32 - std::f32::consts::FRAC_PI_2;
    let radial = egui::vec2(angle.cos(), angle.sin());
    let tangent = egui::vec2(-radial.y, radial.x);
    [(0.68, -0.18), (1.0, -0.115), (1.0, 0.115), (0.68, 0.18)]
        .map(|(out, across)| center + radius * (radial * out + tangent * across))
}

pub fn brand(ui: &mut egui::Ui) {
    let colors = palette(ui);
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(32.0), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 6, colors.accent);
    let radius = 11.5;
    for tooth in 0..COG_TEETH {
        p.add(egui::Shape::convex_polygon(
            cog_tooth(rect.center(), radius, tooth).to_vec(),
            colors.accent_text,
            Stroke::NONE,
        ));
    }
    p.circle_filled(rect.center(), radius * 0.78, colors.accent_text);
    p.circle_filled(rect.center(), radius * 0.38, colors.accent);
}

pub fn window_icon() -> egui::IconData {
    let size = 64;
    let samples = 4;
    let center = egui::pos2(32.0, 32.0);
    let radius: f32 = 23.0;
    let teeth = std::array::from_fn::<_, COG_TEETH, _>(|index| cog_tooth(center, radius, index));
    let background = Color32::from_rgb(18, 111, 92).to_array();
    let mut rgba = Vec::with_capacity(size * size * 4);
    for y in 0..size {
        for x in 0..size {
            let mut opaque_samples = 0_u32;
            let mut channels = [0_u32; 3];
            for sy in 0..samples {
                for sx in 0..samples {
                    let point = egui::pos2(
                        x as f32 + (sx as f32 + 0.5) / samples as f32,
                        y as f32 + (sy as f32 + 0.5) / samples as f32,
                    );
                    let corner_center =
                        egui::pos2(point.x.clamp(12.0, 52.0), point.y.clamp(12.0, 52.0));
                    if point.distance_sq(corner_center) > 12.0_f32.powi(2) {
                        continue;
                    }
                    let distance_sq = point.distance_sq(center);
                    let on_cog = distance_sq >= (radius * 0.38).powi(2)
                        && (distance_sq <= (radius * 0.78).powi(2)
                            || teeth.iter().any(|tooth| point_in_tooth(point, tooth)));
                    let color = if on_cog {
                        [255, 255, 255, 255]
                    } else {
                        background
                    };
                    for channel in 0..3 {
                        channels[channel] += u32::from(color[channel]);
                    }
                    opaque_samples += 1;
                }
            }
            // IconData uses straight RGBA, so keep colors independent of corner alpha.
            if opaque_samples == 0 {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                rgba.extend(channels.map(|channel| (channel / opaque_samples) as u8));
                rgba.push((255 * opaque_samples / (samples * samples)) as u8);
            }
        }
    }
    egui::IconData {
        rgba,
        width: size as u32,
        height: size as u32,
    }
}

fn point_in_tooth(point: egui::Pos2, tooth: &[egui::Pos2; 4]) -> bool {
    (0..4).all(|index| {
        let edge = tooth[(index + 1) % 4] - tooth[index];
        let relative = point - tooth[index];
        edge.x * relative.y - edge.y * relative.x >= 0.0
    })
}
