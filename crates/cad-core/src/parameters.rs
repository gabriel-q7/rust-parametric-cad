//! Typed, validated parameters for each primitive.
//!
//! Fields are private: the only way to build parameters in code is through
//! the validating constructors, so a value of these types is always valid.
//! Deserialized values bypass the constructors and are checked by
//! [`Document::validate`](crate::Document::validate) when loading.

use serde::{Deserialize, Serialize};

use crate::validation::{ValidationError, validate_length};

/// Describes one editable scalar parameter, for generic UIs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParameterDescriptor {
    /// Stable key used to identify the parameter when editing.
    pub key: &'static str,
    /// Human-readable label.
    pub label: &'static str,
    /// Current value in document units (millimeters).
    pub value: f64,
}

/// Axis-aligned box. Width runs along X, depth along Y, height along Z.
/// Local origin: centered in X/Y, resting on Z = 0.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoxParameters {
    width: f64,
    depth: f64,
    height: f64,
}

impl BoxParameters {
    pub fn new(width: f64, depth: f64, height: f64) -> Result<Self, ValidationError> {
        let params = Self {
            width,
            depth,
            height,
        };
        params.validate()?;
        Ok(params)
    }

    pub fn width(&self) -> f64 {
        self.width
    }

    pub fn depth(&self) -> f64 {
        self.depth
    }

    pub fn height(&self) -> f64 {
        self.height
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_length("width", self.width)?;
        validate_length("depth", self.depth)?;
        validate_length("height", self.height)?;
        Ok(())
    }

    pub fn descriptors(&self) -> Vec<ParameterDescriptor> {
        vec![
            ParameterDescriptor {
                key: "width",
                label: "Width",
                value: self.width,
            },
            ParameterDescriptor {
                key: "depth",
                label: "Depth",
                value: self.depth,
            },
            ParameterDescriptor {
                key: "height",
                label: "Height",
                value: self.height,
            },
        ]
    }
}

impl Default for BoxParameters {
    fn default() -> Self {
        Self {
            width: 100.0,
            depth: 100.0,
            height: 100.0,
        }
    }
}

/// Sphere. Local origin: centered in X/Y, resting on Z = 0
/// (the center sits at Z = radius).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SphereParameters {
    radius: f64,
}

impl SphereParameters {
    pub fn new(radius: f64) -> Result<Self, ValidationError> {
        let params = Self { radius };
        params.validate()?;
        Ok(params)
    }

    pub fn radius(&self) -> f64 {
        self.radius
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_length("radius", self.radius)?;
        Ok(())
    }

    pub fn descriptors(&self) -> Vec<ParameterDescriptor> {
        vec![ParameterDescriptor {
            key: "radius",
            label: "Radius",
            value: self.radius,
        }]
    }
}

impl Default for SphereParameters {
    fn default() -> Self {
        Self { radius: 50.0 }
    }
}

/// Cylinder with its axis along Z. Local origin: centered in X/Y, base on Z = 0.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CylinderParameters {
    radius: f64,
    height: f64,
}

impl CylinderParameters {
    pub fn new(radius: f64, height: f64) -> Result<Self, ValidationError> {
        let params = Self { radius, height };
        params.validate()?;
        Ok(params)
    }

    pub fn radius(&self) -> f64 {
        self.radius
    }

    pub fn height(&self) -> f64 {
        self.height
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        validate_length("radius", self.radius)?;
        validate_length("height", self.height)?;
        Ok(())
    }

    pub fn descriptors(&self) -> Vec<ParameterDescriptor> {
        vec![
            ParameterDescriptor {
                key: "radius",
                label: "Radius",
                value: self.radius,
            },
            ParameterDescriptor {
                key: "height",
                label: "Height",
                value: self.height,
            },
        ]
    }
}

impl Default for CylinderParameters {
    fn default() -> Self {
        Self {
            radius: 40.0,
            height: 120.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructors_accept_positive_dimensions() {
        let b = BoxParameters::new(10.0, 20.0, 30.0).unwrap();
        assert_eq!((b.width(), b.depth(), b.height()), (10.0, 20.0, 30.0));
        assert_eq!(SphereParameters::new(5.0).unwrap().radius(), 5.0);
        let c = CylinderParameters::new(3.0, 4.0).unwrap();
        assert_eq!((c.radius(), c.height()), (3.0, 4.0));
    }

    #[test]
    fn constructors_reject_zero_and_negative_dimensions() {
        assert!(BoxParameters::new(0.0, 1.0, 1.0).is_err());
        assert!(BoxParameters::new(1.0, -1.0, 1.0).is_err());
        assert!(BoxParameters::new(1.0, 1.0, f64::NAN).is_err());
        assert!(SphereParameters::new(0.0).is_err());
        assert!(SphereParameters::new(-2.0).is_err());
        assert!(CylinderParameters::new(0.0, 1.0).is_err());
        assert!(CylinderParameters::new(1.0, -0.5).is_err());
    }

    #[test]
    fn error_names_the_offending_parameter() {
        let err = BoxParameters::new(1.0, 1.0, -3.0).unwrap_err();
        assert_eq!(
            err,
            ValidationError::NotPositive {
                parameter: "height",
                value: -3.0
            }
        );
    }

    #[test]
    fn defaults_are_valid() {
        BoxParameters::default().validate().unwrap();
        SphereParameters::default().validate().unwrap();
        CylinderParameters::default().validate().unwrap();
    }
}
