mod limiter;

use core::marker::PhantomData;
use std::{any::TypeId, fmt::Debug, hash::Hash};

use bevy_app::{App, Plugin, SubApp};
use bevy_asset::{Asset, AssetEvent, AssetId, Assets, RenderAssetUsages, UntypedAssetId};
use bevy_ecs::{
    component::Component,
    entity::Entity,
    prelude::{Commands, IntoScheduleConfigs, MessageReader, ResMut, Resource},
    query::{QueryFilter, ROQueryItem, ReadOnlyQueryData, With, Without},
    schedule::ScheduleConfigs,
    system::{
        lifetimeless::Read, Query, Res, ScheduleSystem, StaticSystemParam, SystemParam,
        SystemParamItem, SystemState,
    },
    world::{FromWorld, Mut},
};
use bevy_log::{debug, error};
use bevy_platform::collections::{HashMap, HashSet};
use bevy_reflect::TypePath;
use bevy_render::{
    erased_render_asset::AssetExtractionSystems, render_resource::AsBindGroupError,
    sync_world::RenderEntity, ExtractSchedule, MainWorld, Render, RenderApp, RenderSystems,
};
pub use limiter::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PrepareAssetComponentError<E: Send + Sync + 'static> {
    #[error("Failed to prepare asset")]
    RetryNextUpdate(E),
    #[error("Failed to build bind group: {0}")]
    AsBindGroupError(AsBindGroupError),
}

/// Describes how an asset gets extracted and prepared for rendering.
///
/// In the [`ExtractSchedule`] step the [`ErasedRenderAssetComponent::SourceAsset`] is transferred
/// from the "main world" into the "render world" if the component implementing this trait is
/// added to the entity referring the asset.
///
/// After that in the [`RenderSystems::PrepareAssets`] step the extracted asset
/// is transformed into its GPU-representation of type [`ErasedRenderAsset`].
pub trait ErasedRenderAssetComponent: Send + Sync + 'static + Component {
    /// The representation of the asset in the "main world".
    type SourceAsset: Asset + Clone;
    /// The target representation of the asset in the "render world".
    type ErasedAsset: Send + Sync + 'static + Sized;

    /// Specifies all ECS data required by [`ErasedRenderAsset::prepare_asset`].
    ///
    /// For convenience use the [`lifetimeless`](bevy_ecs::system::lifetimeless) [`SystemParam`].
    type Param: SystemParam;

    /// ECS [`ReadOnlyQueryData`] to fetch the components to extract.
    type QueryData: ReadOnlyQueryData;
    /// Filters the entities with additional constraints.
    type QueryFilter: QueryFilter;

    type Key: Send + Sync + TypePath;

    /// Whether or not to unload the asset after extracting it to the render world.
    #[inline]
    fn asset_usage(_source_asset: &Self::SourceAsset) -> RenderAssetUsages {
        RenderAssetUsages::default()
    }

    /// Size of the data the asset will upload to the gpu. Specifying a return value
    /// will allow the asset to be throttled via [`RenderAssetBytesPerFrameLimiter`].
    #[inline]
    #[expect(
        unused_variables,
        reason = "The parameters here are intentionally unused by the default implementation; however, putting underscores here will result in the underscores being copied by rust-analyzer's tab completion."
    )]
    fn byte_len(erased_asset: &Self::SourceAsset) -> Option<usize> {
        None
    }

    fn asset_id(data: ROQueryItem<Self::QueryData>) -> AssetId<Self::SourceAsset>;

    /// Prepares the [`ErasedRenderAsset::SourceAsset`] for the GPU by transforming it into a [`ErasedRenderAsset`].
    ///
    /// ECS data may be accessed via `param`.
    fn prepare_asset(
        source_asset: Self::SourceAsset,
        asset_id: AssetId<Self::SourceAsset>,
        type_id: TypeId,
        param: &mut SystemParamItem<Self::Param>,
    ) -> Result<Self::ErasedAsset, PrepareAssetComponentError<Self::SourceAsset>>;

    /// Called whenever the [`ErasedRenderAsset::SourceAsset`] has been removed.
    ///
    /// You can implement this method if you need to access ECS data (via
    /// `_param`) in order to perform cleanup tasks when the asset is removed.
    ///
    /// The default implementation does nothing.
    fn unload_asset(
        _source_asset: AssetId<Self::SourceAsset>,
        _param: &mut SystemParamItem<Self::Param>,
    ) {
    }
}

