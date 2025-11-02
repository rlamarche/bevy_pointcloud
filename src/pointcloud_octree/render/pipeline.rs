use super::super::asset::PointData;
use super::POINTCLOUD_SHADER_HANDLE;
use bevy_asset::Handle;
use bevy_core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT;
use bevy_ecs::prelude::*;
use bevy_mesh::{Mesh, VertexBufferLayout};
use bevy_pbr::{MeshPipeline, MeshPipelineKey, MeshPipelineViewLayoutKey};
use bevy_render::render_resource::binding_types::texture_2d_multisampled;
use bevy_render::{
    render_resource::{binding_types::texture_2d, *},
    renderer::RenderDevice,
};
use bevy_shader::Shader;
use bevy_utils::default;
use crate::point_cloud_material::PointCloudMaterial;
use crate::pointcloud_octree::render::data::PointCloudOctree3dUniform;

#[derive(PartialEq, Eq, Hash, Clone)]
pub struct PointCloudOctreePipelineKey {
    pub samples: u32,
    pub use_edl: bool,
    pub edl_neighbour_count: u32,
    pub mesh_pipeline_key: MeshPipelineKey,
}

#[derive(Component)]
pub struct PointCloudOctreePipelineId(pub CachedRenderPipelineId);

#[derive(Resource)]
pub struct PointCloudOctreePipeline {
    mesh_pipeline: MeshPipeline,
    shader_handle: Handle<Shader>,
    pub layout: BindGroupLayout,
    pub layout_msaa: BindGroupLayout,
    point_cloud_octree_3d_layout: BindGroupLayout,
    point_cloud_material_layout: BindGroupLayout,
}

impl FromWorld for PointCloudOctreePipeline {
    fn from_world(world: &mut World) -> Self {
        let mesh_pipeline = world.resource::<MeshPipeline>();
        let render_device = world.resource::<RenderDevice>();

        Self {
            mesh_pipeline: mesh_pipeline.clone(),
            shader_handle: POINTCLOUD_SHADER_HANDLE,
            layout: render_device.create_bind_group_layout(
                "pcl_attribute_pass_bind_group_layout",
                &BindGroupLayoutEntries::single(
                    ShaderStages::VERTEX,
                    // The texture containing the depth
                    // Binding Depth buffer is not supported in WASM/WebGL
                    texture_2d(TextureSampleType::Float { filterable: false }),
                ),
            ),
            layout_msaa: render_device.create_bind_group_layout(
                "pcl_attribute_pass_bind_group_layout_msaa",
                &BindGroupLayoutEntries::single(
                    ShaderStages::VERTEX,
                    // The texture containing the depth
                    // Binding Depth buffer is not supported in WASM/WebGL
                    texture_2d_multisampled(TextureSampleType::Float { filterable: false }),
                ),
            ),
            point_cloud_octree_3d_layout: PointCloudOctree3dUniform::bind_group_layout(render_device),
            point_cloud_material_layout: PointCloudMaterial::bind_group_layout(render_device),
        }
    }
}

impl SpecializedRenderPipeline for PointCloudOctreePipeline {
    type Key = PointCloudOctreePipelineKey;

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        // We will only use the position of the mesh in our shader so we only need to specify that
        // let mut vertex_attributes = Vec::new();
        // vertex_attributes.push(Mesh::ATTRIBUTE_POSITION.at_shader_location(0));
        // This will automatically generate the correct `VertexBufferLayout` based on the vertex attributes

        // let a = BindGroupLayoutEntries::sequential(
        //     ShaderStages::FRAGMENT,
        //     (
        //         // The texture containing the mask
        //         // We could transmit the complete depth as f32, but we don't need
        //         // Binding Depth buffer is not supported in WASM/WebGL
        //         texture_2d_multisampled(TextureSampleType::Float { filterable: false }),
        //         // The texture containing the rendered point cloud (rgb = weighted sum, a = sum of weights)
        //         texture_2d_multisampled(TextureSampleType::Float { filterable: false }),
        //     ),
        // );

        let vertex_buffer_layout = VertexBufferLayout {
            array_stride: VertexFormat::Float32x3.size(),
            step_mode: VertexStepMode::Vertex,
            attributes: vec![VertexAttribute {
                format: VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            }],
        };

        let instance_buffer_layout = VertexBufferLayout {
            array_stride: size_of::<PointData>() as u64,
            step_mode: VertexStepMode::Instance,
            attributes: vec![
                // Point position
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: 0,
                    shader_location: 3, // shader locations 0-2 are taken up by Position, Normal and UV attributes
                },
                // Point color
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: VertexFormat::Float32x4.size(),
                    shader_location: 4,
                },
            ],
        };

        RenderPipelineDescriptor {
            label: Some("pcl_octree_pipeline".into()),
            // We want to reuse the data from bevy so we use the same bind groups as the default
            // mesh pipeline
            layout: vec![
                // Bind group 0 is the view uniform
                self.mesh_pipeline
                    .get_view_layout(MeshPipelineViewLayoutKey::from(key.mesh_pipeline_key))
                    .clone()
                    .main_layout,
                // Bind group 1 is the mesh uniform
                // self.mesh_pipeline.mesh_layouts.model_only.clone(),
                // Bind group 2 is our point cloud uniform
                self.point_cloud_octree_3d_layout.clone(),
                // Bind group 3 is the point cloud material
                self.point_cloud_material_layout.clone(),
                // Bind group 4 is the depth texture from depth pass
                self.layout.clone(),
            ],
            push_constant_ranges: vec![],
            vertex: VertexState {
                shader: self.shader_handle.clone(),
                shader_defs: vec![],
                entry_point: Some("vertex".into()),
                buffers: vec![vertex_buffer_layout, instance_buffer_layout],
            },
            fragment: Some(FragmentState {
                shader: self.shader_handle.clone(),
                shader_defs: vec!["WEIGHTED_SPLATS".into(), "ATTRIBUTE_PASS".into()],
                entry_point: Some("fragment".into()),
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
                topology: PrimitiveTopology::TriangleList,
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
                count: key.samples,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            zero_initialize_workgroup_memory: false,
        }
    }
}
