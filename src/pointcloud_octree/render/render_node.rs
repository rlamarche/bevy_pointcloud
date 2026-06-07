use crate::{
    octree::extract::render::buffer::RenderNodeData, point::GpuPoint,
    pointcloud_octree::asset::data::PointCloudNodeData,
};

impl<U: GpuPoint> RenderNodeData for PointCloudNodeData<U> {
    type InstanceData = U;

    fn instances(&self) -> &[Self::InstanceData] {
        &self.points
    }
}
