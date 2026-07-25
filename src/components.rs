#[cfg(feature = "serialize")]
use bevy::reflect::{ReflectDeserialize, ReflectSerialize};
use bevy::{
    asset::{AsAssetId, AssetId, Handle, HandleTemplate, UntypedAssetId},
    ecs::{
        component::Component,
        entity::Entity,
        query::QueryItem,
        reflect::{ReflectComponent, ReflectFromWorld},
        template::{FromTemplate, OptionTemplate},
        world::{FromWorld, World},
    },
    math::{Affine2, Mat3, Vec2, Vec3},
    mesh::Mesh,
    prelude::{Deref, DerefMut},
    reflect::{std_traits::ReflectDefault, Reflect},
    render::{extract_component::ExtractComponent, sync_component::SyncComponent},
    transform::components::Transform,
};
use derive_more::derive::From;

use crate::{Material, PointCloud, PointCloudChunk, SimplePointCloudMaterialKey};

#[derive(
    Component, FromTemplate, Clone, Debug, Default, Deref, DerefMut, PartialEq, Eq, From, Reflect,
)]
#[component(immutable)]
#[require(Transform, SplatSettings)]
#[reflect(Component, PartialEq, Debug, FromWorld, Clone, Default)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serialize", reflect(Serialize, Deserialize))]
pub struct PointCloud3d(pub Handle<PointCloud>);

impl AsAssetId for PointCloud3d {
    type Asset = PointCloud;

    fn as_asset_id(&self) -> AssetId<Self::Asset> {
        self.into()
    }
}

impl SyncComponent for PointCloud3d {
    type Target = Self;
}

impl ExtractComponent for PointCloud3d {
    type QueryData = &'static PointCloud3d;
    type QueryFilter = ();
    type Out = Self;

    fn extract_component(item: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        Some(item.clone())
    }
}

impl From<&PointCloud3d> for AssetId<PointCloud> {
    fn from(value: &PointCloud3d) -> Self {
        value.id()
    }
}

impl From<&PointCloud3d> for UntypedAssetId {
    fn from(value: &PointCloud3d) -> Self {
        value.id().untyped()
    }
}

#[derive(
    Component, FromTemplate, Clone, Debug, Default, Deref, DerefMut, PartialEq, Eq, From, Reflect,
)]
#[reflect(Component, PartialEq, Debug, FromWorld, Clone)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serialize", reflect(Serialize, Deserialize))]
pub struct PointCloudChunk3d(pub Handle<PointCloudChunk>);

impl AsAssetId for PointCloudChunk3d {
    type Asset = PointCloudChunk;

    fn as_asset_id(&self) -> AssetId<Self::Asset> {
        self.into()
    }
}

impl SyncComponent for PointCloudChunk3d {
    type Target = Self;
}

impl ExtractComponent for PointCloudChunk3d {
    type QueryData = &'static PointCloudChunk3d;
    type QueryFilter = ();
    type Out = Self;

    fn extract_component(item: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        Some(item.clone())
    }
}

impl From<&PointCloudChunk3d> for AssetId<PointCloudChunk> {
    fn from(value: &PointCloudChunk3d) -> Self {
        value.id()
    }
}

impl From<&PointCloudChunk3d> for UntypedAssetId {
    fn from(value: &PointCloudChunk3d) -> Self {
        value.id().untyped()
    }
}

#[derive(Component, FromTemplate, Clone, Debug, PartialEq, From, Reflect)]
#[component(immutable)]
#[require(Transform)]
#[reflect(Component, PartialEq, Debug, FromWorld, Clone, Default)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serialize", reflect(Serialize, Deserialize))]
pub struct SplatSettings {
    // --- Sizing ---
    pub point_size_mode: PointSizeMode,

    /// The point size, its units depends on selected [`PointSizeMode`] in field
    /// [`SplatSettings::point_size_mode`].
    /// Using orthographic projection, the size in pixels will always match this size. Using
    /// perspective projection, the point size will fade with distance, and grow. Defaults to
    /// `30.0` in [`PointSizeMode::ScreenPixels`].
    /// Note: the transform scale is applied to the point size.
    pub point_size: f32,

