use bevy_app::prelude::*;

pub mod pointcloud;
pub mod render;

pub struct PointCloudPlugin;

impl Plugin for PointCloudPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(render::RenderPipelinePlugin);
    }
}
