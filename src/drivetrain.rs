//! A generated SVG drivetrain schematic. Each outline has the requested number
//! of teeth; dimensions are normalized for legibility, not manufacturing.

use std::f64::consts::{FRAC_PI_2, TAU};
use std::fmt::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use cogcalc::GearSetup;
use eframe::egui::{self, Color32, Pos2, Rect, Sense};

use crate::theme::{self, Palette};

static NEXT_VIEW: AtomicU64 = AtomicU64::new(0);

/// Owns only the current SVG and its hit targets. The stable image URI is
/// invalidated whenever the setup, selected gear, layout, or palette changes.
pub struct DrivetrainView {
    uri: String,
    cached: Option<CachedSvg>,
}

impl Default for DrivetrainView {
    fn default() -> Self {
        Self {
            uri: format!(
                "bytes://cogcalc-drivetrain-{}.svg",
                NEXT_VIEW.fetch_add(1, Ordering::Relaxed)
            ),
            cached: None,
        }
    }
}

#[derive(Clone, PartialEq)]
struct ViewKey {
    fronts: Vec<u16>,
    rears: Vec<u16>,
    selected: (u16, u16),
    width: u32,
    palette: Palette,
}

struct CachedSvg {
    key: ViewKey,
    drawing: SvgDrawing,
    bytes: Arc<[u8]>,
}

struct SvgDrawing {
    svg: String,
    width: f32,
    height: f32,
    targets: Vec<GearTarget>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GearKind {
    Front,
    Rear,
}

impl GearKind {
    fn id(self) -> &'static str {
        match self {
            Self::Front => "front",
            Self::Rear => "rear",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Front => "Front chainring",
            Self::Rear => "Rear sprocket",
        }
    }
}

#[derive(Clone, Copy)]
struct GearTarget {
    rect: Rect,
    kind: GearKind,
    teeth: u16,
}

impl DrivetrainView {
    /// The current complete SVG, suitable for saving as a standalone file.
    /// Available after the first call to `show` with a valid setup.
    pub fn svg(&self) -> Option<&str> {
        self.cached
            .as_ref()
            .map(|cached| cached.drawing.svg.as_str())
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        setup: &GearSetup,
        selected: &mut Option<(u16, u16)>,
    ) {
        // Validate public domain fields as callers can construct GearSetup directly.
        let Ok(setup) = GearSetup::new(
            setup.chainrings.clone(),
            setup.sprockets.clone(),
            setup.circumference_mm,
        ) else {
            if self.cached.take().is_some() {
                ui.ctx().forget_image(&self.uri);
            }
            ui.label("Enter a valid bike setup to display its gears.");
            return;
        };

        let pair = selected
            .filter(|(front, rear)| {
                setup.chainrings.contains(front) && setup.sprockets.contains(rear)
            })
            .unwrap_or_else(|| {
                // Validated, normalized inputs always contain at least one gear.
                (
                    *setup.chainrings.last().unwrap(),
                    *setup.sprockets.last().unwrap(),
                )
            });
        *selected = Some(pair);

        let display_width = ui.available_width().max(1.0);
        let width = display_width.floor().clamp(280.0, 1_600.0) as u32;
        let key = ViewKey {
            fronts: setup.chainrings,
            rears: setup.sprockets,
            selected: pair,
            width,
            palette: theme::palette(ui),
        };
        self.prepare(ui.ctx(), key);
        egui_extras::install_image_loaders(ui.ctx());

        let cached = self.cached.as_ref().unwrap();
        let scale = display_width / cached.drawing.width;
        let size = egui::vec2(display_width, cached.drawing.height * scale);
        let response = ui.add(
            egui::Image::from_bytes(self.uri.clone(), cached.bytes.clone())
                .fit_to_exact_size(size)
                .sense(Sense::click())
                .alt_text(format!(
                    "Bicycle drivetrain with {} front chainrings and {} rear sprockets. Selected: {} front teeth and {} rear teeth.",
                    cached.key.fronts.len(), cached.key.rears.len(), pair.0, pair.1
                )),
        );

        let target = response.hover_pos().and_then(|pointer| {
            let local = Pos2::new(
                (pointer.x - response.rect.left()) / scale,
                (pointer.y - response.rect.top()) / scale,
            );
            cached
                .drawing
                .targets
                .iter()
                .find(|target| target.rect.contains(local))
                .copied()
        });
        if let Some(target) = target {
            let clicked = response.clicked();
            response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(format!(
                    "{}: {} teeth. Click to inspect this gear.",
                    target.kind.label(),
                    target.teeth
                ));
            if clicked {
                let pair = match target.kind {
                    GearKind::Front => (target.teeth, pair.1),
                    GearKind::Rear => (pair.0, target.teeth),
                };
                *selected = Some(pair);
                // Keep exported SVG in sync immediately, even before repaint.
                let mut key = self.cached.as_ref().unwrap().key.clone();
                key.selected = pair;
                self.prepare(ui.ctx(), key);
                ui.ctx().request_repaint();
            }
        }
    }

