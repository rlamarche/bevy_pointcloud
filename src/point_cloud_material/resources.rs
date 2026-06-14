use bevy_asset::UntypedAssetId;
use bevy_ecs::{change_detection::Tick, prelude::*};
use bevy_render::sync_world::MainEntityHashMap;

// /// See the comments in [`RenderMaterialInstances::mesh_material`] for more
// /// information.
// pub(crate) static DUMMY_POINTCLOUD_MATERIAL: AssetId<SimplePointCloudMaterial> =
//     AssetId::<SimplePointCloudMaterial>::invalid();

/// Stores all extracted instances of all [`Material`]s in the render world.
#[derive(Resource, Default)]
pub struct RenderPointCloudMaterialInstances {
    /// Maps from each entity in the main world to the
    /// [`RenderMaterialInstance`] associated with it.
    pub instances: MainEntityHashMap<RenderPointCloudMaterialInstance>,
    /// A monotonically-increasing counter, which we use to sweep
    /// [`RenderMaterialInstances::instances`] when the entities and/or required
    /// components are removed.
    pub current_change_tick: Tick,
}

// impl RenderPointCloudMaterialInstances {
//     /// Returns the mesh material ID for the entity with the given mesh, or a
//     /// dummy mesh material ID if the mesh has no material ID.
//     ///
//     /// Meshes almost always have materials, but in very specific circumstances
//     /// involving custom pipelines they won't. (See the
//     /// `specialized_mesh_pipelines` example.)
//     pub(crate) fn pointcloud_material(&self, entity: MainEntity) -> UntypedAssetId {
//         match self.instances.get(&entity) {
//             Some(render_instance) => render_instance.asset_id,
//             None => DUMMY_POINTCLOUD_MATERIAL.into(),
//         }
//     }
// }

/// The material associated with a single mesh instance in the main world.
///
/// Note that this uses an [`UntypedAssetId`] and isn't generic over the
/// material type, for simplicity.
pub struct RenderPointCloudMaterialInstance {
    /// The material asset.
    pub asset_id: UntypedAssetId,
    /// The [`RenderMaterialInstances::current_change_tick`] at which this
    /// material instance was last modified.
    pub last_change_tick: Tick,
}

/// A [`SystemSet`] that contains all `extract_mesh_materials` systems.
#[derive(SystemSet, Clone, PartialEq, Eq, Debug, Hash)]
pub struct PointCloudMaterialExtractionSystems;
