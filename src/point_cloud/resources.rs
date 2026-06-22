use std::marker::PhantomData;

use bevy_asset::UntypedAssetId;
use bevy_ecs::{change_detection::Tick, prelude::*};
use bevy_render::sync_world::MainEntityHashMap;

use crate::point::Point;

/// Stores all extracted instances of all [`PointCloud`]s in the render world.
#[derive(Resource)]
pub struct RenderPointCloudInstances<T: Point> {
    /// Maps from each entity in the main world to the
    /// [`RenderMaterialInstance`] associated with it.
    pub instances: MainEntityHashMap<RenderPointCloudInstance>,
    /// A monotonically-increasing counter, which we use to sweep
    /// [`RenderMaterialInstances::instances`] when the entities and/or required
    /// components are removed.
    pub current_change_tick: Tick,
    phantom: PhantomData<T>,
}

impl<T: Point> Default for RenderPointCloudInstances<T> {
    fn default() -> Self {
        Self {
            instances: Default::default(),
            current_change_tick: Default::default(),
            phantom: Default::default(),
        }
    }
}

/// The material associated with a single mesh instance in the main world.
///
/// Note that this uses an [`UntypedAssetId`] and isn't generic over the
/// material type, for simplicity.
pub struct RenderPointCloudInstance {
    /// The material asset.
    pub asset_id: UntypedAssetId,
    /// The [`RenderPointCloudInstances::current_change_tick`] at which this
    /// point cloud instance was last modified.
    pub last_change_tick: Tick,
}
