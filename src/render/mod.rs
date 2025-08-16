mod aabb;
pub mod attribute_pass;
pub mod depth_pass;
mod extract;
pub mod material;
pub mod normalize_pass;
mod point_cloud;
mod point_cloud_uniform;

use crate::point_cloud::PointCloud3d;
use crate::render::material::{RenderPointCloudMaterial, RenderPointCloudMaterialLayout};
use crate::render::point_cloud::RenderPointCloud;
use aabb::compute_point_cloud_aabb;
use attribute_pass::AttributePassPlugin;
use bevy_app::prelude::*;
use bevy_asset::prelude::*;
use bevy_asset::{load_internal_asset, weak_handle};
use bevy_ecs::prelude::*;
use bevy_ecs::system::{SystemParamItem, lifetimeless::*};
use bevy_pbr::RenderMeshInstances;
use bevy_render::render_asset::RenderAssetPlugin;
use bevy_render::{
    Render, RenderApp, RenderSet,
    extract_component::ExtractComponentPlugin,
    mesh::{RenderMesh, RenderMeshBufferInfo, allocator::MeshAllocator},
    prelude::*,
    render_asset::RenderAssets,
    render_phase::{PhaseItem, RenderCommand, RenderCommandResult, TrackedRenderPass},
};
use depth_pass::DepthPassPlugin;
use normalize_pass::NormalizePassPlugin;
use point_cloud_uniform::{PointCloudUniformLayout, prepare_point_cloud_uniform};

const POINTCLOUD_SHADER_HANDLE: Handle<Shader> =
    weak_handle!("9c7d8df3-86dd-4412-a9cc-dad5c7916a8c");

const NORMALIZE_SHADER_HANDLE: Handle<Shader> =
    weak_handle!("0e5fffec-7e0b-4b44-8c32-b92d9b99fd58");

pub struct RenderPipelinePlugin;

impl Plugin for RenderPipelinePlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(
            app,
            POINTCLOUD_SHADER_HANDLE,
            "point_cloud.wgsl",
            Shader::from_wgsl
        );

        load_internal_asset!(
            app,
            NORMALIZE_SHADER_HANDLE,
            "normalize.wgsl",
            Shader::from_wgsl
        );

        app.add_plugins(RenderAssetPlugin::<RenderPointCloud>::default())
            .add_plugins(RenderAssetPlugin::<RenderPointCloudMaterial>::default())
            .add_plugins(ExtractComponentPlugin::<PointCloud3d>::default())
            // compute point cloud aabb **before** [`bevy_render::view::calculate_bounds`] to prevent using mesh's aabb.
            .add_systems(
                PostUpdate,
                compute_point_cloud_aabb.before(bevy_render::view::calculate_bounds),
            )
            .sub_app_mut(RenderApp)
            .add_systems(
                Render,
                prepare_point_cloud_uniform.in_set(RenderSet::PrepareResources),
            );

        app.add_plugins((DepthPassPlugin, AttributePassPlugin, NormalizePassPlugin));
    }

    fn finish(&self, app: &mut App) {
        app.sub_app_mut(RenderApp)
            .init_resource::<RenderPointCloudMaterialLayout>()
            .init_resource::<PointCloudUniformLayout>();
    }
}

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
            return RenderCommandResult::Skip;
        };
        let Some(render_point_cloud) = render_point_clouds.get(point_cloud_3d) else {
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
