//! A vector drivetrain schematic with an on-demand standalone SVG export.
//! Each outline has the requested number of teeth; dimensions are normalized
//! for legibility, not manufacturing.

use std::cell::OnceCell;
use std::collections::BTreeMap;
use std::f64::consts::{FRAC_PI_2, TAU};
use std::fmt::Write;
use std::sync::Arc;

use cogcalc::GearSetup;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke};

use crate::theme::{self, Palette};

/// Geometry depends only on the gear inventory and available width. Selection
/// and theme changes repaint vectors without generating or rasterizing an SVG.
#[derive(Default)]
pub struct DrivetrainView {
    cached: Option<CachedView>,
}

#[derive(Clone, PartialEq)]
struct ViewKey {
    fronts: Vec<u16>,
    rears: Vec<u16>,
    selected: (u16, u16),
    width: u32,
    palette: Palette,
}

struct CachedView {
    key: ViewKey,
    layout: Arc<DrawingLayout>,
    svg: OnceCell<String>,
}

struct DrawingLayout {
    width: f32,
    height: f32,
    inventories: Vec<InventoryLayout>,
    outlines: BTreeMap<u16, Vec<Pos2>>,
}

struct InventoryLayout {
    origin: Pos2,
    title: String,
    targets: Vec<GearTarget>,
    bottom: f32,
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
    center: Pos2,
    radius: f32,
}

impl DrivetrainView {
    /// Generate the current standalone SVG only when it is requested for export.
    /// The result reflects clicks from the most recent `show` in the same frame.
    pub fn svg(&self) -> Option<&str> {
        self.cached.as_ref().map(|cached| {
            cached
                .svg
                .get_or_init(|| export_svg(&cached.key, &cached.layout))
                .as_str()
        })
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
            self.cached = None;
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
        self.prepare(key);

        let layout = Arc::clone(&self.cached.as_ref().unwrap().layout);
        let scale = display_width / layout.width;
        let size = egui::vec2(display_width, layout.height * scale);
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let transform = egui::emath::TSTransform::new(rect.min.to_vec2(), scale);
        let mut pair = pair;
        let mut hovered = None;
        for target in layout.inventories.iter().flat_map(|list| &list.targets) {
            let hit_rect = transform * target.rect;
            let response = ui.interact(
                hit_rect,
                ui.id().with(("drivetrain", target.kind.id(), target.teeth)),
                Sense::click(),
            );
            response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::SelectableLabel,
                    ui.is_enabled(),
                    match target.kind {
                        GearKind::Front => target.teeth == pair.0,
                        GearKind::Rear => target.teeth == pair.1,
                    },
                    format!("{}: {} teeth", target.kind.label(), target.teeth),
                )
            });
            if response.clicked() {
                match target.kind {
                    GearKind::Front => pair.0 = target.teeth,
                    GearKind::Rear => pair.1 = target.teeth,
                }
            }
            if response.hovered() || response.has_focus() {
                hovered = Some((target.kind, target.teeth));
            }
            response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(format!(
                    "{}: {} teeth. Click to inspect this gear.",
                    target.kind.label(),
                    target.teeth
                ));
        }
        *selected = Some(pair);
        let cached = self.cached.as_mut().unwrap();
        if cached.key.selected != pair {
            cached.key.selected = pair;
            cached.svg.take();
        }
        // Resolve interactions before painting so the highlight and selected
        // pair change immediately, with no extra frame or texture upload.
        paint_drivetrain(ui, rect, scale, cached, hovered);
    }

    fn prepare(&mut self, key: ViewKey) {
        if let Some(cached) = &mut self.cached {
            if cached.key == key {
                return;
            }
            if cached.key.fronts == key.fronts
                && cached.key.rears == key.rears
                && cached.key.width == key.width
            {
                cached.key = key;
                cached.svg.take();
                return;
            }
        }
        let layout = Arc::new(DrawingLayout::new(&key));
        self.cached = Some(CachedView {
            key,
            layout,
            svg: OnceCell::new(),
        });
    }
}

