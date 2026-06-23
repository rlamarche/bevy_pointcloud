pub mod allocate;
pub mod eviction;
pub mod limiter;
pub mod render;
pub mod resources;

use std::marker::PhantomData;

use allocate::allocate_visible_octree_nodes;
use bevy_app::prelude::*;
use bevy_asset::AssetId;
use bevy_ecs::{
    self,
    prelude::*,
    schedule::ScheduleConfigs,
    system::{ScheduleSystem, SystemParam, SystemParamItem},
};
use bevy_reflect::TypePath;
use bevy_render::{
    camera::extract_cameras, extract_component::ExtractComponent, ExtractSchedule, Render,
    RenderApp, RenderSystems,
};
use bytemuck::{Pod, Zeroable};
use eviction::update_extract_octree_node_eviction_queue;
use limiter::{
    extract_render_asset_bytes_per_frame, reset_render_asset_bytes_per_frame,
    RenderOctreeNodesBytesPerFrame, RenderOctreeNodesBytesPerFrameLimiter,
};
use render::{
    buffer::{ErasedRenderOctreesBuffers, RenderNodeData},
    extract::{extract_octree_node_allocations, extract_visible_octree_nodes},
    prepare::prepare_assets,
    resources::{ErasedRenderOctrees, ExtractedOctreeNodes, PrepareNextFrameOctreeNodes},
};
use resources::{ExtractOctreeNodeEvictionQueue, OctreeBufferSettings, OctreeNodeAllocations};

use super::{
    asset::Octree,
    node::{NodeData, OctreeNode},
};
use crate::{
    octree::{
        extract::{
            allocate::on_remove_octree,
            render::{
                asset::RenderOctreeNodeData,
                extract::{clear_removed_octrees, extract_removed_octrees},
                node::PrepareOctreeNodeError,
                prepare::prepare_octrees_uniforms,
                resources::{AllocatedOctreeNodes, OctreeEntityLayout},
            },
        },
        storage::NodeId,
        visibility::OctreeVisibilitySystems,
    },
    point::RGBPoint,
};

pub trait OctreeNodeExtraction: Send + Sync + TypePath {
    type NodeData: NodeData;
    type GpuData: Pod + Zeroable;
    type Component: ExtractComponent;
    type ExtractedNodeData: RenderNodeData;
    type ErasedRenderOctreeNode: Send + Sync + 'static;

    /// Specifies all ECS data required by [`OctreeNodeExtraction::extract_octree_node`].
    ///
    /// For convenience use the [`lifetimeless`](bevy_ecs::system::lifetimeless) [`SystemParam`].
    type ExtractParam: SystemParam;

    /// Specifies all ECS data required by [`OctreeNodeExtraction::prepare_octree_node`].
    ///
    /// For convenience use the [`lifetimeless`](bevy_ecs::system::lifetimeless) [`SystemParam`].
    type PrepareParam: SystemParam;

    /// Defines how the component is transferred into the "render world".
    fn extract_octree_node(
        asset: &Octree<Self::NodeData>,
        node: &OctreeNode<Self::NodeData>,
        param: &mut SystemParamItem<Self::ExtractParam>,
    ) -> Result<Option<Self::ExtractedNodeData>, BevyError>;

    /// Size of the data the asset will upload to the gpu. Specifying a return value
    /// will allow the asset to be throttled via [`RenderOctreeNodesBytesPerFrame`].
    #[inline]
    #[expect(
        unused_variables,
        reason = "The parameters here are intentionally unused by the default implementation; however, putting underscores here will result in the underscores being copied by rust-analyzer's tab completion."
    )]
    fn byte_len(source_node: &RenderOctreeNodeData<Self::ExtractedNodeData>) -> Option<usize> {
        None
    }

    /// Prepares the [`RenderAsset::SourceAsset`] for the GPU by transforming it into a [`RenderAsset`].
    ///
    /// ECS data may be accessed via `param`.
    #[allow(clippy::result_large_err)]
    fn prepare_octree_node(
        source_node: RenderOctreeNodeData<Self::ExtractedNodeData>,
        asset_id: AssetId<Octree<Self::NodeData>>,
        param: &mut SystemParamItem<Self::PrepareParam>,
    ) -> Result<Self::ErasedRenderOctreeNode, PrepareOctreeNodeError<Self::ExtractedNodeData>>;

    /// Called whenever the [`RenderOctreeNode::SourceOctreeNode`] has been removed.
    ///
    /// You can implement this method if you need to access ECS data (via
    /// `_param`) in order to perform cleanup tasks when the asset is removed.
    ///
    /// The default implementation does nothing.
    #[expect(
        unused_variables,
        reason = "The parameters here are intentionally unused by the default implementation; however, putting underscores here will result in the underscores being copied by rust-analyzer's tab completion."
    )]
    fn unload_octree_node(
        source_asset: AssetId<Octree<Self::NodeData>>,
        node_id: NodeId,
        param: &mut SystemParamItem<Self::PrepareParam>,
    ) {
    }
}

