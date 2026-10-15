use std::error::Error;
use std::f64::consts::PI;
use std::fmt::{self, Write};

/// The comparison speeds supported by the original Python calculator.
pub const SPEED_THRESHOLDS_KMH: [f64; 5] = [5.0, 10.0, 20.0, 30.0, 40.0];

const MAX_TEETH: u16 = 200;
const MAX_CHAINRINGS: usize = 3;
const MAX_SPROCKETS: usize = 64;
const KM_PER_MILE: f64 = 1.609_344;

/// A nominal tire size. Circumference is a geometric estimate, not a measured
/// rollout: tire shape, pressure, rim width, and load change the actual value.
/// ETRTO gives tire width and bead-seat diameter in millimeters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WheelPreset {
    pub name: &'static str,
    pub bead_seat_diameter_mm: u16,
    pub tire_width_mm: u16,
}

impl WheelPreset {
    pub const ALL: &'static [Self] = &[
        Self::new("700 × 28C · road", 622, 28),
        Self::new("700 × 25C · road", 622, 25),
        Self::new("700 × 32C · all-road", 622, 32),
        Self::new("700 × 40C · gravel", 622, 40),
        Self::new("650B × 47 · gravel", 584, 47),
        Self::new("29 × 2.25 · mountain", 622, 57),
        Self::new("27.5 × 2.25 · mountain", 584, 57),
        Self::new("26 × 2.0 · mountain", 559, 50),
        Self::new("24 × 1.75 · small wheel", 507, 47),
        Self::new("20 × 1.75 · folding", 406, 47),
    ];

    pub const fn new(name: &'static str, bead_seat_diameter_mm: u16, tire_width_mm: u16) -> Self {
        Self {
            name,
            bead_seat_diameter_mm,
            tire_width_mm,
        }
    }

    pub fn circumference_mm(self) -> f64 {
        (f64::from(self.bead_seat_diameter_mm) + 2.0 * f64::from(self.tire_width_mm)) * PI
    }

    pub fn etrto(self) -> String {
        format!("{}-{}", self.tire_width_mm, self.bead_seat_diameter_mm)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SpeedUnit {
    #[default]
    Metric,
    Imperial,
}

impl SpeedUnit {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Metric => "km/h",
            Self::Imperial => "mph",
        }
    }

    pub fn from_kmh(self, speed_kmh: f64) -> f64 {
        match self {
            Self::Metric => speed_kmh,
            Self::Imperial => speed_kmh / KM_PER_MILE,
        }
    }

    pub fn to_kmh(self, speed: f64) -> f64 {
        match self {
            Self::Metric => speed,
            Self::Imperial => speed * KM_PER_MILE,
        }
    }
}

/// Editable drivetrain input, separate from its validated calculation state.
#[derive(Clone, Debug, PartialEq)]
pub struct GearInput {
    pub chainrings: String,
    pub sprockets: String,
    pub circumference_mm: f64,
}

