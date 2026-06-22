mod gpu_mapper;
mod resources;
use std::{marker::PhantomData, sync::Arc};

use bevy_app::prelude::*;
use bevy_asset::{AsAssetId, Asset, AssetApp, AssetId, Handle};
use bevy_camera::visibility::{add_visibility_class, ViewVisibility, Visibility, VisibilityClass};
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::*;
use bevy_platform::collections::hash_map::Entry;
use bevy_reflect::TypePath;
use bevy_render::{sync_world::MainEntity, Extract, ExtractSchedule, RenderApp};
use bevy_transform::prelude::*;
pub use gpu_mapper::*;
pub use resources::*;

use crate::{point::Point, render::RenderPipelinePlugin};

pub const QUAD_POSITIONS: &[[f32; 3]] = &[
    [-0.5, -0.5, 0.0],
    [0.5, -0.5, 0.0],
    [0.5, 0.5, 0.0],
    [-0.5, 0.5, 0.0],
];
pub const QUAD_INDICES: &[u32] = &[0, 1, 2, 2, 3, 0];

#[derive(Debug, Clone, Asset, TypePath)]
pub struct PointCloud<T: Point> {
    pub points: Arc<Vec<T>>,
}

#[derive(Component, Clone, Debug, Default, Deref, DerefMut, TypePath, PartialEq, Eq)]
#[require(Transform)]
pub struct PointCloud3d<T: Point>(pub Handle<PointCloud<T>>);

impl<T: Point> From<PointCloud3d<T>> for AssetId<PointCloud<T>> {
    fn from(point_cloud: PointCloud3d<T>) -> Self {
        point_cloud.id()
    }
}

impl<T: Point> From<&PointCloud3d<T>> for AssetId<PointCloud<T>> {
    fn from(pointcloud: &PointCloud3d<T>) -> Self {
        pointcloud.id()
    }
}

impl<T: Point> AsAssetId for PointCloud3d<T> {
    type Asset = PointCloud<T>;

    fn as_asset_id(&self) -> AssetId<Self::Asset> {
        self.id()
    }
}

pub struct PointCloudsPlugin<T: Point>(PhantomData<fn() -> T>);

impl<T: Point> Default for PointCloudsPlugin<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T: Point> Plugin for PointCloudsPlugin<T> {
    fn build(&self, app: &mut App) {
        app.init_asset::<PointCloud<T>>()
            .register_required_components::<PointCloud3d<T>, Visibility>()
            .register_required_components::<PointCloud3d<T>, VisibilityClass>()
            .add_plugins(RenderPipelinePlugin::<T>::default());

        app.world_mut()
            .register_component_hooks::<PointCloud3d<T>>()
            .on_add(add_visibility_class::<PointCloud3d<T>>);

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_resource::<RenderPointCloudInstances<T>>()
                .add_systems(
                    ExtractSchedule,
                    (
                        extract_point_clouds::<T>.in_set(PointCloudExtractionSystems),
                        early_sweep_point_cloud_instances::<T>
                            .after(PointCloudExtractionSystems)
                            .before(late_sweep_point_cloud_instances::<T>),
                    ),
                );
        }
    }
}

/// A [`SystemSet`] that contains all `extract_mesh_materials` systems.
#[derive(SystemSet, Clone, PartialEq, Eq, Debug, Hash)]
pub struct PointCloudExtractionSystems;

/// Fills the [`RenderPointCloudMaterialInstances`] resources from the point clouds in the
/// scene.
#[allow(clippy::type_complexity)]
fn extract_point_clouds<T: Point>(
    mut pointcloud_instances: ResMut<RenderPointCloudInstances<T>>,
    changed_meshes_query: Extract<
        Query<
            (Entity, &ViewVisibility, &PointCloud3d<T>),
            Or<(Changed<ViewVisibility>, Changed<PointCloud3d<T>>)>,
        >,
    >,
) {
    let last_change_tick = pointcloud_instances.current_change_tick;

    for (entity, view_visibility, point_cloud) in &changed_meshes_query {
        if view_visibility.get() {
            pointcloud_instances.instances.insert(
                entity.into(),
                RenderPointCloudInstance {
                    asset_id: point_cloud.id().untyped(),
                    last_change_tick,
                },
            );
        } else {
            pointcloud_instances
                .instances
                .remove(&MainEntity::from(entity));
        }
    }
}

/// Removes point cloud from [`RenderPointCloudInstances`] when their
/// [`PointCloud3d`] components are removed.
///
/// This is tricky because we have to deal with the case in which a point cloud of
/// type A was removed and replaced with a material of type B in the same frame
/// (which is actually somewhat common of an operation). In this case, even
/// though an entry will be present in `RemovedComponents<PointCloud3d<T>>`,
/// we must not remove the entry in `RenderPointCloudMaterialInstances` which corresponds
/// to material B. To handle this case, we use change ticks to avoid removing
/// the entry if it was updated this frame.
///
/// This is the first of two sweep phases. Because this phase runs once per
/// material type, we need a second phase in order to guarantee that we only
/// bump [`RenderPointCloudMaterialInstances::current_change_tick`] once.
pub fn early_sweep_point_cloud_instances<T: Point>(
    mut pointcloud_instances: ResMut<RenderPointCloudInstances<T>>,
    mut removed_pointclouds_query: Extract<RemovedComponents<PointCloud3d<T>>>,
) {
    let last_change_tick = pointcloud_instances.current_change_tick;

    for entity in removed_pointclouds_query.read() {
        if let Entry::Occupied(occupied_entry) = pointcloud_instances.instances.entry(entity.into())
        {
            // Only sweep the entry if it wasn't updated this frame.
            if occupied_entry.get().last_change_tick != last_change_tick {
                occupied_entry.remove();
            }
        }
    }
}

/// Removes point cloud materials from [`RenderPointCloudInstances`] when their
/// [`ViewVisibility`] components are removed.
///
/// This runs after all invocations of `early_sweep_point_cloud_instances` and is
/// responsible for bumping [`RenderPointCloudInstances::current_change_tick`] in
/// preparation for a new frame.
pub fn late_sweep_point_cloud_instances<T: Point>(
    mut pointcloud_instances: ResMut<RenderPointCloudInstances<T>>,
    mut removed_meshes_query: Extract<RemovedComponents<PointCloud3d<T>>>,
) {
    let last_change_tick = pointcloud_instances.current_change_tick;

    for entity in removed_meshes_query.read() {
        if let Entry::Occupied(occupied_entry) = pointcloud_instances.instances.entry(entity.into())
        {
            // Only sweep the entry if it wasn't updated this frame. It's
            // possible that a `ViewVisibility` component was removed and
            // re-added in the same frame.
            if occupied_entry.get().last_change_tick != last_change_tick {
                occupied_entry.remove();
            }
        }
    }

    pointcloud_instances
        .current_change_tick
        .set(last_change_tick.get() + 1);
}
