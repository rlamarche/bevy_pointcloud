use bevy::{
    material::descriptor::BindGroupLayoutDescriptor,
    render::{
        render_resource::{BindGroupLayoutEntries, ShaderStages},
        renderer::{RenderAdapter, RenderDevice},
    },
};

/// Individual layout entries.
mod layout_entry {
    use bevy::render::{
        render_resource::{BindGroupLayoutEntryBuilder, GpuArrayBuffer, ShaderStages},
        settings::WgpuLimits,
    };

    use crate::PointCloudUniform;

    pub(super) fn model(limits: &WgpuLimits) -> BindGroupLayoutEntryBuilder {
        GpuArrayBuffer::<PointCloudUniform>::binding_layout(limits)
            .visibility(ShaderStages::VERTEX_FRAGMENT)
    }
}

/// All possible [`BindGroupLayout`]s in bevy's default mesh shader (`mesh.wgsl`).
#[derive(Clone)]
pub struct PointCloudLayouts {
    /// The mesh model uniform (transform) and nothing else.
    pub model_only: BindGroupLayoutDescriptor,
}

impl PointCloudLayouts {
    /// Prepare the layouts used by the default bevy [`Mesh`].
    ///
    /// [`Mesh`]: bevy_mesh::Mesh
    pub fn new(render_device: &RenderDevice, _render_adapter: &RenderAdapter) -> Self {
        PointCloudLayouts {
            model_only: Self::model_only_layout(render_device),
        }
    }

    // ---------- create individual BindGroupLayouts ----------

    fn model_only_layout(render_device: &RenderDevice) -> BindGroupLayoutDescriptor {
        BindGroupLayoutDescriptor::new(
            "point_cloud_layout",
            &BindGroupLayoutEntries::single(
                ShaderStages::empty(),
                layout_entry::model(&render_device.limits()),
            ),
        )
    }
}
