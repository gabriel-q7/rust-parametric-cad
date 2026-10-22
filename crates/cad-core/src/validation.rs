//! Validation rules shared by parameters, features, and documents.

use thiserror::Error;

use crate::feature::ObjectId;

/// Largest accepted length (1 km in millimeters). Guards against absurd
/// values that would overflow rendering precision.
pub const MAX_LENGTH_MM: f64 = 1_000_000.0;

/// Largest accepted absolute coordinate for transforms, in millimeters.
pub const MAX_COORDINATE_MM: f64 = 1_000_000.0;

/// Maximum object name length, in characters.
pub const MAX_NAME_LEN: usize = 128;

/// A rule violation detected while validating CAD data.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ValidationError {
    #[error("{parameter} must be a finite number")]
    NotFinite { parameter: &'static str },
    #[error("{parameter} must be greater than 0 mm (got {value})")]
    NotPositive { parameter: &'static str, value: f64 },
    #[error("{parameter} must not exceed {max} mm (got {value})")]
    TooLarge {
        parameter: &'static str,
        value: f64,
        max: f64,
    },
    #[error("object name must not be empty")]
    EmptyName,
    #[error("object name must be at most {MAX_NAME_LEN} characters")]
    NameTooLong,
    #[error("duplicate object id {0}")]
    DuplicateId(ObjectId),
    #[error("{0}")]
    Mesh(Box<crate::mesh::MeshError>),
    #[error("mesh provenance must be a parametric primitive")]
    InvalidSource,
    #[error("object {id}: {source}")]
    InFeature {
        id: ObjectId,
        #[source]
        source: Box<ValidationError>,
    },
}

/// Validates a strictly positive, finite length in millimeters.
pub fn validate_length(parameter: &'static str, value: f64) -> Result<f64, ValidationError> {
    if !value.is_finite() {
        return Err(ValidationError::NotFinite { parameter });
    }
    if value <= 0.0 {
        return Err(ValidationError::NotPositive { parameter, value });
    }
    if value > MAX_LENGTH_MM {
        return Err(ValidationError::TooLarge {
            parameter,
            value,
            max: MAX_LENGTH_MM,
        });
    }
    Ok(value)
}

/// Validates a finite coordinate (any sign) in millimeters.
pub fn validate_coordinate(parameter: &'static str, value: f64) -> Result<f64, ValidationError> {
    if !value.is_finite() {
        return Err(ValidationError::NotFinite { parameter });
    }
    if value.abs() > MAX_COORDINATE_MM {
        return Err(ValidationError::TooLarge {
            parameter,
            value,
            max: MAX_COORDINATE_MM,
        });
    }
    Ok(value)
}

/// Validates a finite angle in degrees.
pub fn validate_angle(parameter: &'static str, value: f64) -> Result<f64, ValidationError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(ValidationError::NotFinite { parameter })
    }
}

/// Normalizes and validates an object name: surrounding whitespace is trimmed.
pub fn normalize_name(name: &str) -> Result<String, ValidationError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(ValidationError::EmptyName);
    }
    if trimmed.chars().count() > MAX_NAME_LEN {
        return Err(ValidationError::NameTooLong);
    }
    Ok(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_positive_lengths() {
        assert_eq!(validate_length("width", 0.001), Ok(0.001));
        assert_eq!(validate_length("width", MAX_LENGTH_MM), Ok(MAX_LENGTH_MM));
    }

    #[test]
    fn rejects_zero_negative_and_non_finite_lengths() {
        assert!(matches!(
            validate_length("width", 0.0),
            Err(ValidationError::NotPositive { .. })
        ));
        assert!(matches!(
            validate_length("width", -5.0),
            Err(ValidationError::NotPositive { .. })
        ));
        assert!(matches!(
            validate_length("width", f64::NAN),
            Err(ValidationError::NotFinite { .. })
        ));
        assert!(matches!(
            validate_length("width", f64::INFINITY),
            Err(ValidationError::NotFinite { .. })
        ));
        assert!(matches!(
            validate_length("width", MAX_LENGTH_MM * 2.0),
            Err(ValidationError::TooLarge { .. })
        ));
    }

    #[test]
    fn coordinates_may_be_negative_but_finite() {
        assert_eq!(validate_coordinate("x", -20.0), Ok(-20.0));
        assert!(validate_coordinate("x", f64::NAN).is_err());
    }

    #[test]
    fn names_are_trimmed_and_must_not_be_empty() {
        assert_eq!(normalize_name("  Bracket  "), Ok("Bracket".to_owned()));
        assert_eq!(normalize_name("   "), Err(ValidationError::EmptyName));
        assert_eq!(
            normalize_name(&"x".repeat(MAX_NAME_LEN + 1)),
            Err(ValidationError::NameTooLong)
        );
    }
}