    pub adaptive_point_size: bool,

    /// Min point size in screen space.
    /// Defaults to `None`.
    #[template(OptionTemplate<f32>)]
    pub min_point_size: Option<f32>,

    /// Max point size in screen space.
    /// Defaults to `None`.
    #[template(OptionTemplate<f32>)]
    pub max_point_size: Option<f32>,

    // --- Geometry Splat ---
    /// The splat mesh. If `None`, will be a quad or triangle.
    /// Defaults to `None`.
    #[template(OptionTemplate<HandleTemplate<Mesh>>)]
    pub splat: Option<Handle<Mesh>>,

    /// The splat radius, default to None.
    /// Put `Some(0.5)` for a perfect circle.
    /// If set, the points will be truncated to a circle radius of 1 in the fragment shader.
    #[template(OptionTemplate<f32>)]
    pub radius: Option<f32>,

    /// Determines the splats orientation. See [`SplatOrientation`]
    /// documentation for options.
    pub orientation: SplatOrientation,

    /// The default normal (if missing on the vertex attributes).
    /// Used for the splat orientation in [`SplatOrientation::FaceNormal`] mode.
    pub default_normal: Vec3,

    // --- Mapping Logic ---
    /// Determines the UV mapping type, when a [`SimplePointCloudMaterial::base_color_texture`] is
    /// used. See [`UVMapping`] documentation.
    pub uv_mapping: UVMapping,

    /// The UV U vector on which the point position, in object space, will be projected to compute
    /// U when using a computed UV mapping. Computed UV mapping are:
    /// - [`UVMapping::Planar`]
    /// - TODO: add more (spherical, cylindrical, ...)
    pub uv_u: Vec3,

    /// The UV V vector on which the point position, in object space, will be projected to compute
    /// V when using a computed UV mapping. Computed UV mapping are:
    /// - [`UVMapping::Planar`]
    /// - TODO: add more (spherical, cylindrical, ...)
    pub uv_v: Vec3,

    /// Determines an additionnal transformation to apply to UV coordinates obtained from the
    /// [`PointCloudSplatSettings::uv_mapping`]
    // #[template(OptionTemplate<UVTransform>)]
    pub uv_transform: Affine2,
}

impl Default for SplatSettings {
    fn default() -> Self {
        Self {
            point_size_mode: PointSizeMode::ScreenPixels,
            point_size: 30.0,
            adaptive_point_size: true,
            min_point_size: None,
            max_point_size: None,
            splat: None,
            radius: None,
            orientation: SplatOrientation::Billboard,
            default_normal: Vec3::new(0.0, 1.0, 0.0),
            uv_mapping: UVMapping::SplatOnly,
            uv_u: Vec3::new(0.0, 0.0, 1.0),
            uv_v: Vec3::new(-1.0, 0.0, 0.0),
            uv_transform: Affine2::IDENTITY,
        }
    }
}

impl SyncComponent for SplatSettings {
    type Target = Self;
}

impl ExtractComponent for SplatSettings {
    type QueryData = &'static SplatSettings;
    type QueryFilter = ();
    type Out = Self;

    fn extract_component(item: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        Some(item.clone())
    }
}

