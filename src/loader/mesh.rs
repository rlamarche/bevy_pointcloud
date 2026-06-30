use bevy::{camera::primitives::MeshAabb, mesh::Mesh, reflect::TypePath};
use thiserror::Error;

use crate::{ChildIndex, LoadedPointCloudNode, PointCloudLoader, PointCloudNodeStatus};

/// An error that occurs when loading a glTF file.
#[derive(Error, Debug)]
pub enum PointCloudMeshLoaderError {}

#[derive(TypePath)]
pub struct PointCloudMeshLoader {
    mesh: Mesh,
}

impl From<Mesh> for PointCloudMeshLoader {
    fn from(mesh: Mesh) -> Self {
        Self { mesh }
    }
}

impl PointCloudLoader for PointCloudMeshLoader {
    type Source = Mesh;
    type Hierarchy = ();
    type Error = PointCloudMeshLoaderError;

    async fn from_source(source: Self::Source) -> Result<Self, Self::Error> {
        Ok(Self::from(source))
    }

    async fn load_initial_hierarchy(
        &self,
    ) -> Result<Vec<LoadedPointCloudNode<Self::Hierarchy>>, Self::Error> {
        let point_count = self.mesh.count_vertices();
        let aabb = self.mesh.compute_aabb();

        Ok(vec![LoadedPointCloudNode {
            status: PointCloudNodeStatus::Loaded,
            child_index: ChildIndex::ROOT,
            parent_index: None,
            aabb,
            data: (),
            point_count,
        }])
    }

    async fn load_chunk(&self, _: &Self::Hierarchy) -> Result<Mesh, Self::Error> {
        Ok(self.mesh.clone())
    }
}
