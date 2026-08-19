use bevy::{camera::primitives::MeshAabb, mesh::Mesh, reflect::TypePath};
use thiserror::Error;

use crate::{
    ChunkLoadResult, InsertNodeParams, OctreeError, OctreeHierarchyBuilder, OctreeLoader,
    PointCloudNodeStatus,
};

/// An error that occurs when loading a glTF file.
#[derive(Error, Debug)]
pub enum PointCloudMeshLoaderError {
    #[error("octree topology error: {0}")]
    Octree(#[from] OctreeError),
}

#[derive(TypePath)]
pub struct PointCloudMeshLoader {
    mesh: Mesh,
}

impl From<Mesh> for PointCloudMeshLoader {
    fn from(mesh: Mesh) -> Self {
        Self { mesh }
    }
}

impl OctreeLoader for PointCloudMeshLoader {
    type Source = Mesh;
    type Hierarchy = ();
    type Error = PointCloudMeshLoaderError;
    type Settings = ();

    async fn from_source(
        source: Self::Source,
        _settings: Self::Settings,
    ) -> Result<Self, Self::Error> {
        Ok(Self::from(source))
    }

    async fn load_initial_hierarchy(
        &self,
        builder: &mut OctreeHierarchyBuilder<Self::Hierarchy>,
    ) -> Result<(), Self::Error> {
        let point_count = self.mesh.count_vertices();
        let aabb = self.mesh.compute_aabb();

        builder.insert_root(
            InsertNodeParams {
                status: PointCloudNodeStatus::Loaded,
                point_count,
                aabb,
            },
            (),
        )?;

        Ok(())
    }

    async fn load_chunk(&self, _: &Self::Hierarchy) -> Result<ChunkLoadResult, Self::Error> {
        Ok(ChunkLoadResult {
            mesh: Some(self.mesh.clone()),
            offset: None,
            final_point_count: self.mesh.count_vertices(),
        })
    }
}
