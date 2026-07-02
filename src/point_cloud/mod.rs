mod node;

use std::sync::Arc;

use bevy::{
    asset::{Asset, AssetId, Handle},
    camera::primitives::Aabb,
    ecs::reflect::ReflectFromWorld,
    mesh::Mesh,
    reflect::{std_traits::ReflectDefault, Reflect},
};

pub use node::*;
use slotmap::{Key, SlotMap};
use thiserror::Error;

#[derive(Asset, Debug, Clone, Default, Reflect)]
#[reflect(Debug, FromWorld, Clone, Default)]
pub struct PointCloud {
    #[reflect(ignore, clone)]
    pub nodes: SlotMap<NodeId, PointCloudNode>,
    pub root: Option<NodeId>,
    pub aabb: Option<Aabb>,
}

impl PointCloud {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_root(&self) -> Option<&PointCloudNode> {
        self.root.and_then(|node_id| self.get_node(node_id))
    }

    /// Inserts a new child node to an existing node
    pub fn insert_node(
        &mut self,
        node: InsertNode,
        // parent_id: Option<NodeId>,
        // child_index: ChildIndex,
        // status: PointCloudNodeStatus,
        // point_count: usize,
        // data: NodeData,
        // aabb: Option<Aabb>,
        // chunk: Option<Handle<PointCloudChunk>>,
    ) -> Result<NodeId, InsertNodeError> {
        let mut depth = None;
        let name;
        if let Some(parent_id) = node.parent_id {
            if !node.child_index.is_child() {
                return Err(InsertNodeError::ChildIndexOutOfBounds);
            }
            // check the parent
            match self.nodes.get(parent_id) {
                None => {
                    return Err(InsertNodeError::ParentNotExists);
                }
                Some(parent) => {
                    if (parent.children_mask.bits() & (1_u8 << *node.child_index)) > 0 {
                        return Err(InsertNodeError::ChildIndexOccupied);
                    }
                    depth = Some(parent.depth + 1);
                    name = Arc::from(format!("{}{}", parent.name, *node.child_index));
                }
            };
        } else {
            if self.root.is_some() {
                return Err(InsertNodeError::RootAlreadyExists);
            }
            name = Arc::from("r");
        }

        // insert the new node
        let id = self.nodes.insert_with_key(|id| PointCloudNode {
            id,
            name,
            status: node.status,
            child_index: node.child_index,
            parent_id: node.parent_id,
            children: [NodeId::null(); 8],
            children_mask: ChildrenMask::empty(),
            aabb: node.aabb,
            depth: depth.unwrap_or_default(),
            data: node.data,
            chunk: node.chunk,
            point_count: node.point_count,
        });

        // update parent children / children_mask
        if let Some(parent_id) = node.parent_id {
            // infallible because we checked upper
            let parent = self.nodes.get_mut(parent_id).unwrap();

            // add to children array and update mask
            parent.children[node.child_index.index() as usize] = id;
            parent.children_mask |= node.child_index.into();
        } else {
            self.root = Some(id);
        }

        Ok(id)
    }

    pub fn get_node(&self, node_id: NodeId) -> Option<&PointCloudNode> {
        self.nodes.get(node_id)
    }

    pub fn get_node_mut(&mut self, node_id: NodeId) -> Option<&mut PointCloudNode> {
        self.nodes.get_mut(node_id)
    }
}

/// A chunk of points
#[derive(Asset, Debug, Clone, Default, Reflect)]
#[reflect(Debug, FromWorld, Clone, Default)]
pub struct PointCloudChunk {
    pub mesh_handle: Option<Handle<Mesh>>,
    pub aabb: Option<Aabb>,
    pub vertex_buffer_size: usize,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct PointCloudNodeKey {
    pub id: AssetId<PointCloud>,
    pub node_id: NodeId,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct PointCloudChunkKey {
    pub id: AssetId<PointCloud>,
    pub chunk_id: AssetId<PointCloudChunk>,
}

#[cfg(test)]
mod tests {
    use bevy::utils::default;

    use super::*;

    #[test]
    fn test_ok() {
        let mut point_cloud = PointCloud::default();
        let root_id = point_cloud
            .insert_node(InsertNode {
                parent_id: None,
                child_index: ChildIndex::ROOT,
                ..default()
            })
            .expect("Unable to insert a root node");

        for child_index in 0..8 {
            point_cloud
                .insert_node(InsertNode {
                    parent_id: Some(root_id),
                    child_index: ChildIndex::try_from(child_index).unwrap(),
                    ..default()
                })
                .expect("Unable to insert a child node");
        }
    }

    #[test]
    fn test_ko_duplicate_root() {
        let mut point_cloud = PointCloud::default();
        let _ = point_cloud
            .insert_node(InsertNode {
                parent_id: None,
                child_index: ChildIndex::ROOT,
                ..default()
            })
            .expect("Unable to insert a root node");

        let result = point_cloud.insert_node(InsertNode {
            parent_id: None,
            child_index: ChildIndex::ROOT,
            ..default()
        });

        assert!(matches!(result, Err(InsertNodeError::RootAlreadyExists)));
    }

    #[test]
    fn test_ko_duplicate_child() {
        let mut point_cloud = PointCloud::default();
        let root_id = point_cloud
            .insert_node(InsertNode {
                parent_id: None,
                child_index: ChildIndex::ROOT,
                ..default()
            })
            .expect("Unable to insert a root node");

        point_cloud
            .insert_node(InsertNode {
                parent_id: Some(root_id),
                child_index: ChildIndex::X_0_Y_0_Z_0,
                ..default()
            })
            .expect("Unable to insert a child node");

        let result = point_cloud.insert_node(InsertNode {
            parent_id: Some(root_id),
            child_index: ChildIndex::X_0_Y_0_Z_0,
            ..default()
        });

        assert!(matches!(result, Err(InsertNodeError::ChildIndexOccupied)));
    }
}

#[derive(Clone, Debug, Default)]
pub struct InsertNode {
    pub parent_id: Option<NodeId>,
    pub child_index: ChildIndex,
    pub status: PointCloudNodeStatus,
    pub point_count: usize,
    pub data: NodeData,
    pub aabb: Option<Aabb>,
    pub chunk: Option<Handle<PointCloudChunk>>,
}

#[derive(Error, Debug, Clone)]
pub enum InsertNodeError {
    #[error("parent does not exists")]
    ParentNotExists,
    #[error("root already exists")]
    RootAlreadyExists,
    #[error("child index already occupied")]
    ChildIndexOccupied,
    #[error("child index is out of bounds")]
    ChildIndexOutOfBounds,
    #[error("node not found")]
    NodeNotFound,
}