/// This plugin extracts the changed assets from the "app world" into the "render world"
/// and prepares them for the GPU. They can then be accessed from the [`ErasedRenderAssetsComponent`] resource.
///
/// Therefore it sets up the [`ExtractSchedule`] and
/// [`RenderSystems::PrepareAssets`] steps for the specified [`ErasedRenderAsset`].
///
/// The `AFTER` generic parameter can be used to specify that `A::prepare_asset` should not be run until
/// `prepare_assets::<AFTER>` has completed. This allows the `prepare_asset` function to depend on another
/// prepared [`ErasedRenderAsset`], for example `Mesh::prepare_asset` relies on `ErasedRenderAssetsComponent::<GpuImage>` for morph
/// targets, so the plugin is created as `ErasedRenderAssetPlugin::<RenderMesh, GpuImage>::default()`.
pub struct ErasedRenderAssetComponentPlugin<
    A: ErasedRenderAssetComponent,
    AFTER: ErasedRenderAssetComponentDependency + 'static = (),
> {
    phantom: PhantomData<fn() -> (A, AFTER)>,
}

impl<A: ErasedRenderAssetComponent, AFTER: ErasedRenderAssetComponentDependency + 'static> Default
    for ErasedRenderAssetComponentPlugin<A, AFTER>
{
    fn default() -> Self {
        Self {
            phantom: Default::default(),
        }
    }
}

impl<A: ErasedRenderAssetComponent, AFTER: ErasedRenderAssetComponentDependency + 'static> Plugin
    for ErasedRenderAssetComponentPlugin<A, AFTER>
{
    fn build(&self, app: &mut App) {
        app.init_resource::<RenderAssetBytesPerFrame>();
        app.init_resource::<CachedExtractErasedRenderAssetComponentSystemState<A>>();

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.init_resource::<RenderAssetBytesPerFrameLimiter>();
            render_app
                .add_systems(ExtractSchedule, extract_render_asset_bytes_per_frame)
                .add_systems(
                    Render,
                    reset_render_asset_bytes_per_frame.in_set(RenderSystems::Cleanup),
                );
            render_app
                .add_systems(ExtractSchedule, extract_render_asset_bytes_per_frame)
                .add_systems(
                    Render,
                    reset_render_asset_bytes_per_frame.in_set(RenderSystems::Cleanup),
                );
        }
    }

    fn finish(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_resource::<ExtractedAssets<A>>()
                .init_resource::<ErasedRenderAssetsComponent<A::ErasedAsset, A::Key>>()
                .init_resource::<PrepareNextFrameAssets<A>>()
                .add_systems(
                    ExtractSchedule,
                    extract_erased_render_asset::<A>.in_set(AssetExtractionSystems),
                );
            AFTER::register_system(
                render_app,
                prepare_erased_assets::<A>.in_set(RenderSystems::PrepareAssets),
            );
        }
    }
}

// helper to allow specifying dependencies between render assets
pub trait ErasedRenderAssetComponentDependency {
    fn register_system(render_app: &mut SubApp, system: ScheduleConfigs<ScheduleSystem>);
}

impl ErasedRenderAssetComponentDependency for () {
    fn register_system(render_app: &mut SubApp, system: ScheduleConfigs<ScheduleSystem>) {
        render_app.add_systems(Render, system);
    }
}

impl<A: ErasedRenderAssetComponent> ErasedRenderAssetComponentDependency for A {
    fn register_system(render_app: &mut SubApp, system: ScheduleConfigs<ScheduleSystem>) {
        render_app.add_systems(Render, system.after(prepare_erased_assets::<A>));
    }
}

/// Temporarily stores the extracted and removed assets of the current frame.
#[derive(Resource)]
pub struct ExtractedAssets<A: ErasedRenderAssetComponent> {
    /// The assets extracted this frame.
    ///
    /// These are assets that were either added or modified this frame.
    pub extracted: Vec<(AssetId<A::SourceAsset>, A::SourceAsset)>,

    /// IDs of the assets that were removed this frame.
    ///
    /// These assets will not be present in [`ExtractedAssets::extracted`].
    pub removed: HashSet<AssetId<A::SourceAsset>>,

    /// IDs of the assets that were modified this frame.
    pub modified: HashSet<AssetId<A::SourceAsset>>,

    /// IDs of the assets that were added this frame.
    pub added: HashSet<AssetId<A::SourceAsset>>,
}

