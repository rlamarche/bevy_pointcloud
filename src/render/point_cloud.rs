use bevy::{
    asset::{AssetId, Handle},
    camera::{primitives::Aabb, visibility::RenderLayers},
    ecs::{
        component::Component,
        entity::{Entity, EntityHashMap},
        resource::Resource,
    },
    math::{Affine2, Affine3, Affine3Ext, Vec4},
    mesh::Mesh,
    pbr::MaterialBindGroupSlot,
    prelude::{Deref, DerefMut},
    render::{
        render_asset::RenderAsset,
        render_resource::{BindGroup, ShaderType, UniformBuffer},
        sync_world::{MainEntity, MainEntityHashMap},
    },
};
use bitflags::bitflags;

use crate::{
    PointCloudChunk, PointCloudTopologyKind, PointSizeMode, SplatOrientation, SplatSettings,
    UVMapping,
};

#[derive(Debug, Clone)]
pub struct RenderPointCloudChunk {
    pub mesh: Option<Handle<Mesh>>,
    pub offset: Option<f32>,
}

impl RenderAsset for RenderPointCloudChunk {
    type SourceAsset = PointCloudChunk;

    type Param = ();

    fn prepare_asset(
        source_asset: Self::SourceAsset,
        _asset_id: AssetId<Self::SourceAsset>,
        _param: &mut bevy::ecs::system::SystemParamItem<Self::Param>,
        _previous_asset: Option<&Self>,
    ) -> Result<Self, bevy::render::render_asset::PrepareAssetError<Self::SourceAsset>> {
        Ok(RenderPointCloudChunk {
            mesh: source_asset.mesh_handle.clone(),
            offset: source_asset.offset,
        })
    }
}

/// Information that the render world keeps about each entity that contains a
/// point cloud.
/// For each point cloud, it stores all the chunk instances, flat.
#[derive(Resource, Default, Deref, DerefMut)]
pub struct RenderPointCloudInstances(MainEntityHashMap<RenderPointCloudInstance>);

/// CPU data that the render world keeps for each entity, when *not* using GPU
/// mesh uniform building.
pub struct RenderPointCloudInstance {
    /// The entity that point to the root chunk entity, used for loading the corresponding
    /// [`PreparedPointCloudUniform`] in the draw command [`crate::SetPointCloudUniformGroup`].
    pub entity: MainEntity,
    pub render_entity: Entity,
    pub aabb: Aabb,
    pub model_aabb: Aabb,
    pub spacing: Option<f32>,
    pub topology: PointCloudTopologyKind,
    /// The transform of the mesh.
    ///
    /// This will be written into the [`MeshUniform`] at the appropriate time.
    pub transforms: PointCloudTransforms,
    /// The set of render layers that this mesh belongs to.
    pub render_layers: Option<RenderLayers>,
    pub splat_settings: SplatSettings,
    pub splat: AssetId<Mesh>,
}

bitflags! {
    #[repr(C)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct SplatPipelineKey: u64 {
        const POINT_SIZE_MODE_BITS_0        = 1_u64 << 0;
        const POINT_SIZE_MODE_BITS_1        = 1_u64 << 1;

        const POINT_SIZE_MODE_MASK          = Self::POINT_SIZE_MODE_BITS_0.bits() | Self::POINT_SIZE_MODE_BITS_1.bits();

        const POINT_SIZE_MODE_SCREEN_PIXELS        = 0;
        const POINT_SIZE_MODE_SCREEN_LOCAL  = Self::POINT_SIZE_MODE_BITS_0.bits();
        const POINT_SIZE_MODE_WORLD         = Self::POINT_SIZE_MODE_BITS_1.bits();
        const POINT_SIZE_MODE_LOCAL         = Self::POINT_SIZE_MODE_BITS_0.bits() | Self::POINT_SIZE_MODE_BITS_1.bits();

        const ADAPTIVE_POINT_SIZE           = 1_u64 << 2;

        const SPLAT_RADIUS                  = 1_u64 <<  3;
        const SPLAT_ORIENTATION_FACE_NORMAL = 1_u64 <<  4;

        const UV_MAPPING_BITS_0        = 1_u64 <<  5;
        const UV_MAPPING_BITS_1        = 1_u64 <<  6;

        const UV_MAPPING_MASK          = Self::UV_MAPPING_BITS_0.bits() | Self::UV_MAPPING_BITS_1.bits();

        const UV_MAPPING_COMBINED      = 0;
        const UV_MAPPING_POINT_CLOUD   = Self::UV_MAPPING_BITS_0.bits();
        const UV_MAPPING_POINT_SHAPE   = Self::UV_MAPPING_BITS_1.bits();
        const UV_MAPPING_PLANAR        = Self::UV_MAPPING_BITS_0.bits() | Self::UV_MAPPING_BITS_1.bits();

        const UV_TRANSFORM             = 1_u64 <<  7;

        const IS_OCTREE                = 1_u64 << 8;
    }
}

