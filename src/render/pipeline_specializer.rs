use std::hash::Hash;

use bevy::{
    ecs::resource::Resource,
    log::error,
    material::{
        descriptor::{CachedRenderPipelineId, RenderPipelineDescriptor},
        specialize::SpecializedMeshPipelineError,
    },
    mesh::{MeshVertexBufferLayoutRef, VertexBufferLayout},
    platform::{
        collections::{
            hash_map::{Entry, RawEntryMut, VacantEntry},
            HashMap,
        },
        hash::FixedHasher,
    },
    render::render_resource::PipelineCache,
};

/// A trait that allows constructing different variants of a render pipeline from a key and the
/// particular mesh's vertex buffer layout.
///
/// See [`SpecializedMeshPipelines`] for more info.
pub trait SpecializedPointCloudPipeline {
    /// The key that defines each "variant" of the render pipeline.
    type Key: Clone + Hash + PartialEq + Eq;

    /// Construct a new render pipeline based on the provided key and vertex layout.
    ///
    /// The returned pipeline descriptor should have a single vertex buffer, which is derived from
    /// `layout`.
    fn specialize(
        &self,
        key: Self::Key,
        shape_layout: &MeshVertexBufferLayoutRef,
        instance_layout: &MeshVertexBufferLayoutRef,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError>;
}

/// A cache of different variants of a render pipeline based on a key and the particular mesh's
/// vertex buffer layout.
#[derive(Resource)]
pub struct SpecializedPointCloudPipelines<S: SpecializedPointCloudPipeline> {
    mesh_layout_cache: HashMap<
        (MeshVertexBufferLayoutRef, MeshVertexBufferLayoutRef, S::Key),
        CachedRenderPipelineId,
    >,
    vertex_layout_cache: VertexLayoutCache<S>,
}

type VertexLayoutCache<S> = HashMap<
    // the vertex buffer layout of the shape & the mesh
    (VertexBufferLayout, VertexBufferLayout),
    HashMap<<S as SpecializedPointCloudPipeline>::Key, CachedRenderPipelineId>,
>;

impl<S: SpecializedPointCloudPipeline> Default for SpecializedPointCloudPipelines<S> {
    fn default() -> Self {
        Self {
            mesh_layout_cache: Default::default(),
            vertex_layout_cache: Default::default(),
        }
    }
}

impl<S: SpecializedPointCloudPipeline> SpecializedPointCloudPipelines<S> {
    /// Construct a new render pipeline based on the provided key and the mesh's vertex buffer
    /// layout.
    #[inline]
    pub fn specialize(
        &mut self,
        cache: &PipelineCache,
        pipeline_specializer: &S,
        key: S::Key,
        shape_layout: &MeshVertexBufferLayoutRef,
        instance_layout: &MeshVertexBufferLayoutRef,
    ) -> Result<CachedRenderPipelineId, SpecializedMeshPipelineError> {
        return match self.mesh_layout_cache.entry((
            shape_layout.clone(),
            instance_layout.clone(),
            key.clone(),
        )) {
            Entry::Occupied(entry) => Ok(*entry.into_mut()),
            Entry::Vacant(entry) => specialize_slow(
                &mut self.vertex_layout_cache,
                cache,
                pipeline_specializer,
                key,
                shape_layout,
                instance_layout,
                entry,
            ),
        };

        #[cold]
        fn specialize_slow<S>(
            vertex_layout_cache: &mut VertexLayoutCache<S>,
            cache: &PipelineCache,
            specialize_pipeline: &S,
            key: S::Key,
            shape_layout: &MeshVertexBufferLayoutRef,
            instance_layout: &MeshVertexBufferLayoutRef,
            entry: VacantEntry<
                (MeshVertexBufferLayoutRef, MeshVertexBufferLayoutRef, S::Key),
                CachedRenderPipelineId,
                FixedHasher,
            >,
        ) -> Result<CachedRenderPipelineId, SpecializedMeshPipelineError>
        where
            S: SpecializedPointCloudPipeline,
        {
            let descriptor = specialize_pipeline
                .specialize(key.clone(), shape_layout, instance_layout)
                .map_err(|mut err| {
                    {
                        let SpecializedMeshPipelineError::MissingVertexAttribute(err) = &mut err;
                        err.pipeline_type = Some(core::any::type_name::<S>());
                    }
                    err
                })?;
            // Different MeshVertexBufferLayouts can produce the same final VertexBufferLayout
            // We want compatible vertex buffer layouts to use the same pipelines, so we must
            // "deduplicate" them
            // There is 2 vertex buffers because one for the shape, the other for the mesh
            let layout_map = match vertex_layout_cache.raw_entry_mut().from_key(&(
                descriptor.vertex.buffers[0].clone(),
                descriptor.vertex.buffers[1].clone(),
            )) {
                RawEntryMut::Occupied(entry) => entry.into_mut(),
                RawEntryMut::Vacant(entry) => {
                    entry
                        .insert(
                            (
                                descriptor.vertex.buffers[0].clone(),
                                descriptor.vertex.buffers[1].clone(),
                            ),
                            Default::default(),
                        )
                        .1
                }
            };
            Ok(*entry.insert(match layout_map.entry(key) {
                Entry::Occupied(entry) => {
                    if cfg!(debug_assertions) {
                        let stored_descriptor = cache.get_render_pipeline_descriptor(*entry.get());
                        if stored_descriptor != &descriptor {
                            error!(
                                "The cached pipeline descriptor for {} is not \
                                    equal to the generated descriptor for the given key. \
                                    This means the SpecializePipeline implementation uses \
                                    unused' MeshVertexBufferLayout information to specialize \
                                    the pipeline. This is not allowed because it would invalidate \
                                    the pipeline cache.",
                                core::any::type_name::<S>()
                            );
                        }
                    }
                    *entry.into_mut()
                }
                Entry::Vacant(entry) => *entry.insert(cache.queue_render_pipeline(descriptor)),
            }))
        }
    }
}

// TODO later
// impl SpecializedPointCloudPipeline for PrepassPipelineSpecializer {
//     type Key = ErasedMaterialPipelineKey;

//     fn specialize(
//         &self,
//         key: Self::Key,
//         shape_layout: &MeshVertexBufferLayoutRef,
//         instance_layout: &MeshVertexBufferLayoutRef,
//     ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
//         let mut shader_defs = Vec::new();
//         if self.properties.bindless {
//             shader_defs.push("BINDLESS".into());
//         }
//         let mut descriptor = self.pipeline.specialize(
//             key.mesh_key.downcast(),
//             shader_defs,
//             instance_layout,
//             &self.properties,
//         )?;

//         // This is a bit risky because it's possible to change something that would
//         // break the prepass but be fine in the main pass.
//         // Since this api is pretty low-level it doesn't matter that much, but it is a potential
// issue.         if let Some(specialize) = self.properties.user_specialize {
//             specialize(
//                 &self.pipeline.material_pipeline,
//                 &mut descriptor,
//                 instance_layout,
//                 key,
//             )?;
//         }

//         Ok(descriptor)
//     }
// }
