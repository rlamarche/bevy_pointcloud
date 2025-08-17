use crate::point_cloud::PointCloudData;
use crate::render::POINTCLOUD_SHADER_HANDLE;
use crate::render::point_cloud_uniform::PointCloudUniform;
use bevy_asset::prelude::*;
use bevy_core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT;
use bevy_ecs::prelude::*;
use bevy_pbr::{MeshPipeline, MeshPipelineKey, MeshPipelineViewLayoutKey};
use bevy_render::mesh::{VertexBufferLayout, VertexFormat};
use bevy_render::render_resource::{
    AsBindGroup, BindGroupLayout, BlendComponent, BlendFactor, BlendOperation, BlendState,
    CompareFunction, DepthBiasState, DepthStencilState, StencilState, VertexAttribute,
    VertexStepMode,
};
use bevy_render::renderer::RenderDevice;
use bevy_render::{
    mesh::MeshVertexBufferLayoutRef,
    prelude::*,
    render_resource::{
        ColorTargetState, ColorWrites, Face, FragmentState, FrontFace, MultisampleState,
        PolygonMode, PrimitiveState, RenderPipelineDescriptor, SpecializedMeshPipeline,
        SpecializedMeshPipelineError, TextureFormat, VertexState,
    },
};
use bevy_utils::default;
use crate::point_cloud_material::PointCloudMaterial;

#[derive(Resource)]
pub struct AttributePassPipeline {
    mesh_pipeline: MeshPipeline,
    shader_handle: Handle<Shader>,
    point_cloud_layout: BindGroupLayout,
    point_cloud_material_layout: BindGroupLayout,
}
impl FromWorld for AttributePassPipeline {
    fn from_world(world: &mut World) -> Self {
        let mesh_pipeline = world.resource::<MeshPipeline>();
        let render_device = world.resource::<RenderDevice>();

        Self {
            mesh_pipeline: mesh_pipeline.clone(),
            shader_handle: POINTCLOUD_SHADER_HANDLE,
            point_cloud_layout: PointCloudUniform::bind_group_layout(render_device),
            point_cloud_material_layout: PointCloudMaterial::bind_group_layout(render_device),
        }
    }
}

impl SpecializedMeshPipeline for AttributePassPipeline {
    type Key = MeshPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayoutRef,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        // We will only use the position of the mesh in our shader so we only need to specify that
        let mut vertex_attributes = Vec::new();
        if layout.0.contains(Mesh::ATTRIBUTE_POSITION) {
            // Make sure this matches the shader location
            vertex_attributes.push(Mesh::ATTRIBUTE_POSITION.at_shader_location(0));
        }
        // This will automatically generate the correct `VertexBufferLayout` based on the vertex attributes
        let vertex_buffer_layout = layout.0.get_layout(&vertex_attributes)?;

        let instances_buffer_layout = VertexBufferLayout {
            array_stride: size_of::<PointCloudData>() as u64,
            step_mode: VertexStepMode::Instance,
            attributes: vec![
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: 0,
                    shader_location: 3, // shader locations 0-2 are taken up by Position, Normal and UV attributes
                },
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: VertexFormat::Float32x4.size(),
                    shader_location: 4,
                },
            ],
        };

        Ok(RenderPipelineDescriptor {
            label: Some("pcl_attribute_pass_pipeline".into()),
            // We want to reuse the data from bevy so we use the same bind groups as the default
            // mesh pipeline
            layout: vec![
                // Bind group 0 is the view uniform
                self.mesh_pipeline
                    .get_view_layout(MeshPipelineViewLayoutKey::from(key))
                    .clone(),
                // Bind group 1 is the mesh uniform
                self.mesh_pipeline.mesh_layouts.model_only.clone(),
                // Bind group 2 is our point cloud uniform
                self.point_cloud_layout.clone(),
                // Bind group 3 is the point cloud material
                self.point_cloud_material_layout.clone(),
            ],
            push_constant_ranges: vec![],
            vertex: VertexState {
                shader: self.shader_handle.clone(),
                shader_defs: vec![],
                entry_point: "vertex".into(),
                buffers: vec![vertex_buffer_layout, instances_buffer_layout],
            },
            fragment: Some(FragmentState {
                shader: self.shader_handle.clone(),
                shader_defs: vec!["WEIGHTED_SPLATS".into()],
                entry_point: "fragment".into(),
                targets: vec![Some(ColorTargetState {
                    format: TextureFormat::Rgba32Float,
                    // Additive blending to allow merging close points
                    blend: Some(BlendState {
                        color: BlendComponent {
                            // To match Potree blending
                            src_factor: BlendFactor::SrcAlpha,
                            dst_factor: BlendFactor::One,
                            operation: BlendOperation::Add,
                        },
                        alpha: BlendComponent {
                            // To match Potree blending
                            src_factor: BlendFactor::SrcAlpha,
                            dst_factor: BlendFactor::One,
                            operation: BlendOperation::Add,
                        },
                    }),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                topology: key.primitive_topology(),
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                polygon_mode: PolygonMode::Fill,
                ..default()
            },
            depth_stencil: Some(DepthStencilState {
                format: CORE_3D_DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: CompareFunction::GreaterEqual,
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState {
                count: key.msaa_samples(),
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            zero_initialize_workgroup_memory: false,
        })
    }
}

// The code below is not needed because the mesh sorted has been disabled for WASM/WEBGL compatibility

// impl GetBatchData for AttributePassPipeline {
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
// impl GetFullBatchData for AttributePassPipeline {
//     type BufferInputData = MeshInputUniform;
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
//     fn get_binned_index(
//         _param: &SystemParamItem<Self::Param>,
//         _query_item: MainEntity,
//     ) -> Option<NonMaxU32> {
//         None
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
// }