impl DrawingLayout {
    fn new(key: &ViewKey) -> Self {
        let width = key.width as f32;
        let inventory_top = 224.0;
        let (fronts, rears) = if width >= 700.0 {
            let front_width = 230.0;
            let front_x = width - front_width;
            (
                InventoryLayout::new(
                    Pos2::new(front_x, inventory_top),
                    front_width,
                    &key.fronts,
                    GearKind::Front,
                ),
                InventoryLayout::new(
                    Pos2::new(0.0, inventory_top),
                    front_x - 12.0,
                    &key.rears,
                    GearKind::Rear,
                ),
            )
        } else {
            let fronts = InventoryLayout::new(
                Pos2::new(0.0, inventory_top),
                width,
                &key.fronts,
                GearKind::Front,
            );
            let rears = InventoryLayout::new(
                Pos2::new(0.0, fronts.bottom + 16.0),
                width,
                &key.rears,
                GearKind::Rear,
            );
            (fronts, rears)
        };
        let height = fronts.bottom.max(rears.bottom) + 12.0;
        // Unit-radius outlines can be reused for both inventories and the
        // enlarged selected pair, including after selection or theme changes.
        let outlines = key
            .fronts
            .iter()
            .chain(&key.rears)
            .copied()
            .map(|teeth| (teeth, cog_outline(teeth)))
            .collect();
        Self {
            width,
            height,
            inventories: vec![fronts, rears],
            outlines,
        }
    }
}

impl InventoryLayout {
    fn new(origin: Pos2, width: f32, teeth: &[u16], kind: GearKind) -> Self {
        let title = match kind {
            GearKind::Front => format!("Front chainrings ({})", teeth.len()),
            GearKind::Rear => format!("Rear sprockets ({})", teeth.len()),
        };
        let columns = (((width - 24.0) / 96.0) as usize)
            .clamp(1, 8)
            .min(teeth.len());
        let gap = 8.0;
        let cell_width = ((width - 32.0 - gap * (columns - 1) as f32) / columns as f32).min(160.0);
        let cell_height = 108.0;
        let start_x =
            origin.x + (width - (cell_width * columns as f32 + gap * (columns - 1) as f32)) / 2.0;
        let max_teeth = f32::from(*teeth.iter().max().unwrap());
        let targets = teeth
            .iter()
            .copied()
            .enumerate()
            .map(|(index, count)| {
                let x = start_x + (index % columns) as f32 * (cell_width + gap);
                let y = origin.y + 34.0 + (index / columns) as f32 * (cell_height + gap);
                GearTarget {
                    rect: Rect::from_min_size(Pos2::new(x, y), egui::vec2(cell_width, cell_height)),
                    kind,
                    teeth: count,
                    center: Pos2::new(x + cell_width / 2.0, y + 43.0),
                    radius: 19.0 + 17.0 * (f32::from(count) / max_teeth).sqrt(),
                }
            })
            .collect();
        Self {
            origin,
            title,
            targets,
            bottom: origin.y + 34.0 + teeth.len().div_ceil(columns) as f32 * (cell_height + gap),
        }
    }
}

struct VectorPainter<'a> {
    painter: egui::Painter,
    transform: egui::emath::TSTransform,
    palette: Palette,
    outlines: &'a BTreeMap<u16, Vec<Pos2>>,
}

