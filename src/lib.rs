use crate::point_cloud::PointCloud;
use crate::point_cloud_material::PointCloudMaterial;
use crate::pointcloud_octree::PointCloudOctreeAssetPlugin;
use crate::potree::PotreePlugin;
use bevy_app::prelude::*;
use bevy_asset::AssetApp;

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
        app.add_plugins(PointCloudOctreeAssetPlugin::default());

        app.register_type::<PointCloud>()
            .init_asset::<PointCloud>()
            .init_asset::<PointCloudMaterial>()
            .register_asset_reflect::<PointCloud>()
            .register_asset_reflect::<PointCloudMaterial>();
        app.add_plugins(render::RenderPipelinePlugin);

        #[cfg(feature = "potree")]
        app.add_plugins(PotreePlugin);
    }
}
