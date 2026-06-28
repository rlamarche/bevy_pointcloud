#[cfg(feature = "serialize")]
use bevy::reflect::{ReflectDeserialize, ReflectSerialize};
use bevy::{
    asset::{AssetId, Handle, UntypedAssetId},
    ecs::{
        component::Component,
        entity::Entity,
        query::QueryItem,
        reflect::{ReflectComponent, ReflectFromWorld},
        template::FromTemplate,
        world::{FromWorld, World},
    },
    prelude::{Deref, DerefMut},
    reflect::{std_traits::ReflectDefault, Reflect},
    render::{extract_component::ExtractComponent, sync_component::SyncComponent},
    transform::components::Transform,
};
use derive_more::derive::From;

use crate::{PointCloud, PointCloudChunk};

#[derive(
    Component, FromTemplate, Clone, Debug, Default, Deref, DerefMut, PartialEq, Eq, From, Reflect,
)]
#[component(immutable)]
#[require(Transform)]
#[reflect(Component, PartialEq, Debug, FromWorld, Clone, Default)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serialize", reflect(Serialize, Deserialize))]
pub struct PointCloud3d(pub Handle<PointCloud>);

impl SyncComponent for PointCloud3d {
    type Target = Self;
}

impl ExtractComponent for PointCloud3d {
    type QueryData = &'static PointCloud3d;
    type QueryFilter = ();
    type Out = Self;

    fn extract_component(item: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        Some(item.clone())
    }
}

impl From<&PointCloud3d> for AssetId<PointCloud> {
    fn from(value: &PointCloud3d) -> Self {
        value.id()
    }
}

impl From<&PointCloud3d> for UntypedAssetId {
    fn from(value: &PointCloud3d) -> Self {
        value.id().untyped()
    }
}

#[derive(
    Component, FromTemplate, Clone, Debug, Default, Deref, DerefMut, PartialEq, Eq, From, Reflect,
)]
#[reflect(Component, PartialEq, Debug, FromWorld, Clone)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serialize", reflect(Serialize, Deserialize))]
pub struct PointCloudChunk3d(pub Handle<PointCloudChunk>);

impl SyncComponent for PointCloudChunk3d {
    type Target = Self;
}

impl ExtractComponent for PointCloudChunk3d {
    type QueryData = &'static PointCloudChunk3d;
    type QueryFilter = ();
    type Out = Self;

    fn extract_component(item: QueryItem<'_, '_, Self::QueryData>) -> Option<Self::Out> {
        Some(item.clone())
    }
}

impl From<&PointCloudChunk3d> for AssetId<PointCloudChunk> {
    fn from(value: &PointCloudChunk3d) -> Self {
        value.id()
    }
}

impl From<&PointCloudChunk3d> for UntypedAssetId {
    fn from(value: &PointCloudChunk3d) -> Self {
        value.id().untyped()
    }
}

#[derive(Component, FromTemplate, Clone, PartialEq, Eq, Debug, Reflect)]
#[reflect(Component, PartialEq, Debug, FromWorld, Clone)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serialize", reflect(Serialize, Deserialize))]
#[relationship(relationship_target = ChildrenChunks)]
pub struct ChildChunkOf(#[entities] pub Entity);

impl ChildChunkOf {
    /// The parent entity of this child entity.
    #[inline]
    pub fn parent(&self) -> Entity {
        self.0
    }
}

// TODO: We need to impl either FromWorld or Default so ChildOf can be registered as Reflect.
// This is because Reflect deserialize by creating an instance and apply a patch on top.
// However ChildOf should only ever be set with a real user-defined entity.  Its worth looking into
// better ways to handle cases like this.
impl FromWorld for ChildChunkOf {
    #[inline(always)]
    fn from_world(_world: &mut World) -> Self {
        ChildChunkOf(Entity::PLACEHOLDER)
    }
}

#[derive(Component, Default, Debug, PartialEq, Eq, Reflect)]
#[relationship_target(relationship = ChildChunkOf, linked_spawn)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serialize", reflect(Serialize, Deserialize))]
#[reflect(Component, FromWorld, Default)]
pub struct ChildrenChunks(Vec<Entity>);
