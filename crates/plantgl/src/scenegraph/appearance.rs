//! Colours, materials and textures.
//!
//! Ported from PlantGL `src/cpp/plantgl/scenegraph/appearance/{color,material,
//! texture,appearance}.{h,cpp}` @ 4f3fd6ae6cc89ef8f48285fb005bd80d1ff09189.
//!
//! PlantGL is Copyright CIRAD/INRIA/INRA, authored by F. Boudon et al., and is
//! governed by the CeCILL-C license. This file is a translation of that work
//! and is likewise licensed CeCILL-C; see crates/plantgl/LICENSE.
//!
//! `MonoSpectral` and `MultiSpectral` are not ported: they describe
//! reflectance/transmittance for radiative-transfer work that this crate's
//! callers do not do.

use std::sync::Arc;

use crate::math::{Real, Vec2};

/// An 8-bit RGB colour, as upstream's `Color3` (a `Tuple3<uchar_t>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Color3 {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Color3 {
    pub const BLACK: Color3 = Color3::new(0, 0, 0);
    pub const WHITE: Color3 = Color3::new(255, 255, 255);

    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    /// `getRedClamped` and friends: the component as a 0..=1 real.
    pub fn red_clamped(&self) -> Real {
        self.red as Real / 255.0
    }

    pub fn green_clamped(&self) -> Real {
        self.green as Real / 255.0
    }

    pub fn blue_clamped(&self) -> Real {
        self.blue as Real / 255.0
    }

    /// The three components as 0..=1 reals, the form a renderer wants.
    pub fn to_clamped(&self) -> [Real; 3] {
        [self.red_clamped(), self.green_clamped(), self.blue_clamped()]
    }

    /// Scales every component, saturating at 255. This is how upstream
    /// applies `Material`'s diffuse coefficient.
    pub fn scaled(&self, factor: Real) -> Self {
        let scale = |c: u8| ((c as Real * factor).floor()).clamp(0.0, 255.0) as u8;
        Self::new(scale(self.red), scale(self.green), scale(self.blue))
    }
}

impl Default for Color3 {
    fn default() -> Self {
        Color3::WHITE
    }
}

/// An 8-bit RGBA colour, as upstream's `Color4`.
///
/// Note upstream's convention: the fourth channel is *transparency*, not
/// opacity, and defaults to 0 (fully opaque).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Color4 {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl Color4 {
    pub const fn new(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    /// `Color4(const Color3&, uchar_t alpha = 0)`.
    pub const fn from_color3(color: Color3, alpha: u8) -> Self {
        Self::new(color.red, color.green, color.blue, alpha)
    }

    pub const fn to_color3(self) -> Color3 {
        Color3::new(self.red, self.green, self.blue)
    }

    pub fn to_clamped(&self) -> [Real; 4] {
        [
            self.red as Real / 255.0,
            self.green as Real / 255.0,
            self.blue as Real / 255.0,
            self.alpha as Real / 255.0,
        ]
    }
}

impl Default for Color4 {
    fn default() -> Self {
        Color4::new(255, 255, 255, 0)
    }
}

/// A Phong-ish material, as upstream's `Material`.
///
/// `diffuse` is a *coefficient on `ambient`*, not a colour of its own — see
/// [`Material::diffuse_color`]. Upstream's defaults are reproduced exactly.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Material {
    pub name: Option<String>,
    pub ambient: Color3,
    pub diffuse: Real,
    pub specular: Color3,
    pub emission: Color3,
    pub shininess: Real,
    pub transparency: Real,
}

impl Material {
    pub const DEFAULT_AMBIENT: Color3 = Color3::new(80, 80, 80);
    pub const DEFAULT_DIFFUSE: Real = 2.0;
    pub const DEFAULT_SPECULAR: Color3 = Color3::new(0, 0, 0);
    pub const DEFAULT_EMISSION: Color3 = Color3::new(0, 0, 0);
    pub const DEFAULT_SHININESS: Real = 0.2;
    pub const DEFAULT_TRANSPARENCY: Real = 0.0;

    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            ..Self::default()
        }
    }

    /// `getDiffuseColor()`: `ambient * diffuse`, floored per channel.
    pub fn diffuse_color(&self) -> Color3 {
        self.ambient.scaled(self.diffuse)
    }

    /// `isValid()`: the coefficients must lie in upstream's admissible ranges.
    pub fn is_valid(&self) -> bool {
        self.diffuse >= 0.0
            && (0.0..=1.0).contains(&self.shininess)
            && (0.0..=1.0).contains(&self.transparency)
    }
}

impl Default for Material {
    fn default() -> Self {
        Self {
            name: None,
            ambient: Self::DEFAULT_AMBIENT,
            diffuse: Self::DEFAULT_DIFFUSE,
            specular: Self::DEFAULT_SPECULAR,
            emission: Self::DEFAULT_EMISSION,
            shininess: Self::DEFAULT_SHININESS,
            transparency: Self::DEFAULT_TRANSPARENCY,
        }
    }
}

/// A texture image reference, as upstream's `ImageTexture`.
///
/// Upstream can also carry decoded pixels; this port keeps only the file
/// reference, because the engine owns image loading.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImageTexture {
    pub name: Option<String>,
    pub filename: String,
    pub repeat_s: bool,
    pub repeat_t: bool,
    pub mipmaping: bool,
}

impl ImageTexture {
    pub const DEFAULT_REPEAT_S: bool = true;
    pub const DEFAULT_REPEAT_T: bool = true;
    pub const DEFAULT_MIPMAPING: bool = true;

