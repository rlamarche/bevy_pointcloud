use bevy::{
    asset::{AssetId, Handle},
    camera::{primitives::Aabb, visibility::RenderLayers},
    ecs::{component::Component, resource::Resource},
    math::{Affine3, Affine3Ext, Vec4},
    mesh::Mesh,
    pbr::MaterialBindGroupSlot,
    prelude::{Deref, DerefMut},
    render::{
        render_asset::RenderAsset,
        render_resource::{BindGroup, ShaderType, UniformBuffer},
        sync_world::{MainEntity, MainEntityHashMap},
    },
};

use crate::PointCloudChunk;

#[derive(Debug, Clone)]
pub struct RenderPointCloudChunk {
    pub mesh: Option<Handle<Mesh>>,
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
    pub aabb: Aabb,
    pub spacing: Option<f32>,
    /// The transform of the mesh.
    ///
    /// This will be written into the [`MeshUniform`] at the appropriate time.
    pub transforms: PointCloudTransforms,
    /// The set of render layers that this mesh belongs to.
    pub render_layers: Option<RenderLayers>,
    // pub chunks: MainEntityHashMap<RenderPointCloudChunkInstance>,
}

/// Information that the render world keeps about each entity that contains a
/// mesh.
///
/// The set of information needed is different depending on whether CPU or GPU
/// [`MeshUniform`] building is in use.
#[derive(Resource, Default, Deref, DerefMut)]
pub struct RenderPointCloudChunkInstances(MainEntityHashMap<RenderPointCloudChunkInstance>);

/// CPU data that the render world keeps for each entity, when *not* using GPU
/// mesh uniform building.
pub struct RenderPointCloudChunkInstance {
    /// The entity that point to the root chunk entity, used for loading the corresponding
    /// [`PreparedPointCloudUniform`] in the draw command [`crate::SetPointCloudUniformGroup`].
    pub root_entity: MainEntity,
    pub is_root: bool,
}

#[derive(Component)]
pub struct PointCloudTransforms {
    pub world_from_local: Affine3,
    pub previous_world_from_local: Affine3,
}

#[derive(ShaderType, Clone)]
pub struct PointCloudUniform {
    pub aabb_min: Vec4,
    pub aabb_max: Vec4,
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
    pub spacing: f32,
}

impl PointCloudUniform {
    pub fn new(
        aabb: &Aabb,
        spacing: f32,
        mesh_transforms: &PointCloudTransforms,
        material_bind_group_slot: MaterialBindGroupSlot,
    ) -> Self {
        let (local_from_world_transpose_a, local_from_world_transpose_b) =
            mesh_transforms.world_from_local.inverse_transpose_3x3();

        let material_bind_group_slot = u32::from(material_bind_group_slot);
        debug_assert!(
            material_bind_group_slot <= 0xFFFF,
            "Material bind group slot {material_bind_group_slot} overflowed"
        );

        Self {
            aabb_min: aabb.min().extend(1.0),
            aabb_max: aabb.max().extend(1.0),
            spacing,
            world_from_local: mesh_transforms.world_from_local.to_transpose(),
            previous_world_from_local: mesh_transforms.previous_world_from_local.to_transpose(),
            local_from_world_transpose_a,
            local_from_world_transpose_b,
            material_bind_group_slot,
        }
    }
}

pub struct PreparedPointCloudUniform {
    pub buffer: UniformBuffer<PointCloudUniform>,
    pub bind_group: BindGroup,
}
