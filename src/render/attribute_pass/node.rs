use super::AttributePass3d;
use super::texture::ViewAttributePrepassTextures;
use bevy_ecs::{prelude::*, query::QueryItem};
use bevy_log::{error, warn};
use bevy_render::render_phase::TrackedRenderPass;
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

#[derive(RenderLabel, Debug, Clone, Hash, PartialEq, Eq)]
pub struct AttributePassLabel;

#[derive(Default)]
pub struct AttributePassNode;
impl ViewNode for AttributePassNode {
    type ViewQuery = (
        Option<&'static ExtractedCamera>,
        Option<&'static ExtractedView>,
        Option<&'static ViewDepthTexture>,
        Option<&'static ViewAttributePrepassTextures>,
    );

    fn run<'w>(
        &self,
        graph: &mut RenderGraphContext,
        render_context: &mut RenderContext<'w>,
        (camera, view, depth, view_prepass_textures): QueryItem<'w, Self::ViewQuery>,
        world: &'w World,
    ) -> Result<(), NodeRunError> {
        // First, we need to get our phases resource
        let Some(stencil_phases) = world.get_resource::<ViewSortedRenderPhases<AttributePass3d>>()
        else {
            return Ok(());
        };

        let Some(view_prepass_textures) = view_prepass_textures else {
            warn!("ViewAttributePrepassTextures missing.");
            return Ok(());
        };

        let color_attachments = vec![
            view_prepass_textures
                .attribute
                .as_ref()
                .map(|attribute_texture| attribute_texture.get_attachment()),
        ];

        let view_entity = graph.view_entity();

        let Some(view) = view else {
            warn!("ExtractedView missing.");
            return Ok(());
        };

        let Some(stencil_phase) = stencil_phases.get(&view.retained_view_entity) else {
            return Ok(());
        };

        let Some(depth) = depth else {
            warn!("ViewDepthTexture missing.");
            return Ok(());
        };

        let depth_stencil_attachment = Some(depth.get_attachment(StoreOp::Store));

        render_context.add_command_buffer_generation_task(move |render_device| {
            let mut command_encoder =
                render_device.create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("pcl_attribute_pass_command_encoder"),
                });

            let render_pass = command_encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("pcl_attribute_pass"),
                color_attachments: &color_attachments,
                depth_stencil_attachment,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            let mut render_pass = TrackedRenderPass::new(&render_device, render_pass);

            let Some(camera) = camera else {
                warn!("ExtractedCamera missing.");
                drop(render_pass);
                return command_encoder.finish();
            };

            if let Some(viewport) = camera.viewport.as_ref() {
                render_pass.set_camera_viewport(viewport);
            }

            if let Err(err) = stencil_phase.render(&mut render_pass, world, view_entity) {
                error!("Error encountered while rendering the point cloud attribute phase {err:?}");
            }

            drop(render_pass);

            command_encoder.finish()
        });

        Ok(())
    }
}