impl<A: ErasedRenderAssetComponent> Default for ExtractedAssets<A> {
    fn default() -> Self {
        Self {
            extracted: Default::default(),
            removed: Default::default(),
            modified: Default::default(),
            added: Default::default(),
        }
    }
}

pub struct UntypedAssetIdComponent<T>(UntypedAssetId, RenderAssetKey<T>);

impl<T> Debug for UntypedAssetIdComponent<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("UntypedAssetIdComponent")
            .field(&self.0)
            .field(&self.1)
            .finish()
    }
}

impl<T> Clone for UntypedAssetIdComponent<T> {
    fn clone(&self) -> Self {
        Self(self.0, self.1.clone())
    }
}

impl<T> PartialEq for UntypedAssetIdComponent<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 && self.1 == other.1
    }
}

impl<T> Eq for UntypedAssetIdComponent<T> {}

impl<T> Hash for UntypedAssetIdComponent<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
        self.1.hash(state);
    }
}

impl<T> From<(UntypedAssetId, RenderAssetKey<T>)> for UntypedAssetIdComponent<T> {
    fn from(value: (UntypedAssetId, RenderAssetKey<T>)) -> Self {
        UntypedAssetIdComponent(value.0, value.1)
    }
}

impl<T, A: Asset> From<(AssetId<A>, RenderAssetKey<T>)> for UntypedAssetIdComponent<T> {
    fn from(value: (AssetId<A>, RenderAssetKey<T>)) -> Self {
        UntypedAssetIdComponent(value.0.untyped(), value.1)
    }
}

/// Stores all GPU representations ([`ErasedRenderAsset`])
/// of [`ErasedRenderAsset::SourceAsset`] as long as they exist.
#[derive(Resource)]
pub struct ErasedRenderAssetsComponent<ERA, T>(HashMap<UntypedAssetIdComponent<T>, ERA>);

impl<ERA, T> Default for ErasedRenderAssetsComponent<ERA, T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<ERA, T> ErasedRenderAssetsComponent<ERA, T> {
    pub fn get(&self, id: impl Into<UntypedAssetIdComponent<T>>) -> Option<&ERA> {
        self.0.get(&id.into())
    }

    pub fn get_mut(&mut self, id: impl Into<UntypedAssetIdComponent<T>>) -> Option<&mut ERA> {
        self.0.get_mut(&id.into())
    }

    pub fn insert(&mut self, id: impl Into<UntypedAssetIdComponent<T>>, value: ERA) -> Option<ERA> {
        self.0.insert(id.into(), value)
    }

    pub fn remove(&mut self, id: impl Into<UntypedAssetIdComponent<T>>) -> Option<ERA> {
        self.0.remove(&id.into())
    }

    pub fn iter(&self) -> impl Iterator<Item = (UntypedAssetIdComponent<T>, &ERA)> {
        self.0.iter().map(|(k, v)| (k.clone(), v))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (UntypedAssetIdComponent<T>, &mut ERA)> {
        self.0.iter_mut().map(|(k, v)| (k.clone(), v))
    }
}

#[derive(Copy, Component, Eq)]
pub struct RenderAssetKey<A> {
    pub type_id: TypeId,
    _phantom: PhantomData<A>,
}

impl<T> Debug for RenderAssetKey<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderAssetKey")
            .field("type_id", &self.type_id)
            .field("_phantom", &self._phantom)
            .finish()
    }
}

impl<T> Clone for RenderAssetKey<T> {
    fn clone(&self) -> Self {
        Self {
            type_id: self.type_id,
            _phantom: self._phantom,
        }
    }
}

impl<T> PartialEq for RenderAssetKey<T> {
    fn eq(&self, other: &Self) -> bool {
        self.type_id == other.type_id && self._phantom == other._phantom
    }
}

impl<T> Hash for RenderAssetKey<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.type_id.hash(state);
        self._phantom.hash(state);
    }
}

impl<A> From<TypeId> for RenderAssetKey<A> {
    fn from(type_id: TypeId) -> Self {
        Self {
            type_id,
            _phantom: PhantomData,
        }
    }
}

#[derive(Resource)]
#[allow(clippy::type_complexity)]
struct CachedExtractErasedRenderAssetComponentSystemState<A: ErasedRenderAssetComponent> {
    state: SystemState<(
        Commands<'static, 'static>,
        Query<
            'static,
            'static,
            (Entity, Option<Read<RenderEntity>>, A::QueryData),
            (A::QueryFilter, Without<RenderAssetKey<A::Key>>, With<A>),
        >,
        MessageReader<'static, 'static, AssetEvent<A::SourceAsset>>,
        ResMut<'static, Assets<A::SourceAsset>>,
    )>,
}