pub struct OctreeNodesRenderBufferPlugin<T: NodeData> {
    stride: usize,
    size: usize,
    max_bytes_per_frame: Option<usize>,
    _phantom: PhantomData<T>,
}

impl<T: NodeData> Plugin for OctreeNodesRenderBufferPlugin<T> {
    fn build(&self, app: &mut App) {
        app.insert_resource(OctreeBufferSettings::<T> {
            stride: self.stride,
            max_size: self.size,
            _phantom: PhantomData,
        })
        .insert_resource(RenderOctreeNodesBytesPerFrame::<T> {
            max_bytes: self.max_bytes_per_frame,
            _phantom: PhantomData,
        })
        .init_resource::<OctreeNodeAllocations<T>>()
        .init_resource::<ExtractOctreeNodeEvictionQueue<T>>()
        .add_systems(
            PostUpdate,
            (
                update_extract_octree_node_eviction_queue::<T>,
                allocate_visible_octree_nodes::<T>
                    .after(update_extract_octree_node_eviction_queue::<T>),
            )
                .in_set(ExtractOctreeNode),
        )
        .configure_sets(
            PostUpdate,
            ExtractOctreeNode.after(OctreeVisibilitySystems::CheckOctreeNodesVisibility),
        );

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_resource::<RenderOctreeNodesBytesPerFrameLimiter<T>>()
                .init_resource::<ErasedRenderOctreesBuffers<T>>()
                .init_resource::<AllocatedOctreeNodes<T>>()
                .add_systems(
                    ExtractSchedule,
                    (
                        extract_render_asset_bytes_per_frame::<T>,
                        clear_removed_octrees::<T>,
                    ),
                )
                .add_systems(
                    Render,
                    reset_render_asset_bytes_per_frame::<T>.in_set(RenderSystems::Cleanup),
                );
        }
    }
}

impl<T: NodeData> Default for OctreeNodesRenderBufferPlugin<T> {
    fn default() -> Self {
        Self {
            stride: size_of::<RGBPoint>(),
            size: 512 * 1024 * 1024, // 512 mb
            max_bytes_per_frame: None,
            _phantom: PhantomData,
        }
    }
}

impl<T: NodeData> OctreeNodesRenderBufferPlugin<T> {
    /// Construct with specific max memory size for GPU
    pub fn with_size(size: usize, stride: usize) -> Self {
        Self {
            stride,
            size,
            max_bytes_per_frame: None,
            _phantom: PhantomData,
        }
    }
    /// Construct with specific max memory size for GPU
    pub fn with_size_and_max_bytes_per_frame(
        size: usize,
        stride: usize,
        max_bytes_per_frame: usize,
    ) -> Self {
        Self {
            stride,
            size,
            max_bytes_per_frame: Some(max_bytes_per_frame),
            _phantom: PhantomData,
        }
    }
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
pub struct ExtractVisibleOctreeNodesPlugin<E, AFTER = ()>(PhantomData<fn() -> (E, AFTER)>);

impl<E, AFTER> Default for ExtractVisibleOctreeNodesPlugin<E, AFTER> {
    fn default() -> Self {
        Self(Default::default())
    }
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExtractOctreeNode;

impl<E, AFTER> Plugin for ExtractVisibleOctreeNodesPlugin<E, AFTER>
where
    E: OctreeNodeExtraction,
    AFTER: RenderOctreeDependency + 'static,
{
    fn build(&self, app: &mut App) {
        app.add_observer(on_remove_octree::<E>);

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_resource::<ExtractedOctreeNodes<E>>()
                .init_resource::<ErasedRenderOctrees<E::ErasedRenderOctreeNode>>()
                .init_resource::<PrepareNextFrameOctreeNodes<E>>()
                // Add in [`First`] schedule because it has to run just after the [`ExtractSchedule`] before any observer
                .add_systems(
                    ExtractSchedule,
                    (
                        extract_visible_octree_nodes::<E>.after(extract_cameras),
                        extract_octree_node_allocations::<E>,
                        extract_removed_octrees::<E>.before(clear_removed_octrees::<E::NodeData>),
                    ),
                )
                .add_systems(
                    Render,
                    prepare_octrees_uniforms::<E>.in_set(RenderSystems::PrepareBindGroups),
                );

            AFTER::register_system(
                render_app,
                prepare_assets::<E>.in_set(RenderSystems::PrepareAssets),
            );
        }
    }

    fn finish(&self, app: &mut App) {
        app.sub_app_mut(RenderApp)
            .init_resource::<OctreeEntityLayout>();

        // do nothing
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