impl GearInput {
    pub fn validate(&self) -> Result<GearSetup, GearError> {
        GearSetup::parse(&self.chainrings, &self.sprockets, self.circumference_mm)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GearError {
    Empty(&'static str),
    InvalidTeeth { field: &'static str, value: String },
    TooMany { field: &'static str, max: usize },
    InvalidRange,
    InvalidCircumference,
}

impl fmt::Display for GearError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty(field) => write!(f, "Enter at least one {field} tooth count."),
            Self::InvalidTeeth { field, value } => write!(
                f,
                "Invalid {field} value “{value}”. Use whole numbers from 1 to {MAX_TEETH}."
            ),
            Self::TooMany { field, max } => {
                write!(f, "Use at most {max} different {field} tooth counts.")
            }
            Self::InvalidRange => write!(
                f,
                "Use an ascending cassette range such as 9-60, or a list such as 11, 13, 15."
            ),
            Self::InvalidCircumference => {
                write!(
                    f,
                    "Wheel circumference must be a finite number from 100 to 10,000 mm."
                )
            }
        }
    }
}

impl Error for GearError {}

/// A normalized drivetrain. Use `new` or `parse` to validate user input.
#[derive(Clone, Debug, PartialEq)]
pub struct GearSetup {
    /// Largest chainring first; duplicate tooth counts are removed.
    pub chainrings: Vec<u16>,
    /// Smallest sprocket first; duplicate tooth counts are removed.
    pub sprockets: Vec<u16>,
    pub circumference_mm: f64,
}

impl GearSetup {
    pub fn parse(
        chainrings: &str,
        sprockets: &str,
        circumference_mm: f64,
    ) -> Result<Self, GearError> {
        Self::new(
            parse_chainrings(chainrings)?,
            parse_sprockets(sprockets)?,
            circumference_mm,
        )
    }

    pub fn new(
        chainrings: Vec<u16>,
        sprockets: Vec<u16>,
        circumference_mm: f64,
    ) -> Result<Self, GearError> {
        let mut chainrings = normalize_teeth(chainrings, "chainring", MAX_CHAINRINGS)?;
        chainrings.reverse();
        let sprockets = normalize_teeth(sprockets, "sprocket", MAX_SPROCKETS)?;
        if !valid_circumference(circumference_mm) {
            return Err(GearError::InvalidCircumference);
        }
        Ok(Self {
            chainrings,
            sprockets,
            circumference_mm,
        })
    }

    /// One row for every chainring/sprocket combination, largest chainring first.
    /// Invalid publicly mutated values are skipped, never divided by zero.
    pub fn rows(&self) -> Vec<GearRow> {
        if !valid_circumference(self.circumference_mm) {
            return Vec::new();
        }
        let circumference_m = self.circumference_mm / 1_000.0;
        let diameter_inches = self.circumference_mm / PI / 25.4;
        self.chainrings
            .iter()
            .copied()
            .filter(|teeth| (1..=MAX_TEETH).contains(teeth))
            .flat_map(|front_teeth| {
                self.sprockets
                    .iter()
                    .copied()
                    .filter(|teeth| (1..=MAX_TEETH).contains(teeth))
                    .map(move |rear_teeth| {
                        let ratio = f64::from(front_teeth) / f64::from(rear_teeth);
                        GearRow {
                            front_teeth,
                            rear_teeth,
                            ratio,
                            gear_inches: diameter_inches * ratio,
                            development_m: circumference_m * ratio,
                        }
                    })
            })
            .collect()
    }

    pub fn summary(&self) -> Option<GearSummary> {
        GearSummary::from_rows(&self.rows())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GearRow {
    pub front_teeth: u16,
    pub rear_teeth: u16,
    pub ratio: f64,
    pub gear_inches: f64,
    /// Meters travelled per complete crank revolution.
    pub development_m: f64,
}

impl GearRow {
    /// Speed at the given crank cadence in revolutions per minute.
    /// Invalid, negative, or unrepresentable inputs produce zero.
    pub fn speed_kmh(self, cadence_rpm: f64) -> f64 {
        if !valid_nonnegative(cadence_rpm)
            || !self.development_m.is_finite()
            || self.development_m <= 0.0
        {
            return 0.0;
        }
        finite_or_zero(self.development_m * cadence_rpm * 0.06)
    }

    /// Required crank cadence in revolutions per minute for a speed in km/h.
    /// Invalid, negative, or unrepresentable inputs produce zero.
    pub fn cadence_at(self, speed_kmh: f64) -> f64 {
        if !valid_nonnegative(speed_kmh)
            || !self.development_m.is_finite()
            || self.development_m <= 0.0
        {
            return 0.0;
        }
        finite_or_zero(speed_kmh / (self.development_m * 0.06))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GearSummary {
    pub lowest: GearRow,
    pub highest: GearRow,
    /// Highest ratio divided by lowest ratio, multiplied by 100.
    /// A 10–50 cassette on a single chainring has a 500% range.
    pub range_percent: f64,
    /// Number of physical combinations, including equivalent ratios.
    pub combinations: usize,
}

impl GearSummary {
    pub fn from_rows(rows: &[GearRow]) -> Option<Self> {
        let mut valid_rows = rows
            .iter()
            .copied()
            .filter(|row| row.ratio.is_finite() && row.ratio > 0.0);
        let first = valid_rows.next()?;
        let mut lowest = first;
        let mut highest = first;
        let mut combinations = 1;
        for row in valid_rows {
            if row.ratio < lowest.ratio {
                lowest = row;
            }
            if row.ratio > highest.ratio {
                highest = row;
            }
            combinations += 1;
        }
        Some(Self {
            lowest,
            highest,
            range_percent: highest.ratio / lowest.ratio * 100.0,
            combinations,
        })
    }
}

pub fn parse_chainrings(input: &str) -> Result<Vec<u16>, GearError> {
    let mut teeth = parse_list(input, "chainring", MAX_CHAINRINGS)?;
    teeth.reverse();
    Ok(teeth)
}

pub fn parse_sprockets(input: &str) -> Result<Vec<u16>, GearError> {
    let input = input.trim();
    if input.contains('-') {
        let (start, end) = input.split_once('-').ok_or(GearError::InvalidRange)?;
        let start = parse_tooth_count(start.trim(), "sprocket")?;
        let end = parse_tooth_count(end.trim(), "sprocket")?;
        if start > end {
            return Err(GearError::InvalidRange);
        }
        if usize::from(end - start) + 1 > MAX_SPROCKETS {
            return Err(GearError::TooMany {
                field: "sprocket",
                max: MAX_SPROCKETS,
            });
        }
        Ok((start..=end).collect())
    } else {
        parse_list(input, "sprocket", MAX_SPROCKETS)
    }
}

fn parse_list(input: &str, field: &'static str, max: usize) -> Result<Vec<u16>, GearError> {
    let teeth = input
        .split(|character: char| character == ',' || character == ';' || character.is_whitespace())
        .filter(|part| !part.is_empty())
        .map(|part| parse_tooth_count(part, field))
        .collect::<Result<Vec<_>, _>>()?;
    normalize_teeth(teeth, field, max)
}

fn parse_tooth_count(input: &str, field: &'static str) -> Result<u16, GearError> {
    if !input.is_empty()
        && input.bytes().all(|byte| byte.is_ascii_digit())
        && let Ok(value) = input.parse::<u16>()
        && (1..=MAX_TEETH).contains(&value)
    {
        return Ok(value);
    }
    Err(GearError::InvalidTeeth {
        field,
        value: input.to_owned(),
    })
}

fn normalize_teeth(
    mut teeth: Vec<u16>,
    field: &'static str,
    max: usize,
) -> Result<Vec<u16>, GearError> {
    if teeth.is_empty() {
        return Err(GearError::Empty(field));
    }
    for value in &teeth {
        if !(1..=MAX_TEETH).contains(value) {
            return Err(GearError::InvalidTeeth {
                field,
                value: value.to_string(),
            });
        }
    }
    teeth.sort_unstable();
    teeth.dedup();
    if teeth.len() > max {
        return Err(GearError::TooMany { field, max });
    }
    Ok(teeth)
}

fn valid_circumference(value: f64) -> bool {
    value.is_finite() && (100.0..=10_000.0).contains(&value)
}

fn valid_nonnegative(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}

fn finite_or_zero(value: f64) -> f64 {
    if value.is_finite() { value } else { 0.0 }
}

/// UTF-8 CSV with a header and CRLF records. The selected cadence speed uses
/// the requested unit; legacy comparison columns remain explicitly in km/h.
pub fn export_csv(rows: &[GearRow], cadence_rpm: f64, unit: SpeedUnit) -> String {
    export_delimited(rows, cadence_rpm, unit, ',', "\r\n")
}

/// Tab-separated data suitable for spreadsheet clipboard paste.
pub fn export_tsv(rows: &[GearRow], cadence_rpm: f64, unit: SpeedUnit) -> String {
    export_delimited(rows, cadence_rpm, unit, '\t', "\n")
}

fn export_delimited(
    rows: &[GearRow],
    cadence_rpm: f64,
    unit: SpeedUnit,
    separator: char,
    newline: &str,
) -> String {
    let cadence_rpm = if valid_nonnegative(cadence_rpm) {
        cadence_rpm
    } else {
        0.0
    };
    let mut output = String::new();
    // Headers contain no separator or quote characters, so quoting is unnecessary.
    let _ = write!(
        output,
        "Chainring (T){separator}Sprocket (T){separator}Gear ratio{separator}Gear inches{separator}Development (m){separator}Speed at {cadence_rpm:.1} rpm ({})",
        unit.label(),
    );
    for speed in SPEED_THRESHOLDS_KMH {
        let _ = write!(output, "{separator}Cadence at {speed:.0} km/h (rpm)");
    }
    output.push_str(newline);
    for row in rows {
        let _ = write!(
            output,
            "{}{separator}{}{separator}{:.4}{separator}{:.2}{separator}{:.3}{separator}{:.2}",
            row.front_teeth,
            row.rear_teeth,
            row.ratio,
            row.gear_inches,
            row.development_m,
            unit.from_kmh(row.speed_kmh(cadence_rpm)),
        );
        for speed in SPEED_THRESHOLDS_KMH {
            let _ = write!(output, "{separator}{:.2}", row.cadence_at(speed));
        }
        output.push_str(newline);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0),
            "actual {actual}, expected {expected}"
        );
    }

    #[test]
    fn known_drivetrain_values_and_inverse_cadence() {
        let setup = GearSetup::parse("50", "25", 2_100.0).unwrap();
        let row = setup.rows()[0];
        close(row.ratio, 2.0);
        close(row.development_m, 4.2);
        close(row.gear_inches, 2_100.0 / PI / 25.4 * 2.0);
        close(row.speed_kmh(90.0), 22.68);
        close(row.cadence_at(22.68), 90.0);
        close(row.cadence_at(0.0), 0.0);
    }

    #[test]
    fn legacy_29_inch_calculations_match_for_all_52_sprockets() {
        let diameter_inches = 29.0;
        let circumference_m = diameter_inches * 0.0254 * PI;
        let setup = GearSetup::parse("42", "9-60", circumference_m * 1_000.0).unwrap();
        let rows = setup.rows();
        assert_eq!(rows.len(), 52);
        for row in rows {
            let legacy_ratio = 42.0 / f64::from(row.rear_teeth);
            close(row.ratio, legacy_ratio);
            close(row.gear_inches, diameter_inches * legacy_ratio);
            for speed in SPEED_THRESHOLDS_KMH {
                let legacy_rpm =
                    (speed * 1_000.0 / 3_600.0 / circumference_m) * 60.0 / legacy_ratio;
                close(row.cadence_at(speed), legacy_rpm);
            }
        }
    }

    #[test]
    fn mixed_delimiters_normalize_duplicate_teeth_and_order() {
        let setup = GearSetup::parse("34, 50;34\n50", "28 11;21,11\t15", 2_100.0).unwrap();
        assert_eq!(setup.chainrings, [50, 34]);
        assert_eq!(setup.sprockets, [11, 15, 21, 28]);
        let rows = setup.rows();
        assert_eq!(rows.len(), 8);
        assert_eq!((rows[0].front_teeth, rows[0].rear_teeth), (50, 11));
        assert_eq!((rows[7].front_teeth, rows[7].rear_teeth), (34, 28));
    }

    #[test]
    fn ranges_are_inclusive_bounded_and_ascending() {
        assert_eq!(parse_sprockets(" 9 - 12 ").unwrap(), [9, 10, 11, 12]);
        assert_eq!(parse_sprockets("200-200").unwrap(), [200]);
        assert_eq!(parse_sprockets("1-64").unwrap().len(), 64);
        for input in [
            "60-9", "1-65", "0-10", "199-201", "9-", "9-12-15", "9-12,15",
        ] {
            assert!(parse_sprockets(input).is_err(), "accepted {input}");
        }
    }

    #[test]
    fn invalid_tooth_counts_and_empty_lists_are_rejected() {
        for input in [
            "",
            ", ;\n",
            "0",
            "201",
            "-1",
            "+1",
            "3.5",
            "NaN",
            "inf",
            "999999999999",
        ] {
            assert!(parse_chainrings(input).is_err(), "accepted {input}");
            assert!(parse_sprockets(input).is_err(), "accepted {input}");
        }
        assert!(parse_chainrings("10,20,30,40").is_err());
        assert_eq!(parse_chainrings("1 200").unwrap(), [200, 1]);
        assert!(GearSetup::new(vec![0], vec![12], 2_100.0).is_err());
        assert!(GearSetup::new(vec![42], vec![201], 2_100.0).is_err());
    }

    #[test]
    fn circumference_checks_reject_nonfinite_and_unusable_values() {
        for circumference in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            -1.0,
            0.0,
            99.9,
            10_001.0,
        ] {
            assert_eq!(
                GearSetup::parse("42", "11", circumference),
                Err(GearError::InvalidCircumference)
            );
        }
        assert!(GearSetup::parse("42", "11", 100.0).is_ok());
        assert!(GearSetup::parse("42", "11", 10_000.0).is_ok());
    }

    #[test]
    fn road_preset_uses_tire_dimensions_instead_of_a_29_inch_diameter() {
        let road = WheelPreset::ALL[0];
        assert_eq!(road.etrto(), "28-622");
        close(road.circumference_mm(), 678.0 * PI);
        for preset in WheelPreset::ALL {
            assert!(valid_circumference(preset.circumference_mm()));
        }
    }

    #[test]
    fn summary_uses_extreme_gears_across_chainrings() {
        let summary = GearSetup::parse("50,34", "11,34", 2_100.0)
            .unwrap()
            .summary()
            .unwrap();
        close(summary.lowest.ratio, 1.0);
        close(summary.highest.ratio, 50.0 / 11.0);
        close(summary.range_percent, 50.0 / 11.0 * 100.0);
        assert_eq!(summary.combinations, 4);
        assert!(GearSummary::from_rows(&[]).is_none());
        let single = GearSetup::parse("40", "10,50", 2_100.0).unwrap();
        close(single.summary().unwrap().range_percent, 500.0);
    }

    #[test]
    fn cadence_and_speed_remain_inverses_across_all_presets() {
        for wheel in WheelPreset::ALL {
            let setup = GearSetup::parse("50,34,22", "9-60", wheel.circumference_mm()).unwrap();
            for row in setup.rows() {
                for cadence in [0.0, 30.0, 60.0, 90.0, 120.0, 200.0] {
                    close(row.cadence_at(row.speed_kmh(cadence)), cadence);
                }
            }
        }
    }

    #[test]
    fn invalid_cadence_or_speed_does_not_propagate_nonfinite_values() {
        let row = GearSetup::parse("42", "11", 2_100.0).unwrap().rows()[0];
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0] {
            assert_eq!(row.speed_kmh(invalid), 0.0);
            assert_eq!(row.cadence_at(invalid), 0.0);
        }
        assert!(row.speed_kmh(f64::MAX).is_finite());
        let invalid = GearRow {
            development_m: 0.0,
            ..row
        };
        assert_eq!(invalid.cadence_at(20.0), 0.0);
        let mut setup = GearSetup::parse("42", "11", 2_100.0).unwrap();
        setup.circumference_mm = f64::NAN;
        assert!(setup.rows().is_empty());
        assert!(setup.summary().is_none());
    }

    #[test]
    fn speed_unit_conversion_has_exact_international_mile_factor() {
        close(SpeedUnit::Imperial.from_kmh(1.609_344), 1.0);
        close(SpeedUnit::Imperial.to_kmh(60.0), 96.560_64);
        close(SpeedUnit::Metric.from_kmh(30.0), 30.0);
        close(SpeedUnit::Metric.to_kmh(30.0), 30.0);
    }

    #[test]
    fn exports_have_labels_consistent_columns_and_legacy_cadences() {
        let rows = GearSetup::parse("50", "25", 2_100.0).unwrap().rows();
        let csv = export_csv(&rows, 90.0, SpeedUnit::Metric);
        let records: Vec<_> = csv.lines().collect();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].split(',').count(), 11);
        assert_eq!(records[1].split(',').count(), 11);
        assert!(csv.contains("Speed at 90.0 rpm (km/h)"));
        assert!(csv.contains("Cadence at 40 km/h (rpm)"));
        assert!(csv.contains("50,25,2.0000,52.63,4.200,22.68,19.84,39.68,79.37,119.05,158.73\r\n"));
        let tsv = export_tsv(&rows, 90.0, SpeedUnit::Imperial);
        assert!(tsv.contains("Speed at 90.0 rpm (mph)"));
        assert!(tsv.lines().nth(1).unwrap().contains("\t14.09\t"));
        assert_eq!(tsv.lines().nth(1).unwrap().split('\t').count(), 11);
        assert!(export_csv(&[], 90.0, SpeedUnit::Metric).lines().count() == 1);
    }
}