    fn prepare(&mut self, ctx: &egui::Context, key: ViewKey) {
        if self.cached.as_ref().is_some_and(|cached| cached.key == key) {
            return;
        }
        ctx.forget_image(&self.uri);
        let drawing = build_svg(&key);
        let bytes = Arc::from(drawing.svg.as_bytes());
        self.cached = Some(CachedSvg {
            key,
            drawing,
            bytes,
        });
    }
}

struct SvgWriter {
    body: String,
    targets: Vec<GearTarget>,
    palette: Palette,
}

impl SvgWriter {
    fn text(&mut self, x: f32, y: f32, size: u16, text: &str, center: bool, color: Color32) {
        let anchor = if center { "middle" } else { "start" };
        let _ = write!(
            self.body,
            "<text x=\"{x:.2}\" y=\"{y:.2}\" font-size=\"{size}\" text-anchor=\"{anchor}\" fill=\"{}\">{}</text>",
            hex(color),
            escape_xml(text)
        );
    }

    fn cog(&mut self, center: Pos2, radius: f32, teeth: u16, selected: bool) {
        let fill = if selected {
            self.palette.selection
        } else {
            self.palette.background
        };
        let stroke = if selected {
            self.palette.accent
        } else {
            self.palette.muted
        };
        let _ = write!(
            self.body,
            "<path class=\"cog-outline\" data-tooth-count=\"{teeth}\" d=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1.1\" stroke-linejoin=\"round\"/>",
            cog_path(center, radius, teeth),
            hex(fill),
            hex(stroke)
        );
        // A hub opening and five lightening holes make the cog's structure clear.
        // These are schematic, not bolt-circle or component specifications.
        for hole in 0..5 {
            let angle = f64::from(hole) * TAU / 5.0 - FRAC_PI_2;
            let x = center.x + angle.cos() as f32 * radius * 0.49;
            let y = center.y + angle.sin() as f32 * radius * 0.49;
            let _ = write!(
                self.body,
                "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"{:.2}\" fill=\"{}\"/>",
                radius * 0.105,
                hex(self.palette.surface)
            );
        }
        let _ = write!(
            self.body,
            "<circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"{:.2}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1.1\"/>",
            center.x,
            center.y,
            radius * 0.2,
            hex(self.palette.surface),
            hex(stroke)
        );
    }

