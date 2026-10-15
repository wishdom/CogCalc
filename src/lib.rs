//! Bicycle drivetrain calculations, input validation, and portable table export.
//!
//! This library has no GUI or platform dependencies. Distances are stored in
//! metric units, with display-unit conversion applied only at the boundary.

pub mod gearing;

pub use gearing::{
    GearError, GearInput, GearRow, GearSetup, GearSummary, SPEED_THRESHOLDS_KMH, SpeedUnit,
    WheelPreset, export_csv, export_tsv, parse_chainrings, parse_sprockets,
};
