//! Features: identified, named, positioned pieces of geometry.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::geometry::Geometry;
use crate::validation::{ValidationError, normalize_name, validate_angle, validate_coordinate};

/// Stable, document-level object identifier (UUID v4).
///
/// IDs are persisted and survive edits, saves, and loads. They are never
/// derived from runtime handles such as ECS entity IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ObjectId(Uuid);

impl ObjectId {
    /// Generates a fresh random identifier.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ObjectId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for ObjectId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Self)
    }
}

/// Rigid placement of a feature: no scale, so dimensions stay parametric.
///
/// The rotation is applied first, as rotations about the fixed X, then Y,
/// then Z axes (degrees); the translation (millimeters) is applied after.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Transform3D {
    pub translation: [f64; 3],
    pub rotation_deg: [f64; 3],
}

impl Transform3D {
    pub fn from_translation(translation: [f64; 3]) -> Self {
        Self {
            translation,
            ..Self::default()
        }
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        const POSITION: [&str; 3] = ["position x", "position y", "position z"];
        const ROTATION: [&str; 3] = ["rotation x", "rotation y", "rotation z"];
        for i in 0..3 {
            validate_coordinate(POSITION[i], self.translation[i])?;
            validate_angle(ROTATION[i], self.rotation_deg[i])?;
        }
        Ok(())
    }
}

/// A named, placed primitive in a document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Feature {
    id: ObjectId,
    name: String,
    geometry: Geometry,
    #[serde(default)]
    transform: Transform3D,
}

impl Feature {
    /// Builds a validated feature.
    pub fn new(
        id: ObjectId,
        name: &str,
        geometry: Geometry,
        transform: Transform3D,
    ) -> Result<Self, ValidationError> {
        let feature = Self {
            id,
            name: normalize_name(name)?,
            geometry,
            transform,
        };
        feature.validate()?;
        Ok(feature)
    }

    pub fn id(&self) -> ObjectId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn geometry(&self) -> &Geometry {
        &self.geometry
    }

    pub fn transform(&self) -> &Transform3D {
        &self.transform
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        let in_feature = |source| ValidationError::InFeature {
            id: self.id,
            source: Box::new(source),
        };
        normalize_name(&self.name).map_err(in_feature)?;
        self.geometry.validate().map_err(in_feature)?;
        self.transform.validate().map_err(in_feature)?;
        Ok(())
    }

    // Mutators are crate-private: documents change only through commands,
    // which validate before calling these.

    pub(crate) fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub(crate) fn set_geometry(&mut self, geometry: Geometry) {
        self.geometry = geometry;
    }

    pub(crate) fn set_transform(&mut self, transform: Transform3D) {
        self.transform = transform;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parameters::SphereParameters;

    fn sphere() -> Geometry {
        Geometry::Sphere(SphereParameters::new(1.0).unwrap())
    }

    #[test]
    fn object_ids_are_unique_and_round_trip_as_strings() {
        let a = ObjectId::new();
        assert_ne!(a, ObjectId::new());
        assert_eq!(a.to_string().parse::<ObjectId>().unwrap(), a);
    }

    #[test]
    fn feature_rejects_blank_names_and_non_finite_transforms() {
        assert!(Feature::new(ObjectId::new(), " ", sphere(), Transform3D::default()).is_err());
        let bad = Transform3D::from_translation([f64::NAN, 0.0, 0.0]);
        assert!(Feature::new(ObjectId::new(), "S", sphere(), bad).is_err());
    }

    #[test]
    fn feature_trims_its_name() {
        let f = Feature::new(ObjectId::new(), "  Ball ", sphere(), Transform3D::default()).unwrap();
        assert_eq!(f.name(), "Ball");
    }
}
