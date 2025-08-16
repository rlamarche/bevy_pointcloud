use crate::point_cloud::PointCloud;
use crate::point_cloud_material::PointCloudMaterial;
use bevy_app::prelude::*;
use bevy_asset::AssetApp;

pub mod loader;
pub mod point_cloud;
pub mod point_cloud_material;
pub mod render;

pub struct PointCloudPlugin;

impl Plugin for PointCloudPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<PointCloud>()
            .init_asset::<PointCloud>()
            .init_asset::<PointCloudMaterial>()
            .register_asset_reflect::<PointCloud>()
            .register_asset_reflect::<PointCloudMaterial>();
        app.add_plugins(render::RenderPipelinePlugin);
    }
}
