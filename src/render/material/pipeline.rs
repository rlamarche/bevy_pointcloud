use std::any::TypeId;

use bevy::{
    ecs::{
        resource::Resource,
        system::{Commands, Res},
    },
    material::key::{ErasedMaterialKey, ErasedMeshPipelineKey},
    pbr::MeshPipelineKey,
};

use crate::{
    render::{
        ErasedSplatPipelineKey, ErasedViewSettingsKey, PointCloudMaterial, PointCloudPipeline,
        SplatPipelineKey,
    },
    ViewSettings,
};

/// A key uniquely identifying a specialized [`MaterialPipeline`].
pub struct PointCloudMaterialPipelineKey<M: PointCloudMaterial> {
    pub mesh_key: MeshPipelineKey,
    pub splat_key: SplatPipelineKey,
    pub bind_group_data: M::Data,
    pub pass: Option<usize>,
    pub view_settings_key: <M::ViewSettings as ViewSettings>::Key,
}

/// Render pipeline data for a given [`Material`].
#[derive(Resource, Clone)]
pub struct PointCloudMaterialPipeline {
    pub pointcloud_pipeline: PointCloudPipeline,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ErasedPointCloudMaterialPipelineKey {
    pub mesh_key: ErasedMeshPipelineKey,
    pub splat_key: ErasedSplatPipelineKey,
    pub material_key: ErasedMaterialKey,
    pub view_settings_key: ErasedViewSettingsKey,
    pub pass: Option<usize>,
    pub type_id: TypeId,
}

pub fn init_point_cloud_material_pipeline(
    mut commands: Commands,
    mesh_pipeline: Res<PointCloudPipeline>,
) {
    commands.insert_resource(PointCloudMaterialPipeline {
        pointcloud_pipeline: mesh_pipeline.clone(),
    });
}