impl VectorPainter<'_> {
    fn text(&self, x: f32, y: f32, size: u16, text: &str, center: bool, color: Color32) {
        self.painter.text(
            self.transform * Pos2::new(x, y + 2.0),
            if center {
                Align2::CENTER_BOTTOM
            } else {
                Align2::LEFT_BOTTOM
            },
            text,
            FontId::proportional(f32::from(size) * self.transform.scaling),
            color,
        );
    }

    fn cog(&self, center: Pos2, radius: f32, teeth: u16, selected: bool) {
        let fill = if selected {
            self.palette.selection
        } else {
            self.palette.background
        };
        let stroke_color = if selected {
            self.palette.accent
        } else {
            self.palette.muted
        };
        let center = self.transform * center;
        let radius = radius * self.transform.scaling;
        let points: Vec<_> = self.outlines[&teeth]
            .iter()
            .map(|point| center + point.to_vec2() * radius)
            .collect();
        // Cog outlines are concave but star-shaped around their center. A fan
        // fills them correctly; convex_polygon would cut across tooth valleys.
        let mut mesh = egui::Mesh::default();
        mesh.vertices.reserve(points.len() + 1);
        mesh.indices.reserve(points.len() * 3);
        mesh.colored_vertex(center, fill);
        for point in &points {
            mesh.colored_vertex(*point, fill);
        }
        for index in 0..points.len() as u32 {
            mesh.add_triangle(0, index + 1, (index + 1) % points.len() as u32 + 1);
        }
        self.painter.add(Shape::mesh(mesh));
        let stroke = Stroke::new(1.1 * self.transform.scaling, stroke_color);
        self.painter.add(Shape::closed_line(points, stroke));
        for hole in 0..5 {
            let angle = f64::from(hole) * TAU / 5.0 - FRAC_PI_2;
            let offset = egui::vec2(angle.cos() as f32, angle.sin() as f32) * radius * 0.49;
            self.painter
                .circle_filled(center + offset, radius * 0.105, self.palette.surface);
        }
        self.painter
            .circle(center, radius * 0.2, self.palette.surface, stroke);
    }
}

fn paint_drivetrain(
    ui: &egui::Ui,
    rect: Rect,
    scale: f32,
    cached: &CachedView,
    hovered: Option<(GearKind, u16)>,
) {
    let layout = &cached.layout;
    let colors = cached.key.palette;
    let painter = VectorPainter {
        painter: ui.painter().with_clip_rect(rect),
        transform: egui::emath::TSTransform::new(rect.min.to_vec2(), scale),
        palette: colors,
        outlines: &layout.outlines,
    };
    painter.painter.rect_filled(rect, 0, colors.surface);
    let (front, rear) = cached.key.selected;
    let width = layout.width;
    let header = Rect::from_min_size(rect.min, egui::vec2(rect.width(), 224.0 * scale));
    if painter.painter.clip_rect().intersects(header) {
        painter.text(
            width / 2.0,
            27.0,
            18,
            &format!("Selected gear: {front} / {rear} teeth"),
            true,
            colors.text,
        );
        painter.text(
            width / 2.0,
            48.0,
            13,
            &format!(
                "Ratio {:.2} · front teeth ÷ rear teeth",
                f64::from(front) / f64::from(rear)
            ),
            true,
            colors.muted,
        );
        let largest = f32::from(front.max(rear));
        let front_radius = 26.0 + 30.0 * (f32::from(front) / largest);
        let rear_radius = 26.0 + 30.0 * (f32::from(rear) / largest);
        let front_center = Pos2::new(width * 0.75, 113.0);
        let rear_center = Pos2::new(width * 0.25, 113.0);
        let chain: Vec<_> = chain_outline(
            rear_center,
            rear_radius + 3.0,
            front_center,
            front_radius + 3.0,
        )
        .into_iter()
        .map(|point| painter.transform * point)
        .collect();
        painter.painter.add(Shape::line(
            chain.clone(),
            Stroke::new(5.0 * scale, colors.muted),
        ));
        painter.painter.extend(Shape::dashed_line(
            &chain,
            Stroke::new(1.8 * scale, colors.surface),
            4.0 * scale,
            4.0 * scale,
        ));
        painter.cog(front_center, front_radius, front, true);
        painter.cog(rear_center, rear_radius, rear, true);
        painter.text(
            front_center.x,
            188.0,
            14,
            &format!("Front · {front} teeth"),
            true,
            colors.text,
        );
        painter.text(
            rear_center.x,
            188.0,
            14,
            &format!("Rear · {rear} teeth"),
            true,
            colors.text,
        );
        painter.text(
            width / 2.0,
            212.0,
            12,
            "Schematic: exact tooth counts, sizes normalized for visibility.",
            true,
            colors.muted,
        );
    }
    for inventory in &layout.inventories {
        let title_rect =
            painter.transform * Rect::from_min_size(inventory.origin, egui::vec2(width, 30.0));
        if painter.painter.clip_rect().intersects(title_rect) {
            painter.text(
                inventory.origin.x + 16.0,
                inventory.origin.y + 20.0,
                16,
                &inventory.title,
                false,
                colors.text,
            );
        }
        for target in &inventory.targets {
            let card = painter.transform * target.rect;
            // Large custom cassettes may extend well below the viewport. Their
            // invisible cogs need neither meshes nor text layout this frame.
            if !painter.painter.clip_rect().intersects(card) {
                continue;
            }
            let selected = target.teeth
                == match target.kind {
                    GearKind::Front => front,
                    GearKind::Rear => rear,
                };
            let highlighted = selected || hovered == Some((target.kind, target.teeth));
            painter.painter.rect(
                card,
                5,
                colors.surface,
                Stroke::new(
                    if highlighted { 2.0 } else { 1.0 } * scale,
                    if highlighted {
                        colors.accent
                    } else {
                        colors.border
                    },
                ),
                egui::StrokeKind::Inside,
            );
            painter.cog(target.center, target.radius, target.teeth, selected);
            painter.text(
                target.center.x,
                target.rect.top() + 96.0,
                14,
                &format!("{} teeth", target.teeth),
                true,
                colors.text,
            );
        }
    }
}

