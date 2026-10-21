//! Unit conventions.
//!
//! Every length stored in a [`Document`](crate::Document) is expressed in the
//! document's [`LengthUnit`]. Phase 1 supports a single unit: millimeters.
//! Angles are stored in degrees.
//!
//! Coordinate convention: right-handed, **Z-up** (the usual CAD convention).
//! Renderers with a different convention (Bevy is Y-up, meters) convert at
//! their own boundary.

use serde::{Deserialize, Serialize};

/// Number of millimeters in one meter.
pub const MM_PER_M: f64 = 1000.0;

/// Length unit used by a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum LengthUnit {
    #[default]
    #[serde(rename = "mm")]
    Millimeter,
}

impl LengthUnit {
    /// Short symbol shown to users, e.g. `"mm"`.
    pub fn symbol(self) -> &'static str {
        match self {
            LengthUnit::Millimeter => "mm",
        }
    }

    /// Converts a length expressed in this unit to meters.
    pub fn to_meters(self, value: f64) -> f64 {
        match self {
            LengthUnit::Millimeter => mm_to_m(value),
        }
    }
}

/// Converts millimeters to meters.
pub fn mm_to_m(mm: f64) -> f64 {
    mm / MM_PER_M
}

/// Converts meters to millimeters.
pub fn m_to_mm(m: f64) -> f64 {
    m * MM_PER_M
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_between_mm_and_m() {
        assert_eq!(mm_to_m(1500.0), 1.5);
        assert_eq!(m_to_mm(0.25), 250.0);
        assert_eq!(LengthUnit::Millimeter.to_meters(10.0), 0.01);
    }

    #[test]
    fn serializes_unit_as_symbol() {
        let json = serde_json::to_string(&LengthUnit::Millimeter).unwrap();
        assert_eq!(json, "\"mm\"");
        assert_eq!(LengthUnit::Millimeter.symbol(), "mm");
    }
}
