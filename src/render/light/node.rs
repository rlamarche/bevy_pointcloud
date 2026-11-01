use bevy::{
    ecs::prelude::*,
    log::error,
    pbr::{ShadowView, ViewLightEntities},
    render::{
        diagnostic::RecordDiagnostics,
        occlusion_culling::OcclusionCulling,
        render_resource::*,
        renderer::{RenderContext, ViewQuery},
        view::ExtractedView,
    },
};

#[cfg(feature = "trace")]
use tracing::info_span;

use crate::render::{render_phase::ViewBinnedChunksRenderPhases, ShadowChunk};

pub const EARLY_SHADOW_PASS: bool = false;
pub const LATE_SHADOW_PASS: bool = true;

/// Renders the shadow maps that aren't associated with a specific view.
///
/// At present, these consist of the point and spot light shadow maps.
pub fn shared_shadow_chunk_pass<const IS_LATE: bool>(
    world: &World,
    view_light_query: ViewQuery<(Entity, &ShadowView, &ExtractedView, Has<OcclusionCulling>)>,
    shadow_render_phases: Res<ViewBinnedChunksRenderPhases<ShadowChunk>>,
    mut ctx: RenderContext,
) {
    let (view_light_entity, view_light, extracted_light_view, occlusion_culling) =
        view_light_query.into_inner();
    view_shadow_chunk_pass::<IS_LATE>(
        view_light_entity,
        view_light,
        extracted_light_view,
        occlusion_culling,
        world,
        &shadow_render_phases,
        &mut ctx,
    );
}

/// Renders the shadow maps that are associated with a specific view.
///
/// At present, these consist of the directional light shadows.
pub fn per_view_shadow_chunk_pass<const IS_LATE: bool>(
    world: &World,
    view: ViewQuery<&ViewLightEntities>,
    view_light_query: Query<(&ShadowView, &ExtractedView, Has<OcclusionCulling>)>,
    shadow_render_phases: Res<ViewBinnedChunksRenderPhases<ShadowChunk>>,
    mut ctx: RenderContext,
) {
    let view_lights = view.into_inner();

    let diagnostics = ctx.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let time_span = diagnostics.time_span(ctx.command_encoder(), "per_view_shadow_chunk_pass");

    for view_light_entity in view_lights.lights.iter().copied() {
        let Ok((view_light, extracted_light_view, occlusion_culling)) =
            view_light_query.get(view_light_entity)
        else {
            continue;
        };

        // Draw the shadow map.
        view_shadow_chunk_pass::<IS_LATE>(
            view_light_entity,
            view_light,
            extracted_light_view,
            occlusion_culling,
            world,
            &shadow_render_phases,
            &mut ctx,
        );
    }

    time_span.end(ctx.command_encoder());
}

/// A common helper function to render a shadow map.
fn view_shadow_chunk_pass<const IS_LATE: bool>(
    view_light_entity: Entity,
    view_light: &ShadowView,
    extracted_light_view: &ExtractedView,
    occlusion_culling: bool,
    world: &World,
    shadow_render_phases: &ViewBinnedChunksRenderPhases<ShadowChunk>,
    ctx: &mut RenderContext,
) {
    if IS_LATE && !occlusion_culling {
        return;
    }

    let Some(shadow_phase) = shadow_render_phases.get(&extracted_light_view.retained_view_entity)
    else {
        return;
    };

    #[cfg(feature = "trace")]
    let _shadow_pass_span = info_span!("", "{}", view_light.pass_name).entered();

    let depth_stencil_attachment = Some(view_light.depth_attachment.get_attachment(StoreOp::Store));

    let mut render_pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some(&view_light.pass_name),
        color_attachments: &[],
        depth_stencil_attachment,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });

    if let Err(err) = shadow_phase.render(&mut render_pass, world, view_light_entity) {
        error!("Error encountered while rendering the shadow phase {err:?}");
    }
}
