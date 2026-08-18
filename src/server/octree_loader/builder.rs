use std::{marker::PhantomData, sync::Arc};

use bevy::camera::primitives::Aabb;
use slotmap::SlotMap;

use crate::{
    impl_node_id_wrapper, ChildIndex, ChildrenMask, NodeData, OctreeError, PointCloudNodeStatus,
};

impl_node_id_wrapper!(BuilderNodeId);

#[derive(Clone, Debug)]
pub struct UncommittedOctreeNode<T> {
    pub id: BuilderNodeId,
    pub name: Arc<str>,
    pub status: PointCloudNodeStatus,
    pub point_count: usize,
    pub child_index: ChildIndex,
    pub parent_id: Option<BuilderNodeId>,
    pub children: [BuilderNodeId; 8],
    pub children_mask: ChildrenMask,
    pub aabb: Option<Aabb>,
    pub depth: u32,
    pub data: Arc<T>,
}

#[derive(Clone, Debug, Default)]
pub struct ErasedUncommittedOctreeNode {
    pub id: BuilderNodeId,
    pub name: Arc<str>,
    pub status: PointCloudNodeStatus,
    pub point_count: usize,
    pub child_index: ChildIndex,
    pub parent_id: Option<BuilderNodeId>,
    pub children: [BuilderNodeId; 8],
    pub children_mask: ChildrenMask,
    pub aabb: Option<Aabb>,
    pub depth: u32,
    pub data: NodeData,
}

impl<T> From<&ErasedUncommittedOctreeNode> for UncommittedOctreeNode<T>
where
    T: Send + Sync + 'static,
{
    fn from(value: &ErasedUncommittedOctreeNode) -> Self {
        Self {
            id: value.id,
            name: value.name.clone(),
            status: value.status,
            point_count: value.point_count,
            child_index: value.child_index,
            parent_id: value.parent_id,
            children: value.children,
            children_mask: value.children_mask,
            aabb: value.aabb,
            depth: value.depth,
            data: value.data.0.clone().downcast().unwrap(),
        }
    }
}

/// A temporary builder used by workers and loaders to assemble sub-hierarchies
/// off the main thread.
#[derive(Debug, Clone)]
pub struct OctreeHierarchyBuilder<T> {
    nodes: SlotMap<BuilderNodeId, ErasedUncommittedOctreeNode>,
    root: Option<BuilderNodeId>,
    _phantom: PhantomData<T>,
}

impl<T> Default for OctreeHierarchyBuilder<T> {
    fn default() -> Self {
        Self {
            nodes: SlotMap::default(),
            root: None,
            _phantom: PhantomData,
        }
    }
}

impl<T> OctreeHierarchyBuilder<T>
where
    T: Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn root_id(&self) -> Option<BuilderNodeId> {
        self.root
    }

    pub fn get_root(&self) -> Option<&ErasedUncommittedOctreeNode> {
        self.nodes.get(self.root?)
    }

    pub fn get_node(&self, id: BuilderNodeId) -> Option<&ErasedUncommittedOctreeNode> {
        self.nodes.get(id)
    }

    /// Consumes the builder and returns an iterator over its nodes for main-thread merging.
    pub fn drain(self) -> impl Iterator<Item = (BuilderNodeId, ErasedUncommittedOctreeNode)> {
        self.nodes.into_iter()
    }

    /// Inserts the root node for this hierarchy builder.
    pub fn try_insert_root(
        &mut self,
        status: PointCloudNodeStatus,
        point_count: usize,
        data: T,
        aabb: Option<Aabb>,
    ) -> Result<BuilderNodeId, OctreeError> {
        if self.root.is_some() {
            return Err(OctreeError::RootAlreadyExists);
        }

        let name: Arc<str> = Arc::from("r");

        let id = self
            .nodes
            .insert_with_key(|id| ErasedUncommittedOctreeNode {
                id,
                name,
                status,
                point_count,
                child_index: ChildIndex::ROOT,
                parent_id: None,
                children: [BuilderNodeId::null(); 8],
                children_mask: ChildrenMask::empty(),
                aabb,
                depth: 0,
                data: NodeData::new(data),
            });

        self.root = Some(id);
        Ok(id)
    }

    /// Inserts a child node under an existing parent.
    pub fn try_insert_child(
        &mut self,
        parent_id: BuilderNodeId,
        child_index: ChildIndex,
        status: PointCloudNodeStatus,
        point_count: usize,
        data: T,
        aabb: Option<Aabb>,
    ) -> Result<BuilderNodeId, OctreeError> {
        // Check parent
        let (parent_depth, new_name) = {
            let parent = self
                .nodes
                .get(parent_id)
                .ok_or(OctreeError::ParentNotExists)?;

            if parent.children_mask.has_child(child_index) {
                return Err(OctreeError::ChildIndexOccupied);
            }

            let new_name: Arc<str> = Arc::from(format!("{}{}", parent.name, child_index.index()));
            (parent.depth, new_name)
        };

        // Insert the child
        let child_id = self
            .nodes
            .insert_with_key(|id| ErasedUncommittedOctreeNode {
                id,
                name: new_name,
                status,
                point_count,
                child_index,
                parent_id: Some(parent_id),
                children: [BuilderNodeId::null(); 8],
                children_mask: ChildrenMask::empty(),
                aabb,
                depth: parent_depth + 1,
                data: NodeData::new(data),
            });

        // Update parent relationships
        // Infallible because we checked existence earlier, but we need a mutable borrow now.
        let parent = self.nodes.get_mut(parent_id).unwrap();
        parent.children[child_index.index() as usize] = child_id;
        parent.children_mask |= child_index.into();

        Ok(child_id)
    }
}

impl<T> From<OctreeHierarchyBuilder<T>> for ErasedOctreeHierarchy {
    fn from(value: OctreeHierarchyBuilder<T>) -> Self {
        ErasedOctreeHierarchy {
            nodes: value.nodes,
            root: value.root,
        }
    }
}

/// A temporary builder used by workers and loaders to assemble sub-hierarchies
/// off the main thread.
#[derive(Debug, Clone, Default)]
pub struct ErasedOctreeHierarchy {
    nodes: SlotMap<BuilderNodeId, ErasedUncommittedOctreeNode>,
    root: Option<BuilderNodeId>,
}

impl ErasedOctreeHierarchy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn root_id(&self) -> Option<BuilderNodeId> {
        self.root
    }

    pub fn get_root(&self) -> Option<&ErasedUncommittedOctreeNode> {
        self.nodes.get(self.root?)
    }

    pub fn get_node(&self, id: BuilderNodeId) -> Option<&ErasedUncommittedOctreeNode> {
        self.nodes.get(id)
    }

    /// Consumes the builder and returns an iterator over its nodes for main-thread merging.
    pub fn drain(self) -> impl Iterator<Item = (BuilderNodeId, ErasedUncommittedOctreeNode)> {
        self.nodes.into_iter()
    }
}