/// Sample the same circular roots and three tooth segments used by SVG export.
/// Root arcs get enough samples even for the valid one- and two-tooth cases.
fn cog_outline(teeth: u16) -> Vec<Pos2> {
    if teeth == 0 {
        return Vec::new();
    }
    let pitch = TAU / f64::from(teeth);
    let root = 1.0 - 0.18_f64.min(pitch * 0.8);
    let base_half = (pitch * 0.32).min(0.32);
    let tip_half = (pitch * 0.18).min(0.18);
    let point = |angle: f64, radius: f64| {
        Pos2::new((angle.cos() * radius) as f32, (angle.sin() * radius) as f32)
    };
    let arc_steps = ((pitch / 2.0 - base_half) / 0.12).ceil().max(1.0) as usize;
    let mut points = Vec::with_capacity(usize::from(teeth) * (arc_steps * 2 + 3));
    for tooth in 0..teeth {
        let angle = -FRAC_PI_2 + f64::from(tooth) * pitch;
        for step in 0..arc_steps {
            points.push(point(
                angle - pitch / 2.0 + (pitch / 2.0 - base_half) * step as f64 / arc_steps as f64,
                root,
            ));
        }
        points.push(point(angle - base_half, root));
        points.push(point(angle - tip_half, 1.0));
        points.push(point(angle + tip_half, 1.0));
        for step in 0..arc_steps {
            points.push(point(
                angle + base_half + (pitch / 2.0 - base_half) * step as f64 / arc_steps as f64,
                root,
            ));
        }
    }
    points
}

fn chain_outline(left: Pos2, left_radius: f32, right: Pos2, right_radius: f32) -> Vec<Pos2> {
    let normal_x = ((left_radius - right_radius) / (right.x - left.x)).clamp(-0.99, 0.99);
    let normal_y = (1.0 - normal_x * normal_x).sqrt();
    let angle = f64::from(normal_y).atan2(f64::from(normal_x));
    let mut points = vec![left + egui::vec2(normal_x, -normal_y) * left_radius];
    for (center, radius, start, sweep) in [
        (right, right_radius, -angle, 2.0 * angle),
        (left, left_radius, angle, TAU - 2.0 * angle),
    ] {
        let steps = (sweep / 0.1).ceil() as usize;
        for step in 0..=steps {
            let angle = start + sweep * step as f64 / steps as f64;
            points.push(center + egui::vec2(angle.cos() as f32, angle.sin() as f32) * radius);
        }
    }
    points
}

struct SvgWriter {
    body: String,
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

    fn inventory(&mut self, inventory: &InventoryLayout, selected: u16) {
        self.text(
            inventory.origin.x + 16.0,
            inventory.origin.y + 20.0,
            16,
            &inventory.title,
            false,
            self.palette.text,
        );
        for (index, target) in inventory.targets.iter().enumerate() {
            let kind = target.kind;
            let count = target.teeth;
            let x = target.rect.left();
            let y = target.rect.top();
            let cell_width = target.rect.width();
            let cell_height = target.rect.height();
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
            self.cog(target.center, target.radius, count, is_selected);
            self.text(
                target.center.x,
                y + 96.0,
                14,
                &format!("{count} teeth"),
                true,
                self.palette.text,
            );
            self.body.push_str("</g>");
        }
    }
}

