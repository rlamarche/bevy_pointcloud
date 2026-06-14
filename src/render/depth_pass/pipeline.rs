use std::{hash::Hash, marker::PhantomData, sync::Arc};

use bevy_core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT;
use bevy_ecs::prelude::*;
use bevy_mesh::{PrimitiveTopology, VertexBufferLayout, VertexFormat};
use bevy_pbr::{ErasedMaterialKey, MeshPipeline, MeshPipelineKey, MeshPipelineViewLayoutKey};
#[cfg(feature = "pointcloud_octree")]
use bevy_render::render_resource::{
    binding_types::{texture_2d, uniform_buffer},
    BindGroupLayoutEntries, ShaderStages, TextureSampleType,
};
use bevy_render::{
    render_resource::{
        AsBindGroup, BindGroupLayoutDescriptor, ColorTargetState, ColorWrites, CompareFunction,
        DepthBiasState, DepthStencilState, Face, FragmentState, FrontFace, MultisampleState,
        PolygonMode, PrimitiveState, RenderPipelineDescriptor, SpecializedRenderPipeline,
        StencilState, TextureFormat, VertexAttribute, VertexState, VertexStepMode,
    },
    renderer::RenderDevice,
};
use bevy_shader::ShaderDefVal;
use bevy_utils::default;

#[cfg(feature = "pointcloud_octree")]
use crate::pointcloud_octree::extract::{PointCloudNodeDataUniform, PointCloudOctreeUniform};
use crate::{
    point::{GpuPoint, Point},
    render::{point_cloud_uniform::PointCloudUniform, MATERIAL_BIND_GROUP_INDEX},
    PointCloudMaterialProperties,
};

#[derive(Clone, Resource)]
pub struct DepthPipeline<T: Point, U: GpuPoint> {
    mesh_pipeline: MeshPipeline,
    point_cloud_layout: BindGroupLayoutDescriptor,
    #[cfg(feature = "pointcloud_octree")]
    point_cloud_octree_visible_nodes_layout: BindGroupLayoutDescriptor,
    #[cfg(feature = "pointcloud_octree")]
    point_cloud_octree_node_data_layout: BindGroupLayoutDescriptor,
    #[cfg(feature = "pointcloud_octree")]
    point_cloud_octree_data_layout: BindGroupLayoutDescriptor,
    #[allow(clippy::type_complexity)]
    _phantom: PhantomData<fn() -> (T, U)>,
}
impl<T: Point, U: GpuPoint> FromWorld for DepthPipeline<T, U> {
    fn from_world(world: &mut World) -> Self {
        let mesh_pipeline = world.resource::<MeshPipeline>();
        let render_device = world.resource::<RenderDevice>();

        Self {
            mesh_pipeline: mesh_pipeline.clone(),
            point_cloud_layout: PointCloudUniform::bind_group_layout_descriptor(render_device),
            #[cfg(feature = "pointcloud_octree")]
            point_cloud_octree_visible_nodes_layout: BindGroupLayoutDescriptor {
                label: "pcl_octree_visible_nodes_layout".into(),
                entries: BindGroupLayoutEntries::single(
                    ShaderStages::VERTEX,
                    texture_2d(TextureSampleType::Uint),
                )
                .to_vec(),
            },
            #[cfg(feature = "pointcloud_octree")]
            point_cloud_octree_node_data_layout: BindGroupLayoutDescriptor {
                label: "pcl_octree_node_data".into(),
                entries: BindGroupLayoutEntries::single(
                    ShaderStages::VERTEX,
                    uniform_buffer::<PointCloudNodeDataUniform>(false),
                )
                .to_vec(),
            },
            #[cfg(feature = "pointcloud_octree")]
            point_cloud_octree_data_layout: BindGroupLayoutDescriptor {
                label: "layout_pcl_octree_layout".into(),
                entries: BindGroupLayoutEntries::single(
                    ShaderStages::VERTEX,
                    uniform_buffer::<PointCloudOctreeUniform>(false),
                )
                .to_vec(),
            },
            _phantom: PhantomData,
        }
    }
}

pub struct DepthPassPipelineSpecializer<T: Point, U: GpuPoint> {
    pub(crate) pipeline: DepthPipeline<T, U>,
    pub(crate) properties: Arc<PointCloudMaterialProperties>,
}

#[derive(PartialEq, Eq, Hash, Clone)]
pub struct DepthPipelineKey {
    pub mesh_key: MeshPipelineKey,
    pub use_edl: bool,
    pub is_octree: bool,
    pub material_key: ErasedMaterialKey,
}

