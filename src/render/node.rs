use crate::render::depth_pass::phase::PointCloud3dDepthPhase;
use crate::render::depth_pass::texture::ViewDepthPrepassTextures;
use bevy_ecs::{prelude::*, query::QueryItem};
use bevy_log::prelude::*;
use bevy_render::render_phase::{TrackedRenderPass, ViewBinnedRenderPhases};
use bevy_render::render_resource::{CommandEncoderDescriptor, StoreOp};
use bevy_render::view::ViewDepthTexture;
use bevy_render::{
    camera::ExtractedCamera,
    render_graph::{NodeRunError, RenderGraphContext, RenderLabel, ViewNode},
    render_phase::ViewSortedRenderPhases,
    render_resource::RenderPassDescriptor,
    renderer::RenderContext,
    view::ExtractedView,
};
use crate::pointcloud_octree::render::attribute_pass::phase::PointCloudOctree3dAttributePhase;
use crate::pointcloud_octree::render::depth_pass::phase::PointCloudOctree3dDepthPhase;
use crate::render::attribute_pass::phase::PointCloud3dAttributePhase;
use crate::render::attribute_pass::texture::ViewAttributePrepassTextures;

#[derive(RenderLabel, Debug, Clone, Hash, PartialEq, Eq)]
pub struct OpaquePointCloud3dLabel;

#[derive(Default)]
pub struct OpaquePointCloud3d;
impl ViewNode for OpaquePointCloud3d {
    type ViewQuery = (
        &'static ExtractedCamera,
        &'static ExtractedView,
        &'static ViewDepthTexture,
        &'static ViewDepthPrepassTextures,
        &'static ViewAttributePrepassTextures,
        &'static ViewDepthTexture,
    );

    fn run<'w>(
        &self,
        graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (camera, view, view_depth_texture, view_depth_prepass_textures, view_attribute_prepass_textures, depth): QueryItem<
            'w,
            '_,
            Self::ViewQuery,
        >,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        // First, we need to get our phases resource
        let Some(point_cloud_3d_depth_phases) =
            world.get_resource::<ViewBinnedRenderPhases<PointCloud3dDepthPhase>>()
        else {
            info!("no pointcloud depth phases");
            return Ok(());
        };

        let Some(point_cloud_octree_3d_depth_phases) =
            world.get_resource::<ViewBinnedRenderPhases<PointCloudOctree3dDepthPhase>>()
        else {
            info!("no pointcloud depth phases");
            return Ok(());
        };

        let Some(point_cloud_3d_attribute_phases) =
            world.get_resource::<ViewBinnedRenderPhases<PointCloud3dAttributePhase>>()
        else {
            info!("no pointcloud attribute phases");
            return Ok(());
        };

        let Some(point_cloud_octree_3d_attribute_phases) =
            world.get_resource::<ViewBinnedRenderPhases<PointCloudOctree3dAttributePhase>>()
        else {
            info!("no pointcloud attribute phases");
            return Ok(());
        };

        let view_entity = graph.view_entity();

        // Get the phase for the current view running our node
        let Some(point_cloud_3d_depth_phase) = point_cloud_3d_depth_phases.get(&view.retained_view_entity)
        else {
            info!("no pointcloud phase");
            return Ok(());
        };

        let Some(point_cloud_octree_3d_depth_phase) = point_cloud_octree_3d_depth_phases.get(&view.retained_view_entity)
        else {
            info!("no pointcloud phase");
            return Ok(());
        };

        let Some(point_cloud_3d_attribute_phase) = point_cloud_3d_attribute_phases.get(&view.retained_view_entity)
        else {
            info!("no pointcloud phase");
            return Ok(());
        };

        let Some(point_cloud_octree_3d_attribute_phase) = point_cloud_octree_3d_attribute_phases.get(&view.retained_view_entity)
        else {
            info!("no pointcloud phase");
            return Ok(());
        };


        let depth_color_attachments = vec![
            view_depth_prepass_textures
                .depth
                .as_ref()
                .map(|attribute_texture| attribute_texture.get_attachment()),
        ];

        let depth_stencil_attachment = Some(view_depth_texture.get_attachment(StoreOp::Store));



        let attribute_color_attachments = vec![
            view_attribute_prepass_textures
                .attribute
                .as_ref()
                .map(|attribute_texture| attribute_texture.get_attachment()),
        ];

        let attribute_stencil_attachment = Some(depth.get_attachment(StoreOp::Store));

        render_context.add_command_buffer_generation_task(move |render_device| {
            // Command encoder setup
            let mut command_encoder =
                render_device.create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("pcl_opaque_command_encoder"),
                });

            // Render pass setup
            let depth_render_pass = command_encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("pcl_depth_pass"),
                // In the depth phase, we write a depth mask to reuse it in normalize pass
                // We could use the depth buffer, but with WebGL we can't bind it in a shader
                color_attachments: &depth_color_attachments,
                // We store the depth for usage in attribute pass
                depth_stencil_attachment: depth_stencil_attachment.clone(),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            let mut render_pass = TrackedRenderPass::new(&render_device, depth_render_pass);

            if let Some(viewport) = camera.viewport.as_ref() {
                render_pass.set_camera_viewport(viewport);
            }

            if let Err(err) = point_cloud_3d_depth_phase.render(&mut render_pass, world, view_entity) {
                error!("Error encountered while rendering the point cloud depth phase {err:?}");
            }

            drop(render_pass);



            let depth_render_pass = command_encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("pcl_octree_depth_pass"),
                // In the depth phase, we write a depth mask to reuse it in normalize pass
                // We could use the depth buffer, but with WebGL we can't bind it in a shader
                color_attachments: &depth_color_attachments,
                // We store the depth for usage in attribute pass
                depth_stencil_attachment,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            let mut render_pass = TrackedRenderPass::new(&render_device, depth_render_pass);

            if let Some(viewport) = camera.viewport.as_ref() {
                render_pass.set_camera_viewport(viewport);
            }

            if let Err(err) = point_cloud_octree_3d_depth_phase.render(&mut render_pass, world, view_entity) {
                error!("Error encountered while rendering the point cloud octree depth phase {err:?}");
            }

            drop(render_pass);


            let render_pass = command_encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("pcl_attribute_pass"),
                color_attachments: &attribute_color_attachments,
                depth_stencil_attachment: attribute_stencil_attachment.clone(),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            let mut render_pass = TrackedRenderPass::new(&render_device, render_pass);

            if let Some(viewport) = camera.viewport.as_ref() {
                render_pass.set_camera_viewport(viewport);
            }

            if let Err(err) = point_cloud_3d_attribute_phase.render(&mut render_pass, world, view_entity) {
                error!("Error encountered while rendering the point cloud attribute phase {err:?}");
            }

            drop(render_pass);

            let render_pass = command_encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("pcl_octree_attribute_pass"),
                color_attachments: &attribute_color_attachments,
                depth_stencil_attachment: attribute_stencil_attachment,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            let mut render_pass = TrackedRenderPass::new(&render_device, render_pass);

            if let Err(err) = point_cloud_octree_3d_attribute_phase.render(&mut render_pass, world, view_entity) {
                error!("Error encountered while rendering the point cloud octree attribute phase {err:?}");
            }

            drop(render_pass);

            command_encoder.finish()
        });

        Ok(())
    }
}