fn export_svg(key: &ViewKey, layout: &DrawingLayout) -> String {
    let width = layout.width;
    let height = layout.height;
    let mut writer = SvgWriter {
        body: String::new(),
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

    for inventory in &layout.inventories {
        let selected = match inventory.targets[0].kind {
            GearKind::Front => front,
            GearKind::Rear => rear,
        };
        writer.inventory(inventory, selected);
    }
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width:.0}\" height=\"{height:.0}\" viewBox=\"0 0 {width:.0} {height:.0}\" role=\"img\" aria-labelledby=\"title description\"><title id=\"title\">Bicycle drivetrain: {} front chainrings and {} rear sprockets</title><desc id=\"description\">Selected front {front} teeth and rear {rear} teeth. All gear outlines contain their specified number of teeth. Gear sizes and chain routing are schematic, not manufacturing geometry.</desc><rect width=\"100%\" height=\"100%\" fill=\"{}\"/><g font-family=\"sans-serif\">{}</g></svg>",
        key.fronts.len(),
        key.rears.len(),
        hex(key.palette.surface),
        writer.body
    )
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

    struct SvgDrawing {
        svg: String,
        width: f32,
        height: f32,
        targets: Vec<GearTarget>,
    }

    fn build_svg(key: &ViewKey) -> SvgDrawing {
        let layout = DrawingLayout::new(key);
        SvgDrawing {
            svg: export_svg(key, &layout),
            width: layout.width,
            height: layout.height,
            targets: layout
                .inventories
                .iter()
                .flat_map(|inventory| inventory.targets.iter().copied())
                .collect(),
        }
    }

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
            let outline = cog_outline(teeth);
            assert!(outline.iter().all(|point| point.is_finite()));
            // Every tooth has two distinct tip vertices at the outer radius;
            // root-arc samples must stay below that radius, even at 200 teeth.
            assert_eq!(
                outline
                    .iter()
                    .filter(|point| (point.to_vec2().length() - 1.0).abs() < 0.000_01)
                    .count(),
                usize::from(teeth) * 2,
            );
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
    fn selection_and_theme_reuse_geometry_and_only_export_generates_svg() {
        let mut view = DrivetrainView::default();
        assert!(view.svg().is_none());
        let original = key("50,34", "11,34", 800);
        view.prepare(original.clone());
        let previous = view.cached.as_ref().unwrap().layout.clone();
        assert!(view.cached.as_ref().unwrap().svg.get().is_none());
        view.prepare(original.clone());
        assert!(Arc::ptr_eq(
            &previous,
            &view.cached.as_ref().unwrap().layout
        ));
        assert!(
            view.svg()
                .unwrap()
                .contains("Selected front 50 teeth and rear 11 teeth")
        );
        assert!(view.cached.as_ref().unwrap().svg.get().is_some());
        let mut changed = original;
        changed.selected = (34, 34);
        view.prepare(changed.clone());
        assert!(Arc::ptr_eq(
            &previous,
            &view.cached.as_ref().unwrap().layout
        ));
        assert!(view.cached.as_ref().unwrap().svg.get().is_none());
        assert!(
            view.svg()
                .unwrap()
                .contains("Selected front 34 teeth and rear 34 teeth")
        );
        changed.palette.surface = Color32::BLACK;
        view.prepare(changed.clone());
        assert!(Arc::ptr_eq(
            &previous,
            &view.cached.as_ref().unwrap().layout
        ));
        assert!(view.cached.as_ref().unwrap().svg.get().is_none());
        assert!(view.svg().unwrap().contains("fill=\"#000000\""));
        changed.width = 400;
        view.prepare(changed);
        assert!(!Arc::ptr_eq(
            &previous,
            &view.cached.as_ref().unwrap().layout
        ));
        assert!(view.cached.as_ref().unwrap().svg.get().is_none());
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
        let original_layout = view.cached.as_ref().unwrap().layout.clone();
        let point = original_layout.inventories[0].targets[0].rect.center();
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
        assert!(Arc::ptr_eq(
            &original_layout,
            &view.cached.as_ref().unwrap().layout
        ));
        assert!(view.cached.as_ref().unwrap().svg.get().is_none());
        assert!(
            view.svg()
                .unwrap()
                .contains("Selected front 50 teeth and rear 34 teeth")
        );
    }

    #[test]
    fn native_view_does_not_create_image_textures_when_selection_changes() {
        let ctx = egui::Context::default();
        let setup = GearSetup::parse("200,100,1", "137-200", 2_100.0).unwrap();
        let mut view = DrivetrainView::default();
        let mut original_layout = None;
        for rear in 137..=200 {
            let mut selected = Some((200, rear));
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(840.0, 700.0))),
                    ..Default::default()
                },
                |ui| view.show(ui, &setup, &mut selected),
            );
            let primitives =
                ctx.tessellate(std::mem::take(&mut output.shapes), output.pixels_per_point);
            assert!(!primitives.is_empty());
            assert!(
                output
                    .textures_delta
                    .set
                    .iter()
                    .all(|(id, _)| *id == egui::TextureId::Managed(0)),
                "Only egui's shared font atlas may be uploaded"
            );
            let cached = view.cached.as_ref().unwrap();
            assert!(cached.svg.get().is_none());
            if let Some(previous) = &original_layout {
                assert!(Arc::ptr_eq(previous, &cached.layout));
            } else {
                original_layout = Some(cached.layout.clone());
            }
            output.drop_without_applying_deltas();
        }
    }

    /// Run manually with `cargo test --release --locked drivetrain_selection_timing
    /// -- --ignored --nocapture`. Timings are diagnostic, never CI thresholds.
    #[test]
    #[ignore = "manual comparison of native frames with the previous SVG rasterization path"]
    fn drivetrain_selection_timing() {
        use std::hint::black_box;
        use std::time::Instant;

        for (label, fronts, rears) in [
            ("normal 2x11", "50,34", "11,12,13,14,16,18,20,22,25,28,32"),
            ("maximum 3x64", "200,100,1", "137-200"),
        ] {
            let setup = GearSetup::parse(fronts, rears, 2_100.0).unwrap();
            let mut original = key(fronts, rears, 840);
            let layout = DrawingLayout::new(&original);
            let mut old_times = Vec::new();
            for index in 0..3 {
                original.selected.1 = setup.sprockets[index % setup.sprockets.len()];
                let start = Instant::now();
                let svg = export_svg(&original, &layout);
                let image = egui_extras::image::load_svg_bytes(svg.as_bytes(), &Default::default())
                    .unwrap();
                black_box(image);
                old_times.push(start.elapsed().as_secs_f64() * 1_000.0);
            }
            let ctx = egui::Context::default();
            let mut view = DrivetrainView::default();
            let mut native_times = Vec::new();
            for index in 0..125 {
                let mut selected = Some((
                    setup.chainrings[0],
                    setup.sprockets[index % setup.sprockets.len()],
                ));
                let start = Instant::now();
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            egui::vec2(840.0, 700.0),
                        )),
                        ..Default::default()
                    },
                    |ui| view.show(ui, &setup, &mut selected),
                );
                black_box(
                    ctx.tessellate(std::mem::take(&mut output.shapes), output.pixels_per_point),
                );
                output.drop_without_applying_deltas();
                if index >= 5 {
                    native_times.push(start.elapsed().as_secs_f64() * 1_000.0);
                }
            }
            native_times.sort_by(f64::total_cmp);
            let old_mean = old_times.iter().sum::<f64>() / old_times.len() as f64;
            let native_mean = native_times.iter().sum::<f64>() / native_times.len() as f64;
            println!(
                "{label}: SVG generation+raster mean {old_mean:.3} ms (3 samples); native frame+tessellation mean {native_mean:.3} ms, p95 {:.3} ms (120 warm samples); ratio {:.1}x",
                native_times[113],
                old_mean / native_mean
            );
        }
    }
}