#[derive(Component, FromTemplate, Clone, PartialEq, Eq, Debug, Reflect)]
#[reflect(Component, PartialEq, Debug, FromWorld, Clone)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serialize", reflect(Serialize, Deserialize))]
#[relationship(relationship_target = ChildrenChunks)]
pub struct ChildChunkOf(#[entities] pub Entity);

impl ChildChunkOf {
    /// The parent entity of this child entity.
    #[inline]
    pub fn parent(&self) -> Entity {
        self.0
    }
}

// TODO: We need to impl either FromWorld or Default so ChildOf can be registered as Reflect.
// This is because Reflect deserialize by creating an instance and apply a patch on top.
// However ChildOf should only ever be set with a real user-defined entity.  Its worth looking into
// better ways to handle cases like this.
impl FromWorld for ChildChunkOf {
    #[inline(always)]
    fn from_world(_world: &mut World) -> Self {
        ChildChunkOf(Entity::PLACEHOLDER)
    }
}

#[derive(Component, Default, Debug, PartialEq, Eq, Reflect)]
#[relationship_target(relationship = ChildChunkOf, linked_spawn)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serialize", reflect(Serialize, Deserialize))]
#[reflect(Component, FromWorld, Default)]
pub struct ChildrenChunks(Vec<Entity>);

/// A [material](Material) used for rendering a [`Mesh3d`].
///
/// See [`Material`] for general information about 3D materials and how to implement your own
/// materials.
///
/// [`Mesh3d`]: bevy_mesh::Mesh3d
///
/// # Example
///
/// ```
/// # use bevy_pbr::{Material, MeshMaterial3d, StandardMaterial};
/// # use bevy_ecs::prelude::*;
/// # use bevy_mesh::{Mesh, Mesh3d};
/// # use bevy_color::palettes::basic::RED;
/// # use bevy_asset::Assets;
/// # use bevy_math::primitives::Capsule3d;
/// #
/// // Spawn an entity with a mesh using `StandardMaterial`.
/// fn setup(
///     mut commands: Commands,
///     mut meshes: ResMut<Assets<Mesh>>,
///     mut materials: ResMut<Assets<StandardMaterial>>,
/// ) {
///     commands.spawn((
///         Mesh3d(meshes.add(Capsule3d::default())),
///         MeshMaterial3d(materials.add(StandardMaterial {
///             base_color: RED.into(),
///             ..Default::default()
///         })),
///     ));
/// }
/// ```
#[derive(Component, FromTemplate, Clone, Debug, Deref, DerefMut, Reflect, From)]
#[reflect(Component, Default, Clone, PartialEq)]
pub struct PointCloudMaterial3d<M: Material>(pub Handle<M>);

impl<M: Material> Default for PointCloudMaterial3d<M> {
    fn default() -> Self {
        Self(Handle::default())
    }
}

impl<M: Material> PartialEq for PointCloudMaterial3d<M> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<M: Material> Eq for PointCloudMaterial3d<M> {}

impl<M: Material> From<PointCloudMaterial3d<M>> for AssetId<M> {
    fn from(material: PointCloudMaterial3d<M>) -> Self {
        material.id()
    }
}

impl<M: Material> From<&PointCloudMaterial3d<M>> for AssetId<M> {
    fn from(material: &PointCloudMaterial3d<M>) -> Self {
        material.id()
    }
}

impl<M: Material> AsAssetId for PointCloudMaterial3d<M> {
    type Asset = M;

    fn as_asset_id(&self) -> AssetId<Self::Asset> {
        self.id()
    }
}

/// Determines how the point size is interpreted.
#[derive(FromTemplate, Reflect, Debug, Clone, Default, PartialEq, Eq, Hash)]
#[reflect(Default, Debug, Clone)]
pub enum PointSizeMode {
    /// Point size is specified in screen pixels.
    ///
    /// - In [`bevy::camera::Projection::Perspective`] mode, points will appear smaller as they get
    ///   further away from the camera (perspective divide).
    /// - In [`bevy::camera::Projection::Orthographic`] mode, points will maintain a constant pixel
    ///   size regardless of the camera's distance or zoom level.
    ScreenPixels,

    /// Point size is specified in screen pixels, relative to the entity's transform.
    ///
    /// Similar to [`PointSizeMode::ScreenPixels`], but the final size is multiplied by the
    /// entity's [`bevy::transform::components::Transform`] scale. If you scale the entity,
    /// the points will scale accordingly. If the scaling is not the same on all axes, the max
    /// scale will be retained.
    #[default]
    ScreenPixelsLocal,

    /// Point size is specified in world-space units (e.g., meters).
    ///
    /// The size is absolute within the 3D world and is unaffected by the entity's
    /// [`bevy::transform::components::Transform`] scale. Points will correctly scale
    /// with camera distance, zoom, and projection modes (both Perspective and Orthographic).
    WorldSpace,

    /// Point size is specified in local-space units, relative to the entity's transform.
    ///
    /// Similar to [`PointSizeMode::WorldSpace`], but the final size is multiplied by the
    /// entity's [`bevy::transform::components::Transform`] scale. If you scale the entity,
    /// the points will scale accordingly. If the scaling is not the same on all axes, the max
    /// scale will be retained.
    LocalSpace,
}

impl PointSizeMode {
    /// Maps the UV mapping mode to its corresponding bits in `SimplePointCloudMaterialKey`.
    pub fn pipeline_key_bits(&self) -> SimplePointCloudMaterialKey {
        match self {
            PointSizeMode::ScreenPixels => SimplePointCloudMaterialKey::POINT_SIZE_SCREEN,
            PointSizeMode::ScreenPixelsLocal => {
                SimplePointCloudMaterialKey::POINT_SIZE_SCREEN_LOCAL
            }
            PointSizeMode::WorldSpace => SimplePointCloudMaterialKey::POINT_SIZE_WORLD,
            PointSizeMode::LocalSpace => SimplePointCloudMaterialKey::POINT_SIZE_LOCAL,
        }
    }
}

/// Determines the splat orientation.
#[derive(FromTemplate, Reflect, Debug, Clone, Default, PartialEq, Eq, Hash)]
#[reflect(Default, Debug, Clone)]
pub enum SplatOrientation {
    /// The splat always faces the camera (classic billboard).
    #[default]
    Billboard,
    /// The splat is oriented along the point's normal vector.
    /// Works only if a normal is provided, and works better if also a tangent is provided.
    FaceNormal,
}

/// Determine the UV mapping coordinates mode when using a
/// [`SimplePointCloudMaterial::base_color_texture`].
#[derive(FromTemplate, Reflect, Debug, Clone, Default, PartialEq, Eq, Hash)]
#[reflect(Default, Debug, Clone)]
pub enum UVMapping {
    /// Combine global UV (from point cloud data) and Local UV (from the point's splat).
    /// **Needs stabilization.**
    Combined,
    /// Uses UVs defined per vertex in the point cloud. The texture stretches across the whole
    /// cloud and is pixelated. Needs the point cloud's UVs.
    PointCloudOnly,
    /// Uses UVs of the point's local geometry. The texture is repeated on every single point using
    /// splat's UV.
    #[default]
    SplatOnly,
    /// Compute UV coordinates based on the provided [`SimplePointCloudMaterial::uv_u`] and
    /// [`SimplePointCloudMaterial::uv_v`].
    /// **Needs stabilization:**
    ///  * in billboard mode,the texture on each point does not follow the rotation of the view
    ///  * in face normal mode, the U/V projection vectors must match the UV coordinates of the
    ///    splat
    Planar,
}

impl UVMapping {
    /// Maps the UV mapping mode to its corresponding bits in `SimplePointCloudMaterialKey`.
    pub fn pipeline_key_bits(&self) -> SimplePointCloudMaterialKey {
        match self {
            Self::Combined => SimplePointCloudMaterialKey::UV_MAPPING_COMBINED,
            Self::PointCloudOnly => SimplePointCloudMaterialKey::UV_MAPPING_POINT_CLOUD,
            Self::SplatOnly => SimplePointCloudMaterialKey::UV_MAPPING_POINT_SHAPE,
            Self::Planar => SimplePointCloudMaterialKey::UV_MAPPING_PLANAR,
        }
    }
}

#[derive(FromTemplate, Reflect, Debug, Clone, Default, PartialEq)]
pub struct UVTransform {
    /// Décalage U et V (translation)
    pub offset: Vec2,
    /// Répétition / Échelle sur les axes U et V (Scale non-uniforme)
    pub scale: Vec2,
    /// Rotation en radians de la texture
    pub rotation: f32,
}

impl UVTransform {
    /// Computes 3x3 matrix from UV transform
    pub fn compute_matrix(&self) -> Mat3 {
        Mat3::from_translation(self.offset)
            * Mat3::from_angle(self.rotation)
            * Mat3::from_scale(self.scale)
    }
}
