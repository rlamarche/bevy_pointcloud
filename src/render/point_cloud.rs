use bevy::{
    asset::Handle,
    ecs::component::Component,
    math::{Affine3, Affine3Ext, Vec4},
    mesh::Mesh,
    render::{render_asset::RenderAsset, render_resource::ShaderType},
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
        _asset_id: bevy::asset::AssetId<Self::SourceAsset>,
        _param: &mut bevy::ecs::system::SystemParamItem<Self::Param>,
        _previous_asset: Option<&Self>,
    ) -> Result<Self, bevy::render::render_asset::PrepareAssetError<Self::SourceAsset>> {
        Ok(RenderPointCloudChunk {
            mesh: source_asset.mesh_handle.clone(),
        })
    }
}

#[derive(Component)]
pub struct PointCloudTransforms {
    pub world_from_local: Affine3,
    pub previous_world_from_local: Affine3,
    pub flags: u32,
}

#[derive(ShaderType, Clone)]
pub struct PointCloudUniform {
    // Affine 4x3 matrices transposed to 3x4
    pub world_from_local: [Vec4; 3],
    pub previous_world_from_local: [Vec4; 3],
    // 3x3 matrix packed in mat2x4 and f32 as:
    //   [0].xyz, [1].x,
    //   [1].yz, [2].xy
    //   [2].z
    pub local_from_world_transpose_a: [Vec4; 2],
    pub local_from_world_transpose_b: f32,
    pub flags: u32,
    pub first_vertex_index: u32,
    /// User supplied tag to identify this mesh instance.
    pub tag: u32,
}

impl PointCloudUniform {
    pub fn new(
        mesh_transforms: &PointCloudTransforms,
        first_vertex_index: u32,
        // material_bind_group_slot: MaterialBindGroupSlot,
        tag: Option<u32>,
    ) -> Self {
        let (local_from_world_transpose_a, local_from_world_transpose_b) =
            mesh_transforms.world_from_local.inverse_transpose_3x3();

        // let material_slot = u32::from(material_bind_group_slot);
        // debug_assert!(
        //     material_slot <= 0xFFFF,
        //     "Material bind group slot {material_slot} overflowed"
        // );

        Self {
            world_from_local: mesh_transforms.world_from_local.to_transpose(),
            previous_world_from_local: mesh_transforms.previous_world_from_local.to_transpose(),
            local_from_world_transpose_a,
            local_from_world_transpose_b,
            flags: mesh_transforms.flags,
            first_vertex_index,
            tag: tag.unwrap_or(0),
        }
    }
}
