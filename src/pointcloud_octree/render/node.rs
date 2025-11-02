use crate::pointcloud_octree::render::phase::PointCloudOctree3dPhase;
use crate::render::attribute_pass::texture::ViewAttributePrepassTextures;
use bevy_camera::Viewport;
use bevy_ecs::prelude::*;
use bevy_ecs::query::QueryItem;
use bevy_log::prelude::*;
use bevy_render::camera::ExtractedCamera;
use bevy_render::render_graph::{NodeRunError, RenderGraphContext, RenderLabel, ViewNode};
use bevy_render::render_phase::{TrackedRenderPass, ViewBinnedRenderPhases};
use bevy_render::render_resource::{CommandEncoderDescriptor, RenderPassDescriptor, StoreOp};
use bevy_render::renderer::RenderContext;
use bevy_render::view::{ExtractedView, ViewDepthTexture, ViewTarget};

// Render label used to order our render graph node that will render our phase
#[derive(RenderLabel, Debug, Clone, Hash, PartialEq, Eq)]
pub struct Pointcloud3dDrawLabel;

#[derive(Default)]
pub struct PointCloudOctree3dDrawNode;
impl ViewNode for PointCloudOctree3dDrawNode {
    type ViewQuery = (
        &'static ExtractedCamera,
        &'static ExtractedView,
        &'static ViewDepthTexture,
        &'static ViewAttributePrepassTextures,
    );

    fn run<'w>(
        &self,
        graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (camera, view, depth_texture, view_prepass_textures): QueryItem<
            'w,
            '_,
            Self::ViewQuery,
        >,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        // First, we need to get our phases resource
        let Some(pointcloud_phases) =
            world.get_resource::<ViewBinnedRenderPhases<PointCloudOctree3dPhase>>()
        else {
            info!("no pointcloud phases");
            return Ok(());
        };

        // Get the view entity from the graph
        let view_entity = graph.view_entity();

        // Get the phase for the current view running our node
        let Some(stencil_phase) = pointcloud_phases.get(&view.retained_view_entity) else {
            info!("no stencil phase");
            return Ok(());
        };

        let color_attachments = vec![
            view_prepass_textures
                .attribute
                .as_ref()
                .map(|attribute_texture| attribute_texture.get_attachment()),
        ];

        let depth_stencil_attachment = Some(depth_texture.get_attachment(StoreOp::Store));

        render_context.add_command_buffer_generation_task(move |render_device| {
            let mut command_encoder =
                render_device.create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("pcl_octree_attribute_pass_command_encoder"),
                });

            let render_pass = command_encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("pcl_octree_attribute_pass"),
                color_attachments: &color_attachments,
                depth_stencil_attachment,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            let mut render_pass = TrackedRenderPass::new(&render_device, render_pass);

            if let Some(viewport) = camera.viewport.as_ref() {
                render_pass.set_camera_viewport(viewport);
            }

            if let Err(err) = stencil_phase.render(&mut render_pass, world, view_entity) {
                error!("Error encountered while rendering the point cloud attribute phase {err:?}");
            }

            drop(render_pass);

            command_encoder.finish()
        });

        // // Render pass setup
        // let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
        //     label: Some("stencil pass"),
        //     // For the purpose of the example, we will write directly to the view target. A real
        //     // stencil pass would write to a custom texture and that texture would be used in later
        //     // passes to render custom effects using it.
        //     color_attachments: &[Some(target.get_color_attachment())],
        //     // We don't bind any depth buffer for this pass
        //     depth_stencil_attachment,
        //     timestamp_writes: None,
        //     occlusion_query_set: None,
        // });
        //
        // if let Some(viewport) =
        //     Viewport::from_viewport_and_override(camera.viewport.as_ref(), resolution_override)
        // {
        //     render_pass.set_camera_viewport(&viewport);
        // }
        //
        // // Render the phase
        // // This will execute each draw functions of each phase items queued in this phase
        // if let Err(err) = stencil_phase.render(&mut render_pass, world, view_entity) {
        //     error!("Error encountered while rendering the pointcloud 3d phase {err:?}");
        // }

        Ok(())
    }
}
