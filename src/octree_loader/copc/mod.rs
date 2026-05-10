mod classification;
mod density;
#[cfg(feature = "copc_ehttp")]
mod http_source;

use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use async_lock::RwLock;
use async_trait::async_trait;
use bevy_camera::primitives::Aabb;
use bevy_ecs::error::BevyError;
use bevy_math::{DVec3, Vec4};
use bevy_reflect::TypePath;
use copc_streaming::{ByteSource, CopcStreamingReader, HierarchyEntry, VoxelKey};
#[cfg(feature = "copc_ehttp")]
pub use http_source::*;

use crate::{
    octree::{
        hierarchy::HierarchyNodeStatus,
        loader::{LoadedHierarchyNode, OctreeLoader},
    },
    octree_loader::copc::{classification::classification_to_color, density::compute_density},
    pointcloud_octree::asset::data::{PointCloudNodeData, PointData},
};

pub struct CopcLoader<S: ByteSource> {
    pub(crate) reader: Arc<RwLock<CopcStreamingReader<S>>>,
}

#[derive(Clone, TypePath)]
pub struct CopcHierarchy(HierarchyEntry);

#[async_trait]
impl<S: ByteSource + 'static + Send + Sync> OctreeLoader<PointCloudNodeData> for CopcLoader<S> {
    type Source = S;
    type Hierarchy = CopcHierarchy;
    type Error = BevyError;

    async fn from_source(
        source: Self::Source,
    ) -> Result<Self, <Self as OctreeLoader<PointCloudNodeData>>::Error> {
        let reader = CopcStreamingReader::open(source)
            .await
            .map_err(|err| err.to_string())?;

        Ok(CopcLoader {
            reader: Arc::new(RwLock::new(reader)),
        })
    }

    /// Loads the initial hierarchy by converting hierarchy cache to a vec
    async fn load_initial_hierarchy(
        &self,
    ) -> Result<Vec<LoadedHierarchyNode<Self::Hierarchy>>, Self::Error> {
        let mut reader = self.reader.write().await;
        reader
            .load_all_hierarchy()
            .await
            .map_err(|err| err.to_string())?;

        let copc_info = reader.copc_info();
        let aabb = copc_info.root_bounds();

        let mut initial_hierarchy = Vec::new();

        // contains a mapping between voxel key and index in output vec
        let mut parent_indexes = HashMap::<VoxelKey, usize>::new();

        // stack of the nodes to process
        let mut stack = VecDeque::<(&HierarchyEntry, Option<VoxelKey>)>::new();
        let root_hierarchy = reader
            .get(&VoxelKey {
                level: 0,
                x: 0,
                y: 0,
                z: 0,
            })
            .ok_or_else(|| "Root node is missing in the copc hierarchy.".to_string())?;

        // initialize the stack with the root node
        stack.push_back((root_hierarchy, None));

        // iterate recursively in hierarchy tree to gather hierarchy nodes
        while let Some((hierarchy_entry, parent_key)) = stack.pop_front() {
            // add root node to index
            parent_indexes.insert(hierarchy_entry.key, initial_hierarchy.len());

            // add root node to hierarchy vec
            initial_hierarchy.push(LoadedHierarchyNode {
                status: HierarchyNodeStatus::Loaded,
                child_index: hierarchy_entry.key.child_index(),
                // retrieve the parent id in the map
                parent_id: parent_key
                    .map(|key| {
                        parent_indexes.get(&key).copied().ok_or_else(|| {
                            format!("Voxel key {:?} is missing in parent indexes", key).to_string()
                        })
                    })
                    .transpose()?,
                bounding_box: copc_aabb_to_aabb(hierarchy_entry.key.bounds(&aabb)),
                data: CopcHierarchy(hierarchy_entry.clone()),
            });

            // load children
            let children = reader.children(&hierarchy_entry.key);

            // append children to stack
            for child in children {
                stack.push_back((child, Some(hierarchy_entry.key)));
            }
        }

        Ok(initial_hierarchy)
    }

    async fn load_hierarchy(
        &self,
        _node: &LoadedHierarchyNode<Self::Hierarchy>,
    ) -> Result<Vec<LoadedHierarchyNode<Self::Hierarchy>>, Self::Error> {
        Ok(Vec::new())
    }

    async fn load_node_data(
        &self,
        node: &LoadedHierarchyNode<Self::Hierarchy>,
    ) -> Result<PointCloudNodeData, Self::Error> {
        let key = node.data.0.key;
        let reader = self.reader.read().await;

        let chunk = reader
            .fetch_chunk(&key)
            .await
            .map_err(|err| err.to_string())?;

        let spacing = reader.copc_info().spacing;

        let aabb = node.data.0.key.bounds(&reader.copc_info().root_bounds());

        let points = reader.read_points(&chunk)?;

        drop(reader);

        let level = node.data.0.key.level as f64;
        let density = compute_density(&points, &aabb);

        let spacing = (spacing / level.exp2()) as f32;

        // magic formula from Potree
        let offset = (density as f32).log2() / 2.0 - 1.5;

        Ok(PointCloudNodeData {
            spacing,
            level: node.data.0.key.level as u32,
            offset,
            num_points: node.data.0.point_count as usize,
            points: Arc::new(
                points
                    .into_iter()
                    .map(|point| {
                        let position =
                            Vec4::new(point.x as f32, point.y as f32, point.z as f32, 1.0);
                        let color = compute_point_color(&point);
                        PointData { position, color }
                    })
                    .collect(),
            ),
        })
    }
}

/// Compute the point color from the point
/// Returns the color available, else compute a color based on the classification
fn compute_point_color(value: &copc_streaming::Point) -> Vec4 {
    if let Some(color) = value.color {
        return Vec4::new(
            color.red as f32 / 65535.0,
            color.blue as f32 / 65535.0,
            color.green as f32 / 65535.0,
            1.0,
        );
    }

    // let intensity = value.intensity;

    // Vec4::new(
    //     intensity as f32 / 65535.0,
    //     intensity as f32 / 256.0,
    //     intensity as f32 / 256.0,
    //     1.0,
    // )

    let color = classification_to_color(&value.classification);

    Vec4::new(
        color.0 as f32 / 256.0,
        color.1 as f32 / 256.0,
        color.2 as f32 / 256.0,
        1.0,
    )
}

/// Swap the child index between potree and copc (LSB / MSB)
#[allow(unused)]
fn swap_child_index(child_index: u8) -> u8 {
    let mut swaped_index = 0;

    if child_index & 0b100 != 0 {
        swaped_index |= 0b001;
    } // X: bit 2 -> bit 0
    if child_index & 0b010 != 0 {
        swaped_index |= 0b010;
    } // Y: bit 1 -> bit 1
    if child_index & 0b001 != 0 {
        swaped_index |= 0b100;
    } // Z: bit 0 -> bit 2

    swaped_index
}

pub trait VoxelKeyExt {
    /// Computes the potree based child index
    fn child_index(&self) -> u8;
}

impl VoxelKeyExt for VoxelKey {
    fn child_index(&self) -> u8 {
        let x = self.x & 1;
        let y = self.y & 1;
        let z = self.z & 1;

        (x * 4 + y * 2 + z) as u8
    }
}

fn copc_aabb_to_aabb(value: copc_streaming::Aabb) -> Aabb {
    let min = DVec3::from_array(value.min);
    let max = DVec3::from_array(value.max);

    Aabb::from_min_max(min.as_vec3(), max.as_vec3())
}
