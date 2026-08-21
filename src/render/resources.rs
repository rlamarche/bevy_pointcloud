use bevy::{
    ecs::{
        entity::Entity,
        resource::Resource,
        world::{FromWorld, World},
    },
    platform::collections::HashMap,
    prelude::{Deref, DerefMut},
    render::{
        camera::PendingQueues,
        render_resource::{
            Extent3d, Texture, TextureDescriptor, TextureDimension, TextureFormat::Rgba8Uint,
            TextureUsages, TextureView, TextureViewDescriptor, TextureViewDimension,
        },
        renderer::RenderDevice,
        sync_world::MainEntity,
    },
};
use slotmap::{new_key_type, Key, SlotMap};

use crate::PreparedPointCloudUniform;

/// A resource that holds entities that couldn't be specialized and/or queued
///
/// See the documentation of [`PendingQueues`] for more information.
#[derive(Default, Deref, DerefMut, Resource)]
pub struct PendingPointCloudPhaseItemQueues(pub PendingQueues);

new_key_type! { pub struct OctreeInstanceIndex; }

impl OctreeInstanceIndex {
    /// Returns the unique index of the point cloud instance, starting at 0
    /// Note: [`slotmap::KeyData::as_ffi`] returns the generation shifted by 32 bits, and index.
    /// Casting to u32 truncates the generation and keep only the index.
    /// Then, for obvious reason, it starts at 1.
    pub fn index(&self) -> u32 {
        self.data().as_ffi() as u32 - 1
    }
}

/// This resource stores a unique autoincremented index for each point cloud instance of kind
/// [`PointCloudTopology::Octree`]. It is used as line number in the visible node's texture.
#[derive(Clone, Debug, Default, Resource)]
pub struct RenderOctreeInstancesIndex {
    /// TODO: use something else (not a slotmap)
    pub(crate) slab: SlotMap<OctreeInstanceIndex, Entity>,
    pub(crate) index: HashMap<Entity, OctreeInstanceIndex>,
    // pub(crate) added: Vec<Entity>,
}

impl RenderOctreeInstancesIndex {
    /// Add point cloud instance to index, if it already exists, does nothing.
    pub fn add(&mut self, entity: Entity) -> OctreeInstanceIndex {
        let index = *self
            .index
            .entry(entity)
            // as_ffi returns the generation << 32 + index, casting to u32 truncates the generation
            .or_insert_with(|| self.slab.insert(entity));

        // self.added.push(entity);

        index
    }

    /// Removes an entity from the index.
    /// TODO: call it on octree removal
    pub fn remove(&mut self, entity: Entity) -> Option<OctreeInstanceIndex> {
        if let Some(index) = self.index.remove(&entity) {
            self.slab.remove(index);
            Some(index)
        } else {
            None
        }
    }

    pub fn get(&self, entity: Entity) -> Option<OctreeInstanceIndex> {
        self.index.get(&entity).copied()
    }
}

#[derive(Resource, Default, Deref, DerefMut)]
pub struct PreparedPointCloudUniforms(HashMap<MainEntity, PreparedPointCloudUniform>);

#[derive(Resource)]
pub struct FallbackVisibleNodesTexture {
    pub texture: Texture,
    pub texture_view: TextureView,
}

impl FromWorld for FallbackVisibleNodesTexture {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let size = Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        };

        let descriptor = TextureDescriptor {
            label: Some("fallback_visible_nodes_texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: Rgba8Uint,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        };

        let texture = render_device.create_texture(&descriptor);

        let texture_view = texture.create_view(&TextureViewDescriptor {
            dimension: Some(TextureViewDimension::D2),
            array_layer_count: Some(size.depth_or_array_layers),
            ..TextureViewDescriptor::default()
        });

        Self {
            texture,
            texture_view,
        }
    }
}
