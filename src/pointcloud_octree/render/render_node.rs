use crate::{
    octree::extract::buffer::RenderNodeData,
    pointcloud_octree::asset::data::{PointCloudNodeData, PointData},
};

impl RenderNodeData for PointCloudNodeData {
    type InstanceData = PointData;

    fn instances(&self) -> &[Self::InstanceData] {
        &self.points
    }
}