impl<A: ErasedRenderAssetComponent> FromWorld
    for CachedExtractErasedRenderAssetComponentSystemState<A>
{
    fn from_world(world: &mut bevy_ecs::world::World) -> Self {
        Self {
            state: SystemState::new(world),
        }
    }
}

/// This system extracts all created or modified assets of the corresponding [`ErasedRenderAsset::SourceAsset`] type
/// into the "render world".
pub(crate) fn extract_erased_render_asset<A: ErasedRenderAssetComponent>(
    mut render_commands: Commands,
    mut main_world: ResMut<MainWorld>,
) {
    main_world.resource_scope(
        |world, mut cached_state: Mut<CachedExtractErasedRenderAssetComponentSystemState<A>>| {
            let type_id = TypeId::of::<A>();
            let key = RenderAssetKey::<A::Key>::from(type_id);

            let (
                mut commands,
                components_not_loaded,
                mut events,
                mut assets) = cached_state.state.get_mut(world);

            let mut entities_per_asset =
                <HashMap<AssetId<A::SourceAsset>, Vec<(Entity, Option<RenderEntity>)>>>::new();

            for (entity, render_entity, query_item) in &components_not_loaded {
                let asset_id = A::asset_id(query_item);
                entities_per_asset.entry(asset_id).or_default().push((entity, render_entity.copied()));
            }

            let mut needs_extracting = <HashMap<_, _>>::default();
            let mut removed = <HashSet<_>>::default();
            let mut modified = <HashSet<_>>::default();

            for event in events.read() {
                #[expect(
                    clippy::match_same_arms,
                    reason = "LoadedWithDependencies is marked as a TODO, so it's likely this will no longer lint soon."
                )]
                match event {
                    AssetEvent::Added { id } => {
                        bevy_log::info!("Added asset {}", id);
                        if let Some(entities) = entities_per_asset.remove(id) {
                            bevy_log::info!("Added asset {} needs extracting", id);
                            needs_extracting.insert(*id, entities);
                        }
                    }
                    AssetEvent::Modified { id } => {
                        // the asset will be prepared only if it was previously prepared
                        if !needs_extracting.contains_key(id) {
                            needs_extracting.insert(*id, Vec::new());
                            modified.insert(*id);
                        }
                    }
                    AssetEvent::Removed { .. } => {
                        // We don't care that the asset was removed from Assets<T> in the main world.
                        // An asset is only removed from ErasedRenderAssetsComponent<T> when its last handle is dropped (AssetEvent::Unused).
                    }
                    AssetEvent::Unused { id } => {
                        needs_extracting.remove(id);
                        modified.remove(id);
                        removed.insert(*id);
                    }
                    AssetEvent::LoadedWithDependencies { .. } => {
                        // TODO: handle this
                    }
                }
            }

            // add all new components
            needs_extracting.extend(entities_per_asset);

            let mut extracted_assets = Vec::new();
            let mut added = <HashSet<_>>::default();
            let mut component_values = Vec::new();
            let mut render_component_values = Vec::new();
            for (id, entities) in needs_extracting.drain() {
                if let Some(asset) = assets.get(id) {
                    let asset_usage = A::asset_usage(asset);
                    if asset_usage.contains(RenderAssetUsages::RENDER_WORLD) {
                        if asset_usage == RenderAssetUsages::RENDER_WORLD {
                            if let Some(asset) = assets.remove(id) {
                                extracted_assets.push((id, asset));
                                added.insert(id);
                            }
                        } else {
                            extracted_assets.push((id, asset.clone()));
                            added.insert(id);
                        }
                    }
                    // do not add key if the asset was just modified (because already done before)
                    if !modified.contains(&id) {
                        component_values.extend(
                            entities.iter().map(
                                |(entity, _)| (
                                    *entity,
                                    key.clone()
                                )
                        ));
                        render_component_values.extend(
                            entities.into_iter().flat_map(
                                |(_, render_entity)| Some(
                                    (render_entity?.id(), key.clone())
                                )
                        ));
                    }
                }
            }

            render_commands.insert_resource(ExtractedAssets::<A> {
                extracted: extracted_assets,
                removed,
                modified,
                added,
            });
            if !render_component_values.is_empty() {
                render_commands.try_insert_batch(render_component_values);
            }

            commands.try_insert_batch(component_values);

            cached_state.state.apply(world);
        },
    );
}

