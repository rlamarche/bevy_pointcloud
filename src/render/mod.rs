//! A shader that renders a mesh multiple times in one draw call.
//!
//! Bevy will automatically batch and instance your meshes assuming you use the same
//! `Handle<Material>` and `Handle<Mesh>` for all of your instances.
//!
//! This example is intended for advanced users and shows how to make a custom instancing
//! implementation using bevy's low level rendering api.
//! It's generally recommended to try the built-in instancing before going with this approach.

use crate::point_cloud::{GpuPointCloudData, PointCloudData, PointCloudInstance};
use bevy_app::prelude::*;
use bevy_asset::prelude::*;
use bevy_asset::{load_internal_asset, weak_handle};
use bevy_core_pipeline::core_3d::{CORE_3D_DEPTH_FORMAT, Transparent3d};
use bevy_ecs::{
    prelude::*,
    system::{SystemParamItem, lifetimeless::*},
};
use bevy_log::prelude::*;
use bevy_math::prelude::*;
use bevy_pbr::{
    MeshPipeline, MeshPipelineKey, RenderMeshInstances, SetMeshBindGroup, SetMeshViewBindGroup,
};
use bevy_render::{
    Render, RenderApp, RenderSet,
    extract_component::ExtractComponentPlugin,
    mesh::{MeshVertexBufferLayoutRef, RenderMesh, RenderMeshBufferInfo, allocator::MeshAllocator},
    prelude::*,
    render_asset::RenderAssets,
    render_phase::{
        AddRenderCommand, DrawFunctions, PhaseItem, PhaseItemExtraIndex, RenderCommand,
        RenderCommandResult, SetItemPipeline, TrackedRenderPass, ViewSortedRenderPhases,
    },
    render_resource::*,
    renderer::RenderDevice,
    renderer::RenderQueue,
    sync_world::MainEntity,
    view::ExtractedView,
};

const POINTCLOUD_SHADER_HANDLE: Handle<Shader> =
    weak_handle!("9c7d8df3-86dd-4412-a9cc-dad5c7916a8c");

pub struct RenderPipelinePlugin;

impl Plugin for RenderPipelinePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractComponentPlugin::<PointCloudInstance>::default());

        app.sub_app_mut(RenderApp)
            .add_render_command::<Transparent3d, DrawCustom>()
            .init_resource::<SpecializedMeshPipelines<CustomPipeline>>()
            .add_systems(
                Render,
                (
                    queue_custom.in_set(RenderSet::QueueMeshes),
                    prepare_instance_buffers.in_set(RenderSet::PrepareResources),
                ),
            );

        load_internal_asset!(
            app,
            POINTCLOUD_SHADER_HANDLE,
            "pointcloud.wgsl",
            Shader::from_wgsl
        );
    }

    fn finish(&self, app: &mut App) {
        app.sub_app_mut(RenderApp).init_resource::<CustomPipeline>();
    }
}

fn prepare_instance_buffers(
    mut commands: Commands,
    query: Query<(Entity, &PointCloudInstance, Option<&GpuPointCloudData>)>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    for (entity, instance_data, instance_buffer_opt) in &query {
        if let Some(instance_buffer) = instance_buffer_opt {
            // Buffer exists, we update it if necessary
            // render_queue.write_buffer(
            //     &instance_buffer.buffer,
            //     0,
            //     bytemuck::cast_slice(instance_data.as_slice()),
            // );
        } else {
            // Buffer not existing, create it
            let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: Some("instance data buffer"),
                contents: bytemuck::cast_slice(instance_data.as_slice()),
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            });
            commands.entity(entity).insert(GpuPointCloudData {
                buffer,
                length: instance_data.len(),
            });
        }
    }
}

fn queue_custom(
    transparent_3d_draw_functions: Res<DrawFunctions<Transparent3d>>,
    custom_pipeline: Res<CustomPipeline>,
    mut pipelines: ResMut<SpecializedMeshPipelines<CustomPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    meshes: Res<RenderAssets<RenderMesh>>,
    render_mesh_instances: Res<RenderMeshInstances>,
    material_meshes: Query<(Entity, &MainEntity), With<PointCloudInstance>>,
    mut transparent_render_phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    views: Query<(&ExtractedView, &Msaa)>,
) {
    let draw_custom = transparent_3d_draw_functions.read().id::<DrawCustom>();

    for (view, msaa) in &views {
        let Some(transparent_phase) = transparent_render_phases.get_mut(&view.retained_view_entity)
        else {
            continue;
        };

        let msaa_key = MeshPipelineKey::from_msaa_samples(msaa.samples());

        let view_key = msaa_key | MeshPipelineKey::from_hdr(view.hdr);
        let rangefinder = view.rangefinder3d();
        for (entity, main_entity) in &material_meshes {
            // info!("Queueing entity {}", entity.index());
            let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(*main_entity)
            else {
                continue;
            };
            let Some(mesh) = meshes.get(mesh_instance.mesh_asset_id) else {
                continue;
            };
            let key =
                view_key | MeshPipelineKey::from_primitive_topology(mesh.primitive_topology());
            let pipeline = pipelines
                .specialize(&pipeline_cache, &custom_pipeline, key, &mesh.layout)
                .unwrap();
            // info!("Add phase entity {}", entity.index());
            transparent_phase.add(Transparent3d {
                entity: (entity, *main_entity),
                pipeline,
                draw_function: draw_custom,
                distance: rangefinder.distance_translation(&mesh_instance.translation),
                batch_range: 0..1,
                extra_index: PhaseItemExtraIndex::None,
                indexed: true,
            });
        }
    }
}