    fn inventory(
        &mut self,
        origin: Pos2,
        width: f32,
        teeth: &[u16],
        kind: GearKind,
        selected: u16,
    ) -> f32 {
        let title = match kind {
            GearKind::Front => format!("Front chainrings ({})", teeth.len()),
            GearKind::Rear => format!("Rear sprockets ({})", teeth.len()),
        };
        let top = origin.y;
        self.text(
            origin.x + 16.0,
            top + 20.0,
            16,
            &title,
            false,
            self.palette.text,
        );
        let columns = (((width - 24.0) / 96.0) as usize)
            .clamp(1, 8)
            .min(teeth.len());
        let gap = 8.0;
        let cell_width = ((width - 32.0 - gap * (columns - 1) as f32) / columns as f32).min(160.0);
        let cell_height = 108.0;
        let start_x =
            origin.x + (width - (cell_width * columns as f32 + gap * (columns - 1) as f32)) / 2.0;
        let max_teeth = f32::from(*teeth.iter().max().unwrap());
        for (index, count) in teeth.iter().copied().enumerate() {
            let x = start_x + (index % columns) as f32 * (cell_width + gap);
            let y = top + 34.0 + (index / columns) as f32 * (cell_height + gap);
            let is_selected = count == selected;
            let border = if is_selected {
                self.palette.accent
            } else {
                self.palette.border
            };
            let line_width = if is_selected { 2.0 } else { 1.0 };
            let _ = write!(
                self.body,
                "<g id=\"{}-{index}\" data-role=\"inventory\" data-kind=\"{}\" data-teeth=\"{count}\" data-selected=\"{is_selected}\"><title>{}: {count} teeth</title><rect x=\"{x:.2}\" y=\"{y:.2}\" width=\"{cell_width:.2}\" height=\"{cell_height:.2}\" rx=\"5\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{line_width}\"/>",
                kind.id(),
                kind.id(),
                kind.label(),
                hex(self.palette.surface),
                hex(border)
            );
            let radius = 19.0 + 17.0 * (f32::from(count) / max_teeth).sqrt();
            let center = Pos2::new(x + cell_width / 2.0, y + 43.0);
            self.cog(center, radius, count, is_selected);
            self.text(
                center.x,
                y + 96.0,
                14,
                &format!("{count} teeth"),
                true,
                self.palette.text,
            );
            self.body.push_str("</g>");
            self.targets.push(GearTarget {
                rect: Rect::from_min_size(Pos2::new(x, y), egui::vec2(cell_width, cell_height)),
                kind,
                teeth: count,
            });
        }
        top + 34.0 + teeth.len().div_ceil(columns) as f32 * (cell_height + gap)
    }
}

fn build_svg(key: &ViewKey) -> SvgDrawing {
    let width = key.width as f32;
    let mut writer = SvgWriter {
        body: String::new(),
        targets: Vec::new(),
        palette: key.palette,
    };
    let (front, rear) = key.selected;
    writer.text(
        width / 2.0,
        27.0,
        18,
        &format!("Selected gear: {front} / {rear} teeth"),
        true,
        key.palette.text,
    );
    writer.text(
        width / 2.0,
        48.0,
        13,
        &format!(
            "Ratio {:.2} · front teeth ÷ rear teeth",
            f64::from(front) / f64::from(rear)
        ),
        true,
        key.palette.muted,
    );

    let largest = f32::from(front.max(rear));
    let front_radius = 26.0 + 30.0 * (f32::from(front) / largest);
    let rear_radius = 26.0 + 30.0 * (f32::from(rear) / largest);
    let front_center = Pos2::new(width * 0.75, 113.0);
    let rear_center = Pos2::new(width * 0.25, 113.0);
    let chain = chain_path(
        rear_center,
        rear_radius + 3.0,
        front_center,
        front_radius + 3.0,
    );
    let _ = write!(
        writer.body,
        "<path data-role=\"chain\" d=\"{chain}\" fill=\"none\" stroke=\"{}\" stroke-width=\"5\"/><path d=\"{chain}\" fill=\"none\" stroke=\"{}\" stroke-width=\"1.8\" stroke-dasharray=\"4 4\"/>",
        hex(key.palette.muted),
        hex(key.palette.surface)
    );
    writer.body.push_str("<g data-role=\"selected-front\">");
    writer.cog(front_center, front_radius, front, true);
    writer.body.push_str("</g><g data-role=\"selected-rear\">");
    writer.cog(rear_center, rear_radius, rear, true);
    writer.body.push_str("</g>");
    writer.text(
        front_center.x,
        188.0,
        14,
        &format!("Front · {front} teeth"),
        true,
        key.palette.text,
    );
    writer.text(
        rear_center.x,
        188.0,
        14,
        &format!("Rear · {rear} teeth"),
        true,
        key.palette.text,
    );
    writer.text(
        width / 2.0,
        212.0,
        12,
        "Schematic: exact tooth counts, sizes normalized for visibility.",
        true,
        key.palette.muted,
    );

    let inventory_top = 224.0;
    let height = if width >= 700.0 {
        let front_width = 230.0;
        let front_x = width - front_width;
        let front_bottom = writer.inventory(
            Pos2::new(front_x, inventory_top),
            front_width,
            &key.fronts,
            GearKind::Front,
            front,
        );
        let rear_bottom = writer.inventory(
            Pos2::new(0.0, inventory_top),
            front_x - 12.0,
            &key.rears,
            GearKind::Rear,
            rear,
        );
        front_bottom.max(rear_bottom) + 12.0
    } else {
        let next_y = writer.inventory(
            Pos2::new(0.0, inventory_top),
            width,
            &key.fronts,
            GearKind::Front,
            front,
        );
        writer.inventory(
            Pos2::new(0.0, next_y + 16.0),
            width,
            &key.rears,
            GearKind::Rear,
            rear,
        ) + 12.0
    };
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width:.0}\" height=\"{height:.0}\" viewBox=\"0 0 {width:.0} {height:.0}\" role=\"img\" aria-labelledby=\"title description\"><title id=\"title\">Bicycle drivetrain: {} front chainrings and {} rear sprockets</title><desc id=\"description\">Selected front {front} teeth and rear {rear} teeth. All gear outlines contain their specified number of teeth. Gear sizes and chain routing are schematic, not manufacturing geometry.</desc><rect width=\"100%\" height=\"100%\" fill=\"{}\"/><g font-family=\"sans-serif\">{}</g></svg>",
        key.fronts.len(),
        key.rears.len(),
        hex(key.palette.surface),
        writer.body
    );
    SvgDrawing {
        svg,
        width,
        height,
        targets: writer.targets,
    }
}