impl DepthPipelineKey {
    #[inline]
    pub fn new(
        mesh_key: MeshPipelineKey,
        use_edl: bool,
        is_octree: bool,
        material_key: ErasedMaterialKey,
    ) -> Self {
        Self {
            mesh_key,
            use_edl,
            is_octree,
            material_key,
        }
    }
}

impl<T: Point, U: GpuPoint> SpecializedRenderPipeline for DepthPassPipelineSpecializer<T, U> {
    type Key = DepthPipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let vertex_buffer_layout = VertexBufferLayout {
            array_stride: VertexFormat::Float32x4.size(),
            step_mode: VertexStepMode::Vertex,
            attributes: vec![VertexAttribute {
                format: VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            }],
        };

        let instance_buffer_layout = U::vertex_buffer_layout();

        let mut shader_defs = self.properties.depth_shader_defs.clone();
        shader_defs.push(ShaderDefVal::UInt(
            "MATERIAL_BIND_GROUP".into(),
            MATERIAL_BIND_GROUP_INDEX as u32,
        ));

        if key.use_edl {
            shader_defs.push("USE_EDL".into());
        }
        if key.is_octree {
            shader_defs.push("IS_OCTREE".into());
        }

        #[allow(unused_mut)]
        let mut layout = vec![
            // Bind group 0 is the view uniform
            self.pipeline
                .mesh_pipeline
                .get_view_layout(MeshPipelineViewLayoutKey::from(key.mesh_key))
                .clone()
                .main_layout,
            // Bind group 1 is our point cloud uniform
            self.pipeline.point_cloud_layout.clone(),
            // Bind group 2 is the point cloud material
            self.properties
                .material_layout
                .as_ref()
                .expect("Missing Point Cloud Material Layout")
                .clone(),
        ];

        #[cfg(feature = "pointcloud_octree")]
        if key.is_octree {
            layout.push(
                self.pipeline
                    .point_cloud_octree_visible_nodes_layout
                    .clone(),
            );
            layout.push(self.pipeline.point_cloud_octree_node_data_layout.clone());
            layout.push(self.pipeline.point_cloud_octree_data_layout.clone());
        }

