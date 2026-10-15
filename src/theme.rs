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
    // An original palette inspired by the balance of paper, ink, and earthy
    // accents in Sanzo Wada's color studies, adapted for screen contrast.
    if dark {
        Palette {
            background: Color32::from_rgb(28, 45, 52),
            surface: Color32::from_rgb(39, 59, 66),
            text: Color32::from_rgb(244, 235, 221),
            muted: Color32::from_rgb(180, 191, 185),
            accent: Color32::from_rgb(168, 208, 194),
            selection: Color32::from_rgb(51, 77, 82),
            border: Color32::from_rgb(74, 95, 99),
            error: Color32::from_rgb(243, 160, 135),
            error_background: Color32::from_rgb(80, 59, 57),
            accent_text: Color32::from_rgb(24, 52, 59),
            chart_colors: [
                Color32::from_rgb(168, 208, 194),
                Color32::from_rgb(239, 171, 130),
                Color32::from_rgb(218, 192, 130),
            ],
        }
    } else {
        Palette {
            background: Color32::from_rgb(244, 239, 228),
            surface: Color32::from_rgb(255, 252, 244),
            text: Color32::from_rgb(36, 61, 68),
            muted: Color32::from_rgb(85, 103, 106),
            accent: Color32::from_rgb(32, 92, 99),
            selection: Color32::from_rgb(224, 233, 223),
            border: Color32::from_rgb(215, 210, 198),
            error: Color32::from_rgb(170, 68, 51),
            error_background: Color32::from_rgb(250, 232, 221),
            accent_text: Color32::from_rgb(255, 252, 244),
            chart_colors: [
                Color32::from_rgb(32, 92, 99),
                Color32::from_rgb(169, 70, 47),
                Color32::from_rgb(135, 96, 30),
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
            (TextStyle::Heading, FontId::proportional(23.0)),
            (TextStyle::Body, FontId::proportional(15.0)),
            (TextStyle::Button, FontId::proportional(15.0)),
            (TextStyle::Small, FontId::proportional(13.0)),
            (TextStyle::Monospace, FontId::monospace(13.0)),
        ]
        .into(),
        ..Default::default()
    };
    style.spacing.item_spacing = Vec2::new(9.0, 7.0);
    style.spacing.button_padding = Vec2::new(12.0, 7.0);
    style.spacing.interact_size.y = 32.0;
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
    visuals.window_corner_radius = CornerRadius::same(10);
    visuals.menu_corner_radius = CornerRadius::same(8);
    visuals.extreme_bg_color = colors.background;
    visuals.text_edit_bg_color = Some(colors.background);
    visuals.faint_bg_color = colors.background;
    visuals.code_bg_color = colors.background;
    visuals.warn_fg_color = colors.chart_colors[2];
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
        widget.corner_radius = CornerRadius::same(7);
        widget.expansion = 0.0;
    }
    visuals.widgets.hovered.bg_stroke.width = 1.5;
    visuals.widgets.active.bg_stroke.width = 1.5;
    visuals.widgets.open.bg_stroke.width = 1.5;
    style
}

pub fn card(ui: &egui::Ui) -> egui::Frame {
    let colors = palette(ui);
    egui::Frame::new()
        .fill(colors.surface)
        .stroke(Stroke::new(1.0, colors.border))
        .corner_radius(10)
        .inner_margin(16)
}

pub fn muted(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.label(
        egui::RichText::new(text.into())
            .size(13.0)
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
            7,
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
    p.rect_filled(rect, 8, colors.accent);
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
    let colors = palette_for(false);
    let background = colors.accent.to_array();
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
                        colors.accent_text.to_array()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(color: Color32) -> f64 {
        let linear = [color.r(), color.g(), color.b()].map(|channel| {
            let value = f64::from(channel) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        });
        linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722
    }

    fn assert_readable(foreground: Color32, background: Color32) {
        let foreground_luminance = luminance(foreground);
        let background_luminance = luminance(background);
        let ratio = (foreground_luminance.max(background_luminance) + 0.05)
            / (foreground_luminance.min(background_luminance) + 0.05);
        assert!(
            ratio >= 4.5,
            "Text {foreground:?} on {background:?} has only {ratio:.2}:1 contrast"
        );
    }

    #[test]
    fn both_palettes_keep_labels_readable_on_normal_and_selected_surfaces() {
        for dark in [false, true] {
            let colors = palette_for(dark);
            for background in [colors.background, colors.surface, colors.selection] {
                for foreground in [colors.text, colors.muted, colors.accent]
                    .into_iter()
                    .chain(colors.chart_colors)
                {
                    assert_readable(foreground, background);
                }
            }
            assert_readable(colors.accent_text, colors.accent);
            assert_readable(colors.error, colors.error_background);
        }
    }
}