// const STANDARD_MATERIAL_KEY_DEPTH_BIAS_SHIFT: u64 = 32;

impl From<&PointSizeMode> for SplatPipelineKey {
    fn from(value: &PointSizeMode) -> Self {
        match value {
            PointSizeMode::ScreenPixels => SplatPipelineKey::POINT_SIZE_MODE_SCREEN_PIXELS,
            PointSizeMode::ScreenPixelsLocal => SplatPipelineKey::POINT_SIZE_MODE_SCREEN_LOCAL,
            PointSizeMode::WorldSpace => SplatPipelineKey::POINT_SIZE_MODE_WORLD,
            PointSizeMode::LocalSpace => SplatPipelineKey::POINT_SIZE_MODE_LOCAL,
        }
    }
}

impl From<&UVMapping> for SplatPipelineKey {
    fn from(value: &UVMapping) -> Self {
        match value {
            UVMapping::Combined => SplatPipelineKey::UV_MAPPING_COMBINED,
            UVMapping::PointCloudOnly => SplatPipelineKey::UV_MAPPING_POINT_CLOUD,
            UVMapping::SplatOnly => SplatPipelineKey::UV_MAPPING_POINT_SHAPE,
            UVMapping::Planar => SplatPipelineKey::UV_MAPPING_PLANAR,
        }
    }
}

impl From<&SplatSettings> for SplatPipelineKey {
    fn from(settings: &SplatSettings) -> Self {
        let mut key = SplatPipelineKey::empty();

        key.insert((&settings.point_size_mode).into());
        key.set(
            SplatPipelineKey::ADAPTIVE_POINT_SIZE,
            settings.adaptive_point_size,
        );
        key.set(SplatPipelineKey::SPLAT_RADIUS, settings.radius.is_some());
        key.set(
            SplatPipelineKey::SPLAT_ORIENTATION_FACE_NORMAL,
            matches!(settings.orientation, SplatOrientation::FaceNormal),
        );

        key.insert((&settings.uv_mapping).into());

        key.set(
            SplatPipelineKey::UV_TRANSFORM,
            !settings.uv_transform.eq(&Affine2::IDENTITY),
        );

        key
    }
}

impl From<u64> for SplatPipelineKey {
    fn from(value: u64) -> Self {
        SplatPipelineKey::from_bits_retain(value)
    }
}

impl From<SplatPipelineKey> for u64 {
    fn from(value: SplatPipelineKey) -> Self {
        value.bits()
    }
}

/// Information that the render world keeps about each entity that contains a
/// mesh.
///
/// The set of information needed is different depending on whether CPU or GPU
/// [`MeshUniform`] building is in use.
#[derive(Resource, Default, Deref, DerefMut)]
pub struct RenderPointCloudChunkInstances(EntityHashMap<RenderPointCloudChunkInstance>);

/// CPU data that the render world keeps for each entity, when *not* using GPU
/// mesh uniform building.
pub struct RenderPointCloudChunkInstance {
    /// The entity that point to the root chunk entity, used for loading the corresponding
    /// [`PreparedPointCloudUniform`] in the draw command [`crate::SetPointCloudUniformGroup`].
    pub root_entity: MainEntity,
    pub is_root: bool,
    pub mesh_asset_id: AssetId<Mesh>,
    pub topology: PointCloudTopologyKind,
}

