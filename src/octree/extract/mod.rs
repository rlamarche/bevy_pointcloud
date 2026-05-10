pub mod allocate;
pub mod eviction;
pub mod limiter;
pub mod render;
pub mod resources;

use std::marker::PhantomData;

use allocate::allocate_visible_octree_nodes;
use bevy_app::prelude::*;
use bevy_asset::AssetId;
use bevy_ecs::{prelude::*, schedule::ScheduleConfigs, system::ScheduleSystem};
use bevy_reflect::TypePath;
use bevy_render::{
    camera::extract_cameras,
    extract_component::{ExtractComponent, ExtractComponentPlugin},
    view::ExtractedView,
    ExtractSchedule, Render, RenderApp, RenderSystems,
};
use eviction::update_extract_octree_node_eviction_queue;
use limiter::{
    extract_render_asset_bytes_per_frame, reset_render_asset_bytes_per_frame,
    RenderOctreeNodesBytesPerFrame, RenderOctreeNodesBytesPerFrameLimiter,
};
use render::{
    buffer::{RenderNodeData, RenderOctreesBuffers},
    extract::{extract_octree_node_allocations, extract_visible_octree_nodes},
    node::RenderOctreeNode,
    prepare::prepare_assets,
    resources::{
        ExtractedOctreeNodes, PrepareNextFrameOctreeNodes, RenderOctreeIndex, RenderOctrees,
    },
};
use resources::{ExtractOctreeNodeEvictionQueue, OctreeBufferSettings, OctreeNodeAllocations};

use super::{
    asset::Octree,
    node::{NodeData, OctreeNode},
};
use crate::octree::{
    extract::{
        allocate::on_remove_octree,
        render::{
            components::RenderVisibleOctreeNodes,
            extract::{clear_removed_octrees, extract_removed_octrees},
            prepare::prepare_octrees_uniforms,
            resources::AllocatedOctreeNodes,
        },
    },
    visibility::OctreeVisibilitySystems,
};

pub trait OctreeNodeExtraction: Send + Sync + TypePath {
    type NodeData: NodeData;

    type Component: Component + ExtractComponent;

    type ExtractedNodeData: RenderNodeData;

    /// Defines how the component is transferred into the "render world".
    fn extract_octree_node(node: &OctreeNode<Self::NodeData>) -> Option<Self::ExtractedNodeData>;
}

/// This plugin extracts visible octree nodes from the "app world" into the "render world"
/// and prepares them for the GPU. They can be accessed from the [`RenderVisibleOctreeNodes`] resource.
///
/// The [`OctreeNodeExtraction::NodeData`] generic parameter refers to the type of octree node we are talking about.
/// Because an asset is an octree, we need a component to reference it.
/// This is the role of [`OctreeNodeExtraction::Component`] generic parameter, which is the component used to find the referring octree asset.
/// So, the [`OctreeNodeExtraction::Component`] generic parameter has to implement `Into<AssetId<Octree<T>>`.
///
/// The [`OctreeNodeExtraction::ExtractedNodeData`] generic parameter represents the octree node viewed by the gpu.
/// It has to implement the [`RenderOctreeNode`] trait to determine how octree nodes are converted in gpu format.
///
/// The `AFTER` generic parameter can be used to specify that [`RenderOctreeNode::prepare_octree_node`] should not be run until
/// `prepare_assets::<AFTER>` has completed. This allows the [`RenderOctreeNode::prepare_octree_node`] function to depend on another
/// prepared [`RenderOctreeNode`].
#[allow(clippy::type_complexity)]
pub struct ExtractVisibleOctreeNodesPlugin<E, A, AFTER = ()> {
    max_size: usize,
    _phantom: PhantomData<fn() -> (E, A, AFTER)>,
}

impl<E, A, AFTER> Default for ExtractVisibleOctreeNodesPlugin<E, A, AFTER> {
    fn default() -> Self {
        ExtractVisibleOctreeNodesPlugin {
            max_size: 512 * 1024 * 1024, // 512 mb
            _phantom: PhantomData,
        }
    }
}

impl<E, A, AFTER> ExtractVisibleOctreeNodesPlugin<E, A, AFTER> {
    /// Construct with specific max memory size for GPU
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            max_size,
            _phantom: PhantomData,
        }
    }
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExtractOctreeNode;

impl<E, A, AFTER> Plugin for ExtractVisibleOctreeNodesPlugin<E, A, AFTER>
where
    E: OctreeNodeExtraction,
    for<'a> &'a E::Component: Into<AssetId<Octree<E::NodeData>>>,
    A: RenderOctreeNode<SourceOctreeNode = E::NodeData, ExtractedOctreeNode = E::ExtractedNodeData>,
    AFTER: RenderOctreeDependency + 'static,
    A::ExtractedOctreeNode: RenderNodeData,
{
    fn build(&self, app: &mut App) {
        app.insert_resource(OctreeBufferSettings::<E> {
            max_size: self.max_size,
            _phantom: PhantomData,
        })
        .init_resource::<OctreeNodeAllocations<E>>()
        .init_resource::<ExtractOctreeNodeEvictionQueue<E>>()
        .add_plugins(ExtractComponentPlugin::<E::Component>::default())
        .init_resource::<RenderOctreeNodesBytesPerFrame>()
        .add_systems(
            PostUpdate,
            (
                update_extract_octree_node_eviction_queue::<E>,
                allocate_visible_octree_nodes::<E>
                    .after(update_extract_octree_node_eviction_queue::<E>),
            )
                .in_set(ExtractOctreeNode),
        )
        .configure_sets(
            PostUpdate,
            ExtractOctreeNode.after(OctreeVisibilitySystems::CheckOctreeNodesVisibility),
        )
        .add_observer(on_remove_octree::<E>);

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .world_mut()
            .register_required_components::<ExtractedView, RenderVisibleOctreeNodes::<E::NodeData, E::Component>>();

        render_app
            .init_resource::<RenderOctreeNodesBytesPerFrameLimiter>()
            .add_systems(ExtractSchedule, extract_render_asset_bytes_per_frame)
            .add_systems(
                Render,
                reset_render_asset_bytes_per_frame.in_set(RenderSystems::Cleanup),
            )
            .init_resource::<ExtractedOctreeNodes<E>>()
            .init_resource::<AllocatedOctreeNodes<E>>()
            .init_resource::<RenderOctrees<A>>()
            .init_resource::<RenderOctreesBuffers<A>>()
            .init_resource::<PrepareNextFrameOctreeNodes<A>>()
            .init_resource::<RenderOctreeIndex<E::Component>>()
            // Add in [`First`] schedule because it has to run just after the [`ExtractSchedule`] before any observer
            .add_systems(
                ExtractSchedule,
                (
                    extract_visible_octree_nodes::<E, A>.after(extract_cameras),
                    extract_octree_node_allocations::<E>,
                    extract_removed_octrees::<E>,
                    clear_removed_octrees::<E>.after(extract_removed_octrees::<E>),
                ),
            )
            .add_systems(
                Render,
                prepare_octrees_uniforms::<E>.in_set(RenderSystems::PrepareBindGroups),
            );

        AFTER::register_system(
            render_app,
            prepare_assets::<E, A>.in_set(RenderSystems::PrepareAssets),
        );
    }
}

// helper to allow specifying dependencies between render assets
pub trait RenderOctreeDependency {
    fn register_system(render_app: &mut SubApp, system: ScheduleConfigs<ScheduleSystem>);
}

impl RenderOctreeDependency for () {
    fn register_system(render_app: &mut SubApp, system: ScheduleConfigs<ScheduleSystem>) {
        render_app.add_systems(Render, system);
    }
}