/// One continuous closed outline: two root-circle arcs and three straight
/// segments per tooth. Arcs preserve a circular body even for a one-tooth input.
fn cog_path(center: Pos2, radius: f32, teeth: u16) -> String {
    if teeth == 0 || !radius.is_finite() || radius <= 0.0 {
        return String::new();
    }
    let radius = f64::from(radius);
    let pitch = TAU / f64::from(teeth);
    let depth = (radius * 0.18).min(radius * pitch * 0.8);
    let root = radius - depth;
    let base_half = (pitch * 0.32).min(0.32);
    let tip_half = (pitch * 0.18).min(0.18);
    let point = |angle: f64, distance: f64| -> (f64, f64) {
        (
            f64::from(center.x) + angle.cos() * distance,
            f64::from(center.y) + angle.sin() * distance,
        )
    };
    let (x, y) = point(-FRAC_PI_2 - pitch / 2.0, root);
    let mut path = format!("M{x:.3},{y:.3}");
    for tooth in 0..teeth {
        let angle = -FRAC_PI_2 + f64::from(tooth) * pitch;
        let (x, y) = point(angle - base_half, root);
        let _ = write!(path, " A{root:.3},{root:.3} 0 0 1 {x:.3},{y:.3}");
        for (angle, distance) in [
            (angle - tip_half, radius),
            (angle + tip_half, radius),
            (angle + base_half, root),
        ] {
            let (x, y) = point(angle, distance);
            let _ = write!(path, " L{x:.3},{y:.3}");
        }
        let (x, y) = point(angle + pitch / 2.0, root);
        let _ = write!(path, " A{root:.3},{root:.3} 0 0 1 {x:.3},{y:.3}");
    }
    path.push_str(" Z");
    path
}