    pub fn new(filename: impl Into<String>) -> Self {
        Self {
            name: None,
            filename: filename.into(),
            repeat_s: Self::DEFAULT_REPEAT_S,
            repeat_t: Self::DEFAULT_REPEAT_T,
            mipmaping: Self::DEFAULT_MIPMAPING,
        }
    }
}

/// The affine transform applied to texture coordinates, as upstream's
/// `Texture2DTransformation`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Texture2DTransformation {
    pub scale: Vec2,
    pub translation: Vec2,
    pub rotation_center: Vec2,
    pub rotation_angle: Real,
}

impl Texture2DTransformation {
    /// `transform(const Vector2&)`: scale, then rotate about the rotation
    /// centre, then translate.
    pub fn transform(&self, uv: Vec2) -> Vec2 {
        let scaled = Vec2::new(uv.x * self.scale.x, uv.y * self.scale.y) - self.rotation_center;
        let (sin, cos) = self.rotation_angle.sin_cos();
        let rotated = Vec2::new(
            cos * scaled.x - sin * scaled.y,
            sin * scaled.x + cos * scaled.y,
        );
        rotated + self.rotation_center + self.translation
    }
}

impl Default for Texture2DTransformation {
    fn default() -> Self {
        Self {
            scale: Vec2::new(1.0, 1.0),
            translation: Vec2::new(0.0, 0.0),
            rotation_center: Vec2::new(0.5, 0.5),
            rotation_angle: 0.0,
        }
    }
}

/// A textured appearance, as upstream's `Texture2D`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Texture2D {
    pub name: Option<String>,
    pub image: ImageTexture,
    pub transformation: Option<Texture2DTransformation>,
    pub base_color: Color4,
}

impl Texture2D {
    /// Upstream's `DEFAULT_BASECOLOR` — opaque white, transparency 0.
    pub const DEFAULT_BASE_COLOR: Color4 = Color4::new(255, 255, 255, 0);

    pub fn new(image: ImageTexture) -> Self {
        Self {
            name: None,
            image,
            transformation: None,
            base_color: Self::DEFAULT_BASE_COLOR,
        }
    }
}

/// Upstream's `Appearance` hierarchy, closed into a sum type.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Appearance {
    Material(Material),
    Texture2D(Texture2D),
}

/// Shared appearance handle. Upstream's `AppearancePtr` is an intrusively
/// refcounted `RCPtr`; `Arc` is atomic, so shapes can be processed in
/// parallel.
pub type AppearanceRef = Arc<Appearance>;

impl Appearance {
    /// The `name` field every upstream `SceneObject` carries. Used by the OBJ
    /// codec to name materials.
    pub fn name(&self) -> Option<&str> {
        match self {
            Appearance::Material(m) => m.name.as_deref(),
            Appearance::Texture2D(t) => t.name.as_deref(),
        }
    }

    /// The colour a renderer should use when it cannot honour the full
    /// appearance.
    pub fn base_color(&self) -> Color3 {
        match self {
            Appearance::Material(m) => m.diffuse_color(),
            Appearance::Texture2D(t) => t.base_color.to_color3(),
        }
    }
}

impl From<Material> for Appearance {
    fn from(m: Material) -> Self {
        Appearance::Material(m)
    }
}

impl From<Texture2D> for Appearance {
    fn from(t: Texture2D) -> Self {
        Appearance::Texture2D(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn default_material_matches_upstream() {
        let m = Material::default();
        assert_eq!(m.ambient, Color3::new(80, 80, 80));
        assert_eq!(m.diffuse, 2.0);
        assert_eq!(m.specular, Color3::new(0, 0, 0));
        assert_eq!(m.shininess, 0.2);
        assert_eq!(m.transparency, 0.0);
        assert!(m.is_valid());
    }

    #[test]
    fn diffuse_color_is_ambient_times_diffuse() {
        let m = Material::default();
        assert_eq!(m.diffuse_color(), Color3::new(160, 160, 160));
    }

    #[test]
    fn diffuse_color_saturates_rather_than_wrapping() {
        let m = Material {
            ambient: Color3::new(200, 200, 200),
            diffuse: 2.0,
            ..Material::default()
        };
        assert_eq!(m.diffuse_color(), Color3::new(255, 255, 255));
    }

    #[test]
    fn texture_transform_is_identity_by_default() {
        let t = Texture2DTransformation::default();
        let uv = Vec2::new(0.25, 0.75);
        assert_relative_eq!(t.transform(uv), uv, epsilon = 1e-6);
    }

    #[test]
    fn texture_transform_rotates_about_the_rotation_centre() {
        let t = Texture2DTransformation {
            rotation_angle: std::f32::consts::FRAC_PI_2,
            ..Texture2DTransformation::default()
        };
        // The rotation centre is fixed.
        assert_relative_eq!(t.transform(Vec2::new(0.5, 0.5)), Vec2::new(0.5, 0.5), epsilon = 1e-6);
        // (1, 0.5) is one quarter-turn from (0.5, 1).
        assert_relative_eq!(t.transform(Vec2::new(1.0, 0.5)), Vec2::new(0.5, 1.0), epsilon = 1e-6);
    }

    #[test]
    fn color4_alpha_is_transparency() {
        let opaque = Color4::default();
        assert_eq!(opaque.alpha, 0);
        assert_eq!(opaque.to_color3(), Color3::WHITE);
    }
}