// TODO: consider storing inside system?
/// All assets that should be prepared next frame.
#[derive(Resource)]
pub struct PrepareNextFrameAssets<A: ErasedRenderAssetComponent> {
    assets: Vec<(AssetId<A::SourceAsset>, A::SourceAsset)>,
}

impl<A: ErasedRenderAssetComponent> Default for PrepareNextFrameAssets<A> {
    fn default() -> Self {
        Self {
            assets: Default::default(),
        }
    }
}

/// This system prepares all assets of the corresponding [`ErasedRenderAsset::SourceAsset`] type
/// which where extracted this frame for the GPU.
pub fn prepare_erased_assets<A: ErasedRenderAssetComponent>(
    mut extracted_assets: ResMut<ExtractedAssets<A>>,
    mut render_assets: ResMut<ErasedRenderAssetsComponent<A::ErasedAsset, A::Key>>,
    mut prepare_next_frame: ResMut<PrepareNextFrameAssets<A>>,
    param: StaticSystemParam<<A as ErasedRenderAssetComponent>::Param>,
    bpf: Res<RenderAssetBytesPerFrameLimiter>,
) {
    let type_id = TypeId::of::<A>();
    let key = RenderAssetKey::<A::Key>::from(type_id);

    let mut wrote_asset_count = 0;

    let mut param = param.into_inner();
    let queued_assets = core::mem::take(&mut prepare_next_frame.assets);
    for (id, extracted_asset) in queued_assets {
        if extracted_assets.removed.contains(&id) || extracted_assets.added.contains(&id) {
            // skip previous frame's assets that have been removed or updated
            continue;
        }

        let write_bytes = if let Some(size) = A::byte_len(&extracted_asset) {
            // we could check if available bytes > byte_len here, but we want to make some
            // forward progress even if the asset is larger than the max bytes per frame.
            // this way we always write at least one (sized) asset per frame.
            // in future we could also consider partial asset uploads.
            if bpf.exhausted() {
                prepare_next_frame.assets.push((id, extracted_asset));
                continue;
            }
            size
        } else {
            0
        };

        match A::prepare_asset(extracted_asset, id, type_id, &mut param) {
            Ok(prepared_asset) => {
                render_assets.insert((id, key.clone()), prepared_asset);
                bpf.write_bytes(write_bytes);
                wrote_asset_count += 1;
            }
            Err(PrepareAssetComponentError::RetryNextUpdate(extracted_asset)) => {
                prepare_next_frame.assets.push((id, extracted_asset));
            }
            Err(PrepareAssetComponentError::AsBindGroupError(e)) => {
                error!(
                    "{} Bind group construction failed: {e}",
                    core::any::type_name::<A>()
                );
            }
        }
    }

    for removed in extracted_assets.removed.drain() {
        render_assets.remove((removed, key.clone()));
        A::unload_asset(removed, &mut param);
    }

    let modified = extracted_assets.modified.clone();
    for (id, extracted_asset) in extracted_assets.extracted.drain(..) {
        // we remove previous here to ensure that if we are updating the asset then
        // any users will not see the old asset after a new asset is extracted,
        // even if the new asset is not yet ready or we are out of bytes to write.
        let removed = render_assets.remove((id, key.clone())).is_some();
        if modified.contains(&id) && !removed {
            // this asset was not previously available, so skip it
            continue;
        }

        let write_bytes = if let Some(size) = A::byte_len(&extracted_asset) {
            if bpf.exhausted() {
                prepare_next_frame.assets.push((id, extracted_asset));
                continue;
            }
            size
        } else {
            0
        };

        match A::prepare_asset(extracted_asset, id, type_id, &mut param) {
            Ok(prepared_asset) => {
                render_assets.insert((id, key.clone()), prepared_asset);
                bpf.write_bytes(write_bytes);
                wrote_asset_count += 1;
            }
            Err(PrepareAssetComponentError::RetryNextUpdate(extracted_asset)) => {
                prepare_next_frame.assets.push((id, extracted_asset));
            }
            Err(PrepareAssetComponentError::AsBindGroupError(e)) => {
                error!(
                    "{} Bind group construction failed: {e}",
                    core::any::type_name::<A>()
                );
            }
        }
    }

    if bpf.exhausted() && !prepare_next_frame.assets.is_empty() {
        debug!(
            "{} write budget exhausted with {} assets remaining (wrote {})",
            core::any::type_name::<A>(),
            prepare_next_frame.assets.len(),
            wrote_asset_count
        );
    }
}