        RenderPipelineDescriptor {
            label: Some("pcl_depth_pass_pipeline".into()),
            // We want to reuse the data from bevy so we use the same bind groups as the default
            // mesh pipeline
            layout,
            push_constant_ranges: vec![],
            vertex: VertexState {
                shader: self.properties.depth_pass_vertex_shader_handle.clone(),
                shader_defs: shader_defs.clone(),
                entry_point: Some("vertex".into()),
                buffers: vec![vertex_buffer_layout, instance_buffer_layout],
            },
            fragment: Some(FragmentState {
                shader: self.properties.depth_pass_fragment_shader_handle.clone(),
                shader_defs,
                entry_point: Some("fragment".into()),
                // The target will store a mask to discard outside pixels in normalize pass
                // Because we can't bind the depth buffer in WASM/WebGL
                targets: vec![Some(ColorTargetState {
                    format: if key.use_edl {
                        TextureFormat::Rg32Float
                    } else {
                        TextureFormat::R32Float
                    },
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                polygon_mode: PolygonMode::Fill,
                ..default()
            },
            // We need to write the depth information into the depth buffer
            depth_stencil: Some(DepthStencilState {
                format: CORE_3D_DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: CompareFunction::GreaterEqual,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState {
                count: key.mesh_key.msaa_samples(),
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            zero_initialize_workgroup_memory: false,
        }
    }
}

// The code below is not needed because the mesh sorted has been disabled for WASM/WEBGL compatibility

// impl GetBatchData for DepthPipeline {
//     type Param = (
//         SRes<RenderMeshInstances>,
//         SRes<RenderAssets<RenderMesh>>,
//         SRes<MeshAllocator>,
//     );
//     type CompareData = AssetId<Mesh>;
//     type BufferData = MeshUniform;
//
//     fn get_batch_data(
//         (mesh_instances, _render_assets, mesh_allocator): &SystemParamItem<Self::Param>,
//         (_entity, main_entity): (Entity, MainEntity),
//     ) -> Option<(Self::BufferData, Option<Self::CompareData>)> {
//         let RenderMeshInstances::CpuBuilding(ref mesh_instances) = **mesh_instances else {
//             error!(
//                 "`get_batch_data` should never be called in GPU mesh uniform \
//                 building mode"
//             );
//             return None;
//         };
//         let mesh_instance = mesh_instances.get(&main_entity)?;
//         let first_vertex_index =
//             match mesh_allocator.mesh_vertex_slice(&mesh_instance.mesh_asset_id) {
//                 Some(mesh_vertex_slice) => mesh_vertex_slice.range.start,
//                 None => 0,
//             };
//         let mesh_uniform = {
//             let mesh_transforms = &mesh_instance.transforms;
//             let (local_from_world_transpose_a, local_from_world_transpose_b) =
//                 mesh_transforms.world_from_local.inverse_transpose_3x3();
//             MeshUniform {
//                 world_from_local: mesh_transforms.world_from_local.to_transpose(),
//                 previous_world_from_local: mesh_transforms.previous_world_from_local.to_transpose(),
//                 lightmap_uv_rect: UVec2::ZERO,
//                 local_from_world_transpose_a,
//                 local_from_world_transpose_b,
//                 flags: mesh_transforms.flags,
//                 first_vertex_index,
//                 current_skin_index: u32::MAX,
//                 material_and_lightmap_bind_group_slot: 0,
//                 tag: 0,
//                 pad: 0,
//             }
//         };
//         Some((mesh_uniform, None))
//     }
// }
// impl GetFullBatchData for DepthPipeline {
//     type BufferInputData = MeshInputUniform;
//
//     fn get_index_and_compare_data(
//         (mesh_instances, _, _): &SystemParamItem<Self::Param>,
//         main_entity: MainEntity,
//     ) -> Option<(NonMaxU32, Option<Self::CompareData>)> {
//         // This should only be called during GPU building.
//         let RenderMeshInstances::GpuBuilding(ref mesh_instances) = **mesh_instances else {
//             error!(
//                 "`get_index_and_compare_data` should never be called in CPU mesh uniform building \
//                 mode"
//             );
//             return None;
//         };
//         let mesh_instance = mesh_instances.get(&main_entity)?;
//         Some((
//             mesh_instance.current_uniform_index,
//             mesh_instance
//                 .should_batch()
//                 .then_some(mesh_instance.mesh_asset_id),
//         ))
//     }
//
//     fn get_binned_batch_data(
//         (mesh_instances, _render_assets, mesh_allocator): &SystemParamItem<Self::Param>,
//         main_entity: MainEntity,
//     ) -> Option<Self::BufferData> {
//         let RenderMeshInstances::CpuBuilding(ref mesh_instances) = **mesh_instances else {
//             error!(
//                 "`get_binned_batch_data` should never be called in GPU mesh uniform building mode"
//             );
//             return None;
//         };
//         let mesh_instance = mesh_instances.get(&main_entity)?;
//         let first_vertex_index =
//             match mesh_allocator.mesh_vertex_slice(&mesh_instance.mesh_asset_id) {
//                 Some(mesh_vertex_slice) => mesh_vertex_slice.range.start,
//                 None => 0,
//             };
//
//         Some(MeshUniform::new(
//             &mesh_instance.transforms,
//             first_vertex_index,
//             mesh_instance.material_bindings_index.slot,
//             None,
//             None,
//             None,
//         ))
//     }
//
//     fn write_batch_indirect_parameters_metadata(
//         indexed: bool,
//         base_output_index: u32,
//         batch_set_index: Option<NonMaxU32>,
//         indirect_parameters_buffers: &mut UntypedPhaseIndirectParametersBuffers,
//         indirect_parameters_offset: u32,
//     ) {
//         // Note that `IndirectParameters` covers both of these structures, even
//         // though they actually have distinct layouts. See the comment above that
//         // type for more information.
//         let indirect_parameters = IndirectParametersCpuMetadata {
//             base_output_index,
//             batch_set_index: match batch_set_index {
//                 None => !0,
//                 Some(batch_set_index) => u32::from(batch_set_index),
//             },
//         };
//
//         if indexed {
//             indirect_parameters_buffers
//                 .indexed
//                 .set(indirect_parameters_offset, indirect_parameters);
//         } else {
//             indirect_parameters_buffers
//                 .non_indexed
//                 .set(indirect_parameters_offset, indirect_parameters);
//         }
//     }
//
//     fn get_binned_index(
//         _param: &SystemParamItem<Self::Param>,
//         _query_item: MainEntity,
//     ) -> Option<NonMaxU32> {
//         None
//     }
// }
