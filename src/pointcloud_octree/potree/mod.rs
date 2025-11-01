use bevy_app::prelude::*;
use crate::pointcloud_octree::potree::mapping::PotreePointCloudOctreeNodes;

mod asset;
pub mod snapshot;
pub mod mapping;

pub struct PotreeOctreePlugin;

impl Plugin for PotreeOctreePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PotreePointCloudOctreeNodes>();
    }
}
