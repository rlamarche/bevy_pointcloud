mod aabb;
pub mod attribute_pass;
pub mod depth_pass;
mod draw;
mod extract;
mod eye_dome_lighting;
pub mod material;
pub mod mesh;
pub mod normalize_pass;
pub mod phase;
pub mod point_cloud;
pub mod point_cloud_uniform;

use aabb::compute_point_cloud_aabb;
use attribute_pass::AttributePassPlugin;
use bevy_app::prelude::*;
use bevy_asset::{load_internal_asset, prelude::*, uuid_handle};
use bevy_camera::visibility::calculate_bounds;
use bevy_ecs::prelude::*;
use bevy_render::{
    camera::extract_cameras,
    extract_component::{ExtractComponentPlugin, UniformComponentPlugin},
    prelude::*,
    render_asset::RenderAssetPlugin,
    Render, RenderApp, RenderSystems,
};
use bevy_shader::Shader;
use depth_pass::DepthPassPlugin;
use normalize_pass::NormalizePassPlugin;
use point_cloud_uniform::{prepare_point_cloud_uniform, PointCloudUniformLayout};

use crate::{
    point_cloud::PointCloud3d,
    render::{
        eye_dome_lighting::{extract_cameras_render_mode, EyeDomeLightingUniform, NeighboursCache},
        material::{RenderPointCloudMaterial, RenderPointCloudMaterialLayout},
        mesh::PointCloudMesh,
        point_cloud::RenderPointCloud,
    },
};

const POINTCLOUD_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("9c7d8df3-86dd-4412-a9cc-dad5c7916a8c");

const NORMALIZE_SHADER_HANDLE: Handle<Shader> =
    uuid_handle!("0e5fffec-7e0b-4b44-8c32-b92d9b99fd58");

pub struct RenderPipelinePlugin;

impl Plugin for RenderPipelinePlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            POINTCLOUD_SHADER_HANDLE,
            "point_cloud.wgsl",
            Shader::from_wgsl
        );

        load_internal_asset!(
            app,
            NORMALIZE_SHADER_HANDLE,
            "normalize.wgsl",
            Shader::from_wgsl
        );

        // Automatically create uniform from these settings
        app.add_plugins(RenderAssetPlugin::<RenderPointCloud>::default())
            .add_plugins(RenderAssetPlugin::<RenderPointCloudMaterial>::default())
            .add_plugins(ExtractComponentPlugin::<PointCloud3d>::default())
            .add_plugins(UniformComponentPlugin::<EyeDomeLightingUniform>::default())
            // compute point cloud aabb **before** [`bevy_render::view::calculate_bounds`] to prevent using mesh's aabb.
            .add_systems(
                PostUpdate,
                compute_point_cloud_aabb.before(calculate_bounds),
            )
            .sub_app_mut(RenderApp)
            .add_systems(
                Render,
                prepare_point_cloud_uniform.in_set(RenderSystems::PrepareResources),
            );

        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .insert_resource(NeighboursCache::default())
            .add_systems(
                ExtractSchedule,
                extract_cameras_render_mode.after(extract_cameras),
            );

        app.add_plugins((DepthPassPlugin, AttributePassPlugin, NormalizePassPlugin));
    }

    fn finish(&self, app: &mut App) {
        app.sub_app_mut(RenderApp)
            .init_resource::<RenderPointCloudMaterialLayout>()
            .init_resource::<PointCloudUniformLayout>()
            .init_resource::<PointCloudMesh>();
    }
}

#[derive(Clone, Debug, Component)]
pub struct PointCloudRenderMode {
    pub use_edl: bool,
    pub edl_neighbour_count: u32,
    pub edl_strength: f32,
    pub edl_radius: f32,
}
pub trait PointCloudRenderModeOpt {
    fn use_edl(&self) -> bool;
    fn edl_neighbour_count(&self) -> u32;
}

impl Default for PointCloudRenderMode {
    fn default() -> Self {
        Self {
            use_edl: true,
            edl_neighbour_count: 4,
            edl_strength: 0.4,
            edl_radius: 1.4,
        }
    }
}

impl PointCloudRenderModeOpt for Option<&PointCloudRenderMode> {
    fn use_edl(&self) -> bool {
        match self {
            None => false,
            Some(point_cloud_render_mode) => point_cloud_render_mode.use_edl,
        }
    }

    fn edl_neighbour_count(&self) -> u32 {
        match self {
            None => 0,
            Some(point_cloud_render_mode) => point_cloud_render_mode.edl_neighbour_count,
        }
    }
}