#[derive(Resource)]
struct CustomPipeline {
    shader: Handle<Shader>,
    mesh_pipeline: MeshPipeline,
}

impl FromWorld for CustomPipeline {
    fn from_world(world: &mut World) -> Self {
        let mesh_pipeline = world.resource::<MeshPipeline>();

        CustomPipeline {
            shader: POINTCLOUD_SHADER_HANDLE,
            mesh_pipeline: mesh_pipeline.clone(),
        }
    }
}

impl SpecializedMeshPipeline for CustomPipeline {
    type Key = MeshPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayoutRef,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut descriptor = self.mesh_pipeline.specialize(key, layout)?;

        descriptor.depth_stencil = Some(DepthStencilState {
            format: CORE_3D_DEPTH_FORMAT,
            depth_write_enabled: true,
            depth_compare: CompareFunction::GreaterEqual,
            stencil: StencilState::default(),
            bias: DepthBiasState::default(),
        });
        descriptor.vertex.shader = self.shader.clone();
        descriptor.vertex.buffers.push(VertexBufferLayout {
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
        });
        let fragment_descriptor = descriptor.fragment.as_mut().unwrap();
        fragment_descriptor.shader = self.shader.clone();
        // fragment_descriptor.targets = vec![Some(ColorTargetState {
        //     format: TextureFormat::bevy_default(),
        //     // blend: Some(BlendState::ALPHA_BLENDING),
        //     // blend: Some(BlendState {
        //     //     color: BlendComponent {
        //     //         src_factor: BlendFactor::SrcAlpha,
        //     //         dst_factor: BlendFactor::OneMinusSrcAlpha,
        //     //         operation: BlendOperation::Add,
        //     //     },
        //     //     alpha: BlendComponent {
        //     //         src_factor: BlendFactor::One,
        //     //         dst_factor: BlendFactor::OneMinusSrcAlpha,
        //     //         operation: BlendOperation::Add,
        //     //     },
        //     // }),
        //     blend: Some(BlendState {
        //         color: BlendComponent {
        //             src_factor: BlendFactor::One,
        //             dst_factor: BlendFactor::One,
        //             operation: BlendOperation::Add,
        //         },
        //         alpha: BlendComponent {
        //             src_factor: BlendFactor::One,
        //             dst_factor: BlendFactor::One,
        //             operation: BlendOperation::Add,
        //         },
        //     }),
        //     write_mask: ColorWrites::ALL,
        // })];

        Ok(descriptor)
    }
}

type DrawCustom = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshBindGroup<1>,
    DrawMeshInstanced,
);

struct DrawMeshInstanced;

impl<P: PhaseItem> RenderCommand<P> for DrawMeshInstanced {
    type Param = (
        SRes<RenderAssets<RenderMesh>>,
        SRes<RenderMeshInstances>,
        SRes<MeshAllocator>,
    );
    type ViewQuery = ();
    type ItemQuery = Read<GpuPointCloudData>;

    #[inline]
    fn render<'w>(
        item: &P,
        _view: (),
        gpu_point_cloud_buffer: Option<&'w GpuPointCloudData>,
        (meshes, render_mesh_instances, mesh_allocator): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        // A borrow check workaround.
        let mesh_allocator = mesh_allocator.into_inner();

        let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(item.main_entity())
        else {
            return RenderCommandResult::Skip;
        };
        let Some(gpu_mesh) = meshes.into_inner().get(mesh_instance.mesh_asset_id) else {
            return RenderCommandResult::Skip;
        };
        let Some(gpu_point_cloud_buffer) = gpu_point_cloud_buffer else {
            info!("Skip !!");
            return RenderCommandResult::Skip;
        };
        // info!("Render mesh with {} points", instance_buffer.length);

        let Some(vertex_buffer_slice) =
            mesh_allocator.mesh_vertex_slice(&mesh_instance.mesh_asset_id)
        else {
            return RenderCommandResult::Skip;
        };

        pass.set_vertex_buffer(0, vertex_buffer_slice.buffer.slice(..));
        pass.set_vertex_buffer(1, gpu_point_cloud_buffer.buffer.slice(..));

        match &gpu_mesh.buffer_info {
            RenderMeshBufferInfo::Indexed {
                index_format,
                count,
            } => {
                let Some(index_buffer_slice) =
                    mesh_allocator.mesh_index_slice(&mesh_instance.mesh_asset_id)
                else {
                    return RenderCommandResult::Skip;
                };

                pass.set_index_buffer(index_buffer_slice.buffer.slice(..), 0, *index_format);
                pass.draw_indexed(
                    index_buffer_slice.range.start..(index_buffer_slice.range.start + count),
                    vertex_buffer_slice.range.start as i32,
                    0..gpu_point_cloud_buffer.length as u32,
                );
            }
            RenderMeshBufferInfo::NonIndexed => {
                pass.draw(
                    vertex_buffer_slice.range,
                    0..gpu_point_cloud_buffer.length as u32,
                );
            }
        }
        RenderCommandResult::Success
    }
}