// Trace clockwise around two horizontally aligned gears, supplied left to right.
fn chain_path(left: Pos2, left_radius: f32, right: Pos2, right_radius: f32) -> String {
    let dx = right.x - left.x;
    let normal_x = ((left_radius - right_radius) / dx).clamp(-0.99, 0.99);
    let normal_y = (1.0 - normal_x * normal_x).sqrt();
    let a = Pos2::new(
        left.x + left_radius * normal_x,
        left.y - left_radius * normal_y,
    );
    let b = Pos2::new(
        right.x + right_radius * normal_x,
        right.y - right_radius * normal_y,
    );
    let c = Pos2::new(
        right.x + right_radius * normal_x,
        right.y + right_radius * normal_y,
    );
    let d = Pos2::new(
        left.x + left_radius * normal_x,
        left.y + left_radius * normal_y,
    );
    let right_large = u8::from(right_radius > left_radius);
    let left_large = u8::from(left_radius >= right_radius);
    format!(
        "M{:.3},{:.3} L{:.3},{:.3} A{right_radius:.3},{right_radius:.3} 0 {right_large} 1 {:.3},{:.3} L{:.3},{:.3} A{left_radius:.3},{left_radius:.3} 0 {left_large} 1 {:.3},{:.3} Z",
        a.x, a.y, b.x, b.y, c.x, c.y, d.x, d.y, a.x, a.y
    )
}

fn hex(color: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b())
}

fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette() -> Palette {
        Palette {
            background: Color32::from_gray(240),
            surface: Color32::WHITE,
            text: Color32::BLACK,
            muted: Color32::GRAY,
            accent: Color32::DARK_GREEN,
            selection: Color32::LIGHT_GREEN,
            border: Color32::GRAY,
            error: Color32::RED,
            error_background: Color32::LIGHT_RED,
            accent_text: Color32::WHITE,
            chart_colors: [Color32::BLACK; 3],
        }
    }

    fn key(fronts: &str, rears: &str, width: u32) -> ViewKey {
        let setup = GearSetup::parse(fronts, rears, 2_100.0).unwrap();
        ViewKey {
            selected: (setup.chainrings[0], setup.sprockets[0]),
            fronts: setup.chainrings,
            rears: setup.sprockets,
            width,
            palette: palette(),
        }
    }

    #[test]
    fn every_cog_outline_has_exactly_the_requested_number_of_teeth() {
        for teeth in [1, 2, 3, 9, 11, 34, 60, 199, 200] {
            let path = cog_path(Pos2::new(50.0, 50.0), 36.0, teeth);
            assert_eq!(path.matches('M').count(), 1);
            assert_eq!(path.matches('L').count(), usize::from(teeth) * 3);
            assert_eq!(path.matches('A').count(), usize::from(teeth) * 2);
            assert!(path.ends_with(" Z"));
            assert!(!path.contains("NaN") && !path.contains("inf"));
        }
        assert!(cog_path(Pos2::ZERO, 36.0, 0).is_empty());
    }

    #[test]
    fn svg_contains_all_actual_gears_and_a_separate_selected_pair() {
        let drawing = build_svg(&key("50,34", "11,13,17,28,34", 800));
        assert_eq!(drawing.svg.matches("data-role=\"inventory\"").count(), 7);
        assert_eq!(drawing.svg.matches("data-kind=\"front\"").count(), 2);
        assert_eq!(drawing.svg.matches("data-kind=\"rear\"").count(), 5);
        assert_eq!(drawing.svg.matches("data-selected=\"true\"").count(), 2);
        assert_eq!(drawing.svg.matches("class=\"cog-outline\"").count(), 9);
        assert!(
            drawing
                .svg
                .contains("Selected front 50 teeth and rear 11 teeth")
        );
        assert_eq!(drawing.targets.len(), 7);
    }

    #[test]
    fn maximum_setup_fits_every_hit_target_without_overlap_at_small_width() {
        let drawing = build_svg(&key("200,100,1", "137-200", 440));
        assert_eq!(drawing.targets.len(), 67);
        let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(drawing.width, drawing.height));
        for (index, target) in drawing.targets.iter().enumerate() {
            assert!(bounds.contains_rect(target.rect));
            assert!(target.rect.width() >= 70.0);
            for other in &drawing.targets[index + 1..] {
                assert!(!target.rect.intersects(other.rect));
            }
        }
        assert!(!drawing.svg.contains("NaN") && !drawing.svg.contains("inf"));
    }

    #[test]
    fn narrow_layout_reflows_and_single_gears_remain_valid() {
        let narrow = build_svg(&key("40", "9-60", 440));
        let wide = build_svg(&key("40", "9-60", 840));
        assert!(narrow.height > wide.height);
        let single = build_svg(&key("1", "200", 280));
        assert_eq!(single.targets.len(), 2);
        assert!(single.svg.contains("viewBox=\"0 0 280 "));
        assert!(single.svg.ends_with("</svg>"));
    }

    #[test]
    fn cache_reuses_current_image_and_invalidates_on_selection_or_theme_change() {
        let ctx = egui::Context::default();
        let mut view = DrivetrainView::default();
        assert!(view.svg().is_none());
        let original = key("50,34", "11,34", 800);
        view.prepare(&ctx, original.clone());
        let previous = view.cached.as_ref().unwrap().bytes.clone();
        view.prepare(&ctx, original.clone());
        assert!(Arc::ptr_eq(&previous, &view.cached.as_ref().unwrap().bytes));
        let mut changed = original;
        changed.selected = (34, 34);
        view.prepare(&ctx, changed.clone());
        assert!(!Arc::ptr_eq(
            &previous,
            &view.cached.as_ref().unwrap().bytes
        ));
        assert!(
            view.svg()
                .unwrap()
                .contains("Selected front 34 teeth and rear 34 teeth")
        );
        changed.palette.surface = Color32::BLACK;
        view.prepare(&ctx, changed);
        assert!(view.svg().unwrap().contains("fill=\"#000000\""));
    }

    #[test]
    fn xml_text_is_escaped_for_standalone_export() {
        assert_eq!(
            escape_xml("A&B <1> \"front\" 'rear'"),
            "A&amp;B &lt;1&gt; &quot;front&quot; &apos;rear&apos;"
        );
    }

    #[test]
    fn generated_svg_is_accepted_by_the_actual_svg_renderer() {
        let drawing = build_svg(&key("200,1", "1,50,200", 440));
        let image = egui_extras::image::load_svg_bytes(drawing.svg.as_bytes(), &Default::default())
            .expect("The SVG renderer should accept every generated path");
        assert_eq!(image.size[0], drawing.width as usize);
        assert_eq!(image.size[1], drawing.height as usize);
    }

    #[test]
    fn wide_layout_places_both_inventories_side_by_side_within_bounds() {
        let drawing = build_svg(&key("50,34", "11,12,13,14,16,18,20,22,25,28,32", 856));
        assert!(drawing.height <= 510.0);
        let fronts: Vec<_> = drawing
            .targets
            .iter()
            .filter(|target| target.kind == GearKind::Front)
            .collect();
        let rears: Vec<_> = drawing
            .targets
            .iter()
            .filter(|target| target.kind == GearKind::Rear)
            .collect();
        assert_eq!(fronts[0].rect.top(), rears[0].rect.top());
        assert!(fronts.iter().all(|front| {
            rears
                .iter()
                .all(|rear| rear.rect.right() < front.rect.left())
        }));
        let bounds = Rect::from_min_size(Pos2::ZERO, egui::vec2(drawing.width, drawing.height));
        assert!(
            drawing
                .targets
                .iter()
                .all(|target| bounds.contains_rect(target.rect))
        );
    }

    #[test]
    fn clicking_a_cog_updates_selected_pair_and_export_in_the_same_frame() {
        let ctx = egui::Context::default();
        let setup = GearSetup::parse("50,34", "11,34", 2_100.0).unwrap();
        let mut view = DrivetrainView::default();
        let mut selected = Some((34, 34));
        let render = |view: &mut DrivetrainView,
                      selected: &mut Option<(u16, u16)>,
                      events: Vec<egui::Event>| {
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(500.0, 1_000.0))),
                    events,
                    ..Default::default()
                },
                |ui| view.show(ui, &setup, selected),
            )
            .drop_without_applying_deltas();
        };
        render(&mut view, &mut selected, Vec::new());
        let point = view.cached.as_ref().unwrap().drawing.targets[0]
            .rect
            .center();
        for pressed in [true, false] {
            render(
                &mut view,
                &mut selected,
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        assert_eq!(selected, Some((50, 34)));
        assert!(
            view.svg()
                .unwrap()
                .contains("Selected front 50 teeth and rear 34 teeth")
        );
    }
}
