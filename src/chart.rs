use std::collections::{BTreeMap, BTreeSet};

use cogcalc::gearing::{GearRow, SpeedUnit};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Shape, Stroke};

use crate::theme;

/// A point-selectable speed chart, drawn directly with egui's native painter.
pub fn speed_chart(
    ui: &mut egui::Ui,
    rows: &[GearRow],
    cadence: f64,
    unit: SpeedUnit,
    selected: &mut Option<(u16, u16)>,
) {
    let colors = theme::palette(ui);
    let mut series = BTreeMap::<u16, Vec<(&GearRow, f64)>>::new();
    let mut rear_teeth = BTreeSet::<u16>::new();
    let mut highest_speed = 0.0_f64;
    for row in rows {
        let speed = unit.from_kmh(row.speed_kmh(cadence));
        if speed.is_finite() && speed >= 0.0 {
            series
                .entry(row.front_teeth)
                .or_default()
                .push((row, speed));
            rear_teeth.insert(row.rear_teeth);
            highest_speed = highest_speed.max(speed);
        }
    }
    for points in series.values_mut() {
        points.sort_by_key(|(row, _)| row.rear_teeth);
    }

    let width = ui.available_width().max(1.0);
    let legend_columns = ((width / 140.0) as usize).max(1);
    let legend_rows = series.len().div_ceil(legend_columns).max(1);
    let height = 240.0 + 23.0 * (legend_rows - 1) as f32;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), Sense::click());
    let painter = ui.painter_at(rect);

    if series.is_empty() || width < 110.0 {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "No gear data",
            FontId::proportional(13.0),
            colors.muted,
        );
        return;
    }

    for (index, front) in series.keys().enumerate() {
        let x = rect.left() + (index % legend_columns) as f32 * 140.0;
        let y = rect.top() + (index / legend_columns) as f32 * 23.0 + 8.0;
        let color = colors.chart_colors[index % colors.chart_colors.len()];
        painter.line_segment(
            [egui::pos2(x, y), egui::pos2(x + 15.0, y)],
            Stroke::new(2.5, color),
        );
        painter.circle_filled(egui::pos2(x + 7.5, y), 3.0, color);
        painter.text(
            egui::pos2(x + 22.0, y),
            Align2::LEFT_CENTER,
            format!("{front} teeth front"),
            FontId::proportional(12.0),
            colors.text,
        );
    }

    let plot = Rect::from_min_max(
        rect.min + egui::vec2(40.0, legend_rows as f32 * 23.0 + 24.0),
        rect.max - egui::vec2(13.0, 43.0),
    );
    // A single sprocket is centered; repeated values and zero cadence need no special division.
    let first_rear = f64::from(*rear_teeth.first().unwrap());
    let last_rear = f64::from(*rear_teeth.last().unwrap());
    let (x_min, x_max) = if first_rear == last_rear {
        (first_rear - 1.0, first_rear + 1.0)
    } else {
        (first_rear, last_rear)
    };
    let y_step = nice_step((highest_speed * 1.08).max(1.0) / 4.0);
    let tick_count = ((highest_speed / y_step).ceil() as usize).clamp(1, 5);
    let y_max = y_step * tick_count as f64;
    let position = |rear: u16, speed: f64| -> Pos2 {
        egui::pos2(
            plot.left() + ((f64::from(rear) - x_min) / (x_max - x_min)) as f32 * plot.width(),
            plot.bottom() - (speed / y_max) as f32 * plot.height(),
        )
    };

    painter.text(
        egui::pos2(plot.left(), plot.top() - 15.0),
        Align2::LEFT_CENTER,
        format!("Speed ({})", unit.label()),
        FontId::proportional(11.0),
        colors.muted,
    );
    for tick in 0..=tick_count {
        let value = tick as f64 * y_step;
        let y = plot.bottom() - (value / y_max) as f32 * plot.height();
        painter.line_segment(
            [egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)],
            Stroke::new(1.0, colors.border),
        );
        let label = if y_step.fract().abs() > f64::EPSILON {
            format!("{value:.2}")
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_owned()
        } else {
            format!("{value:.0}")
        };
        painter.text(
            egui::pos2(plot.left() - 9.0, y),
            Align2::RIGHT_CENTER,
            label,
            FontId::proportional(11.0),
            colors.muted,
        );
    }

    let mut previous_label = f32::NEG_INFINITY;
    for rear in &rear_teeth {
        let x = position(*rear, 0.0).x;
        let is_last = f64::from(*rear) == last_rear;
        let room_for_last = is_last || plot.right() - x > 26.0;
        if (x - previous_label >= 28.0 && room_for_last) || is_last {
            painter.text(
                egui::pos2(x, plot.bottom() + 15.0),
                Align2::CENTER_CENTER,
                rear.to_string(),
                FontId::proportional(11.0),
                colors.muted,
            );
            previous_label = x;
        }
        painter.line_segment(
            [
                egui::pos2(x, plot.bottom()),
                egui::pos2(x, plot.bottom() + 4.0),
            ],
            Stroke::new(1.0, colors.border),
        );
    }
    painter.text(
        egui::pos2(plot.center().x, rect.bottom() - 3.0),
        Align2::CENTER_BOTTOM,
        "Rear gear size (teeth)",
        FontId::proportional(11.0),
        colors.muted,
    );

    let point_painter = ui.painter_at(plot.expand(8.0).intersect(rect));
    let pointer = response.hover_pos();
    let mut hovered: Option<(&GearRow, f64, Pos2, Color32, f32)> = None;
    for (index, points) in series.values().enumerate() {
        let color = colors.chart_colors[index % colors.chart_colors.len()];
        let positions: Vec<Pos2> = points
            .iter()
            .map(|(row, speed)| position(row.rear_teeth, *speed))
            .collect();
        if positions.len() > 1 {
            point_painter.add(Shape::line(positions.clone(), Stroke::new(2.5, color)));
        }
        for ((row, speed), pos) in points.iter().zip(positions) {
            let is_selected = *selected == Some((row.front_teeth, row.rear_teeth));
            if is_selected {
                point_painter.circle_filled(pos, 7.5, colors.surface);
                point_painter.circle_stroke(pos, 7.0, Stroke::new(1.5, color));
            }
            point_painter.circle_filled(pos, if is_selected { 4.0 } else { 3.3 }, color);
            if let Some(pointer) = pointer {
                let distance = pointer.distance_sq(pos);
                if distance <= 16.0_f32.powi(2) && hovered.as_ref().is_none_or(|h| distance < h.4) {
                    hovered = Some((row, *speed, pos, color, distance));
                }
            }
        }
    }

    if let Some((row, speed, pos, color, _)) = hovered {
        point_painter.circle(pos, 5.5, color, Stroke::new(2.0, colors.surface));
        if response.clicked() {
            *selected = Some((row.front_teeth, row.rear_teeth));
        }
        response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_ui_at_pointer(|ui| {
                ui.strong(format!(
                    "Front: {} teeth · Rear: {} teeth",
                    row.front_teeth, row.rear_teeth
                ));
                ui.label(format!("{speed:.1} {} at {cadence:.0} rpm", unit.label()));
                ui.label(format!("{:.2} wheel turns per pedal turn", row.ratio));
                ui.label(
                    egui::RichText::new("Click to inspect this gear")
                        .small()
                        .color(colors.muted),
                );
            });
    }
}

fn nice_step(target: f64) -> f64 {
    let magnitude = 10.0_f64.powf(target.log10().floor());
    let normalized = target / magnitude;
    let step = if normalized <= 1.0 {
        1.0
    } else if normalized <= 2.0 {
        2.0
    } else if normalized <= 2.5 {
        2.5
    } else if normalized <= 5.0 {
        5.0
    } else {
        10.0
    };
    step * magnitude
}
