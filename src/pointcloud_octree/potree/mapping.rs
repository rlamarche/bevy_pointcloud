use crate::octree::asset::NodeId;
use crate::pointcloud_octree::asset::PointCloudOctree;
use bevy_asset::AssetId;
use bevy_ecs::prelude::*;
use bevy_platform::collections::HashMap;
use potree::octree::NodeId as PotreeNodeId;

#[derive(Resource, Default)]
pub struct PotreePointCloudOctreeNodes(
    HashMap<
        AssetId<PointCloudOctree>,
        (HashMap<PotreeNodeId, NodeId>, HashMap<NodeId, PotreeNodeId>),
    >,
);

impl PotreePointCloudOctreeNodes {
    pub fn get(
        &self,
        id: impl Into<AssetId<PointCloudOctree>>,
    ) -> Option<PotreePointCloudOctreeMapping> {
        Some(PotreePointCloudOctreeMapping::from_refs(
            self.0.get(&id.into())?,
        ))
    }

    pub fn get_or_insert(
        &mut self,
        id: impl Into<AssetId<PointCloudOctree>>,
    ) -> PotreePointCloudOctreeMapping {
        let refs = self
            .0
            .entry(id.into())
            .or_insert_with(|| (HashMap::new(), HashMap::new()));
        PotreePointCloudOctreeMapping::from_refs(refs)
    }

    pub fn get_mut(
        &mut self,
        id: impl Into<AssetId<PointCloudOctree>>,
    ) -> Option<PotreePointCloudOctreeMappingMut> {
        Some(PotreePointCloudOctreeMappingMut::from_mutable_refs(
            self.0.get_mut(&id.into())?,
        ))
    }

    pub fn get_or_insert_mut(
        &mut self,
        id: impl Into<AssetId<PointCloudOctree>>,
    ) -> PotreePointCloudOctreeMappingMut {
        let refs = self
            .0
            .entry(id.into())
            .or_insert_with(|| (HashMap::new(), HashMap::new()));
        PotreePointCloudOctreeMappingMut::from_mutable_refs(refs)
    }

    pub fn get_octree_node_id(
        &self,
        id: impl Into<AssetId<PointCloudOctree>>,
        potree_node_id: PotreeNodeId,
    ) -> Option<NodeId> {
        self.0
            .get(&id.into())
            .and_then(|map| map.0.get(&potree_node_id))
            .as_deref()
            .cloned()
    }

    pub fn get_octree_node_id_mut(
        &mut self,
        id: impl Into<AssetId<PointCloudOctree>>,
        potree_node_id: PotreeNodeId,
    ) -> Option<&mut NodeId> {
        self.0
            .get_mut(&id.into())
            .and_then(|map| map.0.get_mut(&potree_node_id))
    }

    pub fn insert(
        &mut self,
        id: impl Into<AssetId<PointCloudOctree>>,
        potree_node_id: PotreeNodeId,
        node_id: NodeId,
    ) -> () {
        let asset_id = id.into();
        match self.0.get_mut(&asset_id) {
            Some(map) => {
                map.0.insert(potree_node_id, node_id);
                map.1.insert(node_id, potree_node_id);
            }
            None => {
                let mut map = (HashMap::default(), HashMap::default());
                map.0.insert(potree_node_id, node_id);
                map.1.insert(node_id, potree_node_id);
                self.0.insert(asset_id, map);
            }
        }
    }

    pub fn remove(
        &mut self,
        id: impl Into<AssetId<PointCloudOctree>>,
        potree_node_id: PotreeNodeId,
    ) -> () {
        match self.0.get_mut(&id.into()) {
            Some(map) => {
                if let Some(node_id) = map.0.remove(&potree_node_id) {
                    map.1.remove(&node_id);
                }
            }
            None => {}
        }
    }
}

#[derive(Debug)]
pub struct PotreePointCloudOctreeMapping<'a>(
    &'a HashMap<PotreeNodeId, NodeId>,
    &'a HashMap<NodeId, PotreeNodeId>,
);

impl<'a> PotreePointCloudOctreeMapping<'a> {
    pub fn from_refs(
        refs: &'a (HashMap<PotreeNodeId, NodeId>, HashMap<NodeId, PotreeNodeId>),
    ) -> Self {
        Self(&refs.0, &refs.1)
    }
    pub fn get_octree_node_id(&self, potree_node_id: PotreeNodeId) -> Option<NodeId> {
        self.0.get(&potree_node_id).as_deref().cloned()
    }
    pub fn get_potree_node_id(&self, node_id: NodeId) -> Option<PotreeNodeId> {
        self.1.get(&node_id).as_deref().cloned()
    }
}

#[derive(Debug)]
pub struct PotreePointCloudOctreeMappingMut<'a>(
    &'a mut HashMap<PotreeNodeId, NodeId>,
    &'a mut HashMap<NodeId, PotreeNodeId>,
);

impl<'a> PotreePointCloudOctreeMappingMut<'a> {
    pub fn from_mutable_refs(
        refs: &'a mut (HashMap<PotreeNodeId, NodeId>, HashMap<NodeId, PotreeNodeId>),
    ) -> Self {
        Self(&mut refs.0, &mut refs.1)
    }
    pub fn get_octree_node_id(&self, potree_node_id: PotreeNodeId) -> Option<NodeId> {
        self.0.get(&potree_node_id).as_deref().cloned()
    }

    pub fn get_potree_node_id(&self, node_id: NodeId) -> Option<PotreeNodeId> {
        self.1.get(&node_id).as_deref().cloned()
    }

    pub fn get_octree_node_id_mut(
        &mut self,
        id: impl Into<AssetId<PointCloudOctree>>,
        potree_node_id: PotreeNodeId,
    ) -> Option<&mut NodeId> {
        self.0.get_mut(&potree_node_id)
    }

    pub fn insert(&mut self, potree_node_id: PotreeNodeId, node_id: NodeId) -> () {
        self.0.insert(potree_node_id, node_id);
        self.1.insert(node_id, potree_node_id);
    }

    pub fn remove_potree_node_id(
        &mut self,
        id: impl Into<AssetId<PointCloudOctree>>,
        potree_node_id: PotreeNodeId,
    ) -> () {
        if let Some(node_id) = self.0.remove(&potree_node_id) {
            self.1.remove(&node_id);
        }
    }

    pub fn remove_node_id(
        &mut self,
        id: impl Into<AssetId<PointCloudOctree>>,
        node_id: NodeId,
    ) -> () {
        if let Some(potree_node_id) = self.1.remove(&node_id) {
            self.0.remove(&potree_node_id);
        }
    }
}