#[derive(Component)]
pub struct PointCloudTransforms {
    pub world_from_local: Affine3,
    pub previous_world_from_local: Affine3,
}

#[derive(ShaderType, Clone)]
pub struct PointCloudUniform {
    // --- Transformations & Bounds ---
    pub aabb_min: Vec4,
    pub aabb_max: Vec4,
    pub model_center: Vec4,
    pub model_half_extents: Vec4,
    // Affine 4x3 matrices transposed to 3x4
    pub world_from_local: [Vec4; 3],
    pub previous_world_from_local: [Vec4; 3],
    // 3x3 matrix packed in mat2x4 and f32 as:
    //   [0].xyz, [1].x,
    //   [1].yz, [2].xy
    //   [2].z
    pub local_from_world_transpose_a: [Vec4; 2],
    pub local_from_world_transpose_b: f32,
    pub material_bind_group_slot: u32,
    // octree index in the visible nodes texture
    pub octree_index: u32,
    pub spacing: f32,

    // --- Splat Settings Numeric Values ---
    pub point_size: f32,
    pub min_point_size: f32, // Defaults to 0.0 if None
    pub max_point_size: f32, // Defaults to f32::MAX if None
    pub radius: f32,         // Ignored in shader unless SPLAT_RADIUS shader def is set

    // Vectors aligned to Vec4 for std140 WGSL alignment
    pub default_normal: Vec4,
    pub uv_u: Vec4,
    pub uv_v: Vec4,

    // Affine2 packed into 2x Vec4 (Col 0-1 in A, Translation in B)
    // Used in shader only when SPLAT_UV_TRANSFORM shader def is set
    pub uv_transform_a: Vec4,
    pub uv_transform_b: Vec4,
}

impl PointCloudUniform {
    pub fn new(
        aabb: &Aabb,
        model_aabb: &Aabb,
        octree_index: u32,
        spacing: f32,
        mesh_transforms: &PointCloudTransforms,
        material_bind_group_slot: MaterialBindGroupSlot,
        splat_settings: &SplatSettings,
    ) -> Self {
        let (local_from_world_transpose_a, local_from_world_transpose_b) =
            mesh_transforms.world_from_local.inverse_transpose_3x3();

        let material_bind_group_slot = u32::from(material_bind_group_slot);
        debug_assert!(
            material_bind_group_slot <= 0xFFFF,
            "Material bind group slot {material_bind_group_slot} overflowed"
        );

        // Decompose Affine2 into two Vec4s for std140 layout alignment
        let uv_mat = splat_settings.uv_transform.matrix2;
        let uv_trans = splat_settings.uv_transform.translation;

        let uv_transform_a = Vec4::new(
            uv_mat.col(0).x,
            uv_mat.col(0).y,
            uv_mat.col(1).x,
            uv_mat.col(1).y,
        );
        let uv_transform_b = Vec4::new(uv_trans.x, uv_trans.y, 0.0, 0.0);

        Self {
            aabb_min: aabb.min().extend(1.0),
            aabb_max: aabb.max().extend(1.0),
            model_center: model_aabb.center.extend(1.0),
            model_half_extents: model_aabb.half_extents.extend(1.0),
            octree_index,
            spacing,
            world_from_local: mesh_transforms.world_from_local.to_transpose(),
            previous_world_from_local: mesh_transforms.previous_world_from_local.to_transpose(),
            local_from_world_transpose_a,
            local_from_world_transpose_b,
            material_bind_group_slot,

            // Splat properties mapping
            point_size: splat_settings.point_size,
            min_point_size: splat_settings.min_point_size.unwrap_or(0.0),
            max_point_size: splat_settings.max_point_size.unwrap_or(f32::MAX),
            radius: splat_settings.radius.unwrap_or(0.0),

            default_normal: splat_settings.default_normal.extend(0.0),
            uv_u: splat_settings.uv_u.extend(0.0),
            uv_v: splat_settings.uv_v.extend(0.0),

            uv_transform_a,
            uv_transform_b,
        }
    }
}

pub struct PreparedPointCloudUniform {
    pub buffer: UniformBuffer<PointCloudUniform>,
    pub bind_group: BindGroup,
}
