use crate::point_cloud::{PointCloud, PointCloud3d};
use crate::point_cloud_material::PointCloudMaterial;
use bevy_app::prelude::*;
use bevy_asset::AssetApp;
use bevy_camera::visibility::{add_visibility_class, Visibility, VisibilityClass};
use bevy_ecs::prelude::*;

pub mod loader;
pub mod octree;
pub mod point_cloud;
pub mod point_cloud_material;
pub mod pointcloud_octree;
pub mod prelude;
pub mod render;
#[cfg(feature = "potree")]
pub mod potree;

pub struct PointCloudPlugin;

impl Plugin for PointCloudPlugin {
    fn build(&self, app: &mut App) {
        app.register_required_components::<PointCloud3d, Visibility>()
            .register_required_components::<PointCloud3d, VisibilityClass>();

        app.register_type::<PointCloud>()
            .init_asset::<PointCloud>()
            .init_asset::<PointCloudMaterial>()
            .register_asset_reflect::<PointCloud>()
            .register_asset_reflect::<PointCloudMaterial>();
        app.add_plugins(render::RenderPipelinePlugin);

        app.world_mut()
            .register_component_hooks::<PointCloud3d>()
            .on_add(add_visibility_class::<PointCloud3d>);
    }
}
