mod aabb;
mod components;
mod custom_uniform;
mod point_cloud;

pub use components::PointCloud3d;

use crate::pointcloud::{PointCloud, PointCloudData};
use crate::render::aabb::compute_point_cloud_aabb;
use crate::render::custom_uniform::{CustomUniform, SetCustomUniformGroup, prepare_custom_uniform};
use crate::render::point_cloud::RenderPointCloud;
use bevy_app::prelude::*;
use bevy_asset::prelude::*;
use bevy_asset::{load_internal_asset, weak_handle};
use bevy_core_pipeline::core_3d::{CORE_3D_DEPTH_FORMAT, Transparent3d};
use bevy_ecs::prelude::*;
use bevy_ecs::system::{SystemParamItem, lifetimeless::*};
use bevy_log::prelude::*;
use bevy_pbr::{
    MeshPipeline, MeshPipelineKey, RenderMeshInstances, SetMeshBindGroup, SetMeshViewBindGroup,
};
use bevy_render::render_asset::RenderAssetPlugin;
use bevy_render::renderer::RenderDevice;
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
    sync_world::MainEntity,
    view::ExtractedView,
};

const POINTCLOUD_SHADER_HANDLE: Handle<Shader> =
    weak_handle!("9c7d8df3-86dd-4412-a9cc-dad5c7916a8c");

pub struct RenderPipelinePlugin;

impl Plugin for RenderPipelinePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<PointCloud>()
            .init_asset::<PointCloud>()
            .register_asset_reflect::<PointCloud>()
            .add_plugins(RenderAssetPlugin::<RenderPointCloud>::default())
            .add_plugins(ExtractComponentPlugin::<PointCloud3d>::default())
            // compute point cloud aabb **before** [`bevy_render::view::calculate_bounds`] to prevent using mesh's aabb.
            .add_systems(
                PostUpdate,
                compute_point_cloud_aabb.before(bevy_render::view::calculate_bounds),
            )
            .sub_app_mut(RenderApp)
            .add_render_command::<Transparent3d, DrawCustom>()
            .init_resource::<SpecializedMeshPipelines<CustomPipeline>>()
            .add_systems(Render, queue_custom.in_set(RenderSet::QueueMeshes))
            .add_systems(
                Render,
                prepare_custom_uniform.in_set(RenderSet::PrepareResources),
            );

        load_internal_asset!(
            app,
            POINTCLOUD_SHADER_HANDLE,
            "point_cloud.wgsl",
            Shader::from_wgsl
        );
    }

    fn finish(&self, app: &mut App) {
        app.sub_app_mut(RenderApp).init_resource::<CustomPipeline>();
    }
}

fn queue_custom(
    transparent_3d_draw_functions: Res<DrawFunctions<Transparent3d>>,
    custom_pipeline: Res<CustomPipeline>,
    mut pipelines: ResMut<SpecializedMeshPipelines<CustomPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    meshes: Res<RenderAssets<RenderMesh>>,
    render_mesh_instances: Res<RenderMeshInstances>,
    point_clouds: Query<(Entity, &MainEntity), With<PointCloud3d>>,
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
        for (entity, main_entity) in &point_clouds {
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
pub struct CustomPipeline {
    shader: Handle<Shader>,
    mesh_pipeline: MeshPipeline,
    custom_layout: BindGroupLayout,
}

impl FromWorld for CustomPipeline {
    fn from_world(world: &mut World) -> Self {
        let mesh_pipeline = world.resource::<MeshPipeline>();
        let render_device = world.resource::<RenderDevice>();
        let custom_layout = CustomUniform::bind_group_layout(render_device);

        CustomPipeline {
            shader: POINTCLOUD_SHADER_HANDLE,
            mesh_pipeline: mesh_pipeline.clone(),
            custom_layout,
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

        // add our custom uniform layout
        descriptor.layout.push(self.custom_layout.clone());

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
    SetCustomUniformGroup<2>,
    DrawMeshInstanced,
);

struct DrawMeshInstanced;

impl<P: PhaseItem> RenderCommand<P> for DrawMeshInstanced {
    type Param = (
        SRes<RenderAssets<RenderMesh>>,
        SRes<RenderMeshInstances>,
        SRes<MeshAllocator>,
        SRes<RenderAssets<RenderPointCloud>>,
    );
    type ViewQuery = ();
    type ItemQuery = Read<PointCloud3d>;

    #[inline]
    fn render<'w>(
        item: &P,
        _view: (),
        point_cloud_3d: Option<&'w PointCloud3d>,
        (meshes, render_mesh_instances, mesh_allocator, render_point_clouds): SystemParamItem<
            'w,
            '_,
            Self::Param,
        >,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        // A borrow check workaround.
        let mesh_allocator = mesh_allocator.into_inner();
        let render_point_clouds = render_point_clouds.into_inner();

        let Some(mesh_instance) = render_mesh_instances.render_mesh_queue_data(item.main_entity())
        else {
            return RenderCommandResult::Skip;
        };

        let Some(gpu_mesh) = meshes.into_inner().get(mesh_instance.mesh_asset_id) else {
            return RenderCommandResult::Skip;
        };
        let Some(point_cloud_3d) = point_cloud_3d else {
            warn!("Point cloud 3d not found !");
            return RenderCommandResult::Skip;
        };
        let Some(render_point_cloud) = render_point_clouds.get(point_cloud_3d) else {
            warn!("Point cloud not found !");
            return RenderCommandResult::Skip;
        };

        let Some(vertex_buffer_slice) =
            mesh_allocator.mesh_vertex_slice(&mesh_instance.mesh_asset_id)
        else {
            return RenderCommandResult::Skip;
        };

        pass.set_vertex_buffer(0, vertex_buffer_slice.buffer.slice(..));
        pass.set_vertex_buffer(1, render_point_cloud.buffer.slice(..));

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
                    0..render_point_cloud.length as u32,
                );
            }
            RenderMeshBufferInfo::NonIndexed => {
                pass.draw(
                    vertex_buffer_slice.range,
                    0..render_point_cloud.length as u32,
                );
            }
        }
        RenderCommandResult::Success
    }
}
