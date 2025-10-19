use crate::octree::extract::{
    RenderAssetBytesPerFrame, RenderAssetBytesPerFrameLimiter,
    extract_render_asset_bytes_per_frame, reset_render_asset_bytes_per_frame,
};
use crate::point_cloud::PointCloud;
use crate::point_cloud_material::PointCloudMaterial;
use crate::pointcloud_octree::PointCloudOctreePlugin;
use crate::potree::PotreePlugin;
use bevy_app::prelude::*;
use bevy_asset::AssetApp;
use bevy_ecs::prelude::*;
use bevy_render::{ExtractSchedule, Render, RenderApp, RenderSystems};

pub mod loader;
pub mod octree;
pub mod point_cloud;
pub mod point_cloud_material;
pub mod pointcloud_octree;
pub mod potree;
pub mod prelude;
pub mod render;

pub struct PointCloudPlugin;

impl Plugin for PointCloudPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(PointCloudOctreePlugin);

        app.register_type::<PointCloud>()
            .init_asset::<PointCloud>()
            .init_asset::<PointCloudMaterial>()
            .register_asset_reflect::<PointCloud>()
            .register_asset_reflect::<PointCloudMaterial>();
        app.add_plugins(render::RenderPipelinePlugin);

        #[cfg(feature = "potree")]
        app.add_plugins(PotreePlugin);

        app.init_resource::<RenderAssetBytesPerFrame>();
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<RenderAssetBytesPerFrameLimiter>();
            render_app
                .add_systems(ExtractSchedule, extract_render_asset_bytes_per_frame)
                .add_systems(
                    Render,
                    reset_render_asset_bytes_per_frame.in_set(RenderSystems::Cleanup),
                );

            // render_app.add_systems(RenderStartup, init_empty_bind_group_layout);
        }
    }
}
