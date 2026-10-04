use std::{fmt::Debug, marker::PhantomData};

use bevy::{
    asset::{AssetId, UntypedAssetId},
    ecs::{
        change_detection::Tick,
        entity::{Entity, EntityHashMap},
        resource::Resource,
    },
    material::descriptor::CachedRenderPipelineId,
    pbr::MaterialBindingId,
    platform::collections::HashMap,
    prelude::{Deref, DerefMut},
    render::{
        camera::PendingQueues,
        sync_world::{MainEntity, MainEntityHashMap, RenderEntity},
        view::RetainedViewEntity,
    },
};

use crate::SimplePointCloudMaterial;

/// Stores all extracted instances of all [`Material`]s in the render world.
#[derive(Resource, Default)]
pub struct RenderPointCloudMaterialInstances {
    /// Maps from each entity in the main world to the
    /// [`RenderMaterialInstance`] associated with it.
    pub instances: MainEntityHashMap<RenderMaterialInstance>,
    /// A monotonically-increasing counter, which we use to sweep
    /// [`RenderMaterialInstances::instances`] when the entities and/or required
    /// components are removed.
    pub current_change_tick: Tick,
}

/// A dummy [`AssetId`] that we use as a placeholder whenever a mesh doesn't
/// have a material.
///
/// See the comments in [`RenderMaterialInstances::mesh_material`] for more
/// information.
pub(crate) static DUMMY_POINT_CLOUD_MATERIAL: AssetId<SimplePointCloudMaterial> =
    AssetId::<SimplePointCloudMaterial>::invalid();

impl RenderPointCloudMaterialInstances {
    /// Returns the point cloud material ID for the entity with the given point cloud, or a
    /// dummy mesh material ID if the mesh has no material ID.
    ///
    /// Point clouds almost always have materials, but in very specific circumstances
    /// involving custom pipelines they won't. (See the
    /// `specialized_mesh_pipelines` example.)
    pub(crate) fn point_cloud_material(&self, entity: MainEntity) -> UntypedAssetId {
        match self.instances.get(&entity) {
            Some(render_instance) => render_instance.asset_id,
            None => DUMMY_POINT_CLOUD_MATERIAL.into(),
        }
    }
}

/// The material associated with a single mesh instance in the main world.
///
/// Note that this uses an [`UntypedAssetId`] and isn't generic over the
/// material type, for simplicity.
pub struct RenderMaterialInstance {
    /// The material asset.
    pub asset_id: UntypedAssetId,
    /// The [`RenderMaterialInstances::current_change_tick`] at which this
    /// material instance was last modified.
    pub last_change_tick: Tick,
}

/// Temporarily stores entities that were determined to either need their
/// specialized pipelines updated or to have their specialized pipelines
/// removed.
#[derive(Resource, Clone)]
pub struct EntitiesNeedingSpecialization<M> {
    /// Entities that need to have their pipelines updated.
    pub changed: Vec<(Entity, RenderEntity)>,
    /// Entities that need to have their pipelines removed, *unless* they also
    /// appear in [`Self::changed`].
    ///
    /// We can't determine which entities truly need to have their pipelines removed until all.
    /// Also stores the associated `RenderEntity` because we need it. Populated in observer
    /// [`on_remove_point_cloud_chunk_3d`].
    pub removed: EntityHashMap<RenderEntity>,
    _marker: PhantomData<M>,
}

impl<M> Debug for EntitiesNeedingSpecialization<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EntitiesNeedingSpecialization")
            .field("changed", &self.changed)
            .field("removed", &self.removed)
            .finish()
    }
}

impl<M> Default for EntitiesNeedingSpecialization<M> {
    fn default() -> Self {
        Self {
            changed: Default::default(),
            removed: Default::default(),
            _marker: Default::default(),
        }
    }
}

/// Stores the [`SpecializedMaterialViewPipelineCache`] for each view.
#[derive(Resource, Deref, DerefMut, Default)]
pub struct SpecializedPointCloudMaterialPipelineCache {
    // view entity -> view pipeline cache
    #[deref]
    map: HashMap<RetainedViewEntity, SpecializedPointCloudMaterialViewPipelineCache>,
}

/// Stores the cached render pipeline ID for each entity in a single view, as
/// well as the last time it was changed.
#[derive(Deref, DerefMut, Default)]
pub struct SpecializedPointCloudMaterialViewPipelineCache {
    // material entity -> (tick, pipeline_id)
    #[deref]
    map: EntityHashMap<CachedRenderPipelineId>,
}

/// Holds all entities with mesh materials that couldn't be specialized and/or
/// queued because their materials hadn't loaded yet.
///
/// See the [`PendingQueues`] documentation for more information.
#[derive(Default, Deref, DerefMut, Resource)]
pub struct PendingPointCloudMaterialQueues(pub PendingQueues);

/// A resource that maps each untyped material ID to its binding.
///
/// This duplicates information in `RenderAssets<M>`, but it doesn't have the
/// `M` type parameter, so it can be used in untyped contexts like
/// [`crate::render::mesh::collect_meshes_for_gpu_building`].
#[derive(Resource, Default, Deref, DerefMut)]
pub struct RenderMaterialBindings(HashMap<UntypedAssetId, MaterialBindingId>);
