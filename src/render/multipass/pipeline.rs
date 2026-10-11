use bevy::{
    core_pipeline::FullscreenShader,
    ecs::{
        resource::Resource,
        system::{Commands, Res},
    },
    material::descriptor::{BindGroupLayoutDescriptor, VertexState},
    pbr::MeshPipeline,
    render::render_resource::{binding_types::texture_2d, ShaderStages, TextureSampleType},
};

#[derive(Resource, Clone)]
pub struct FullscreenMaterialPassPipeline {
    pub vertex_state: VertexState,
    pub mesh_pipeline: MeshPipeline,
    pub fullscreen_layout: BindGroupLayoutDescriptor,
}

pub fn init_fullscreen_pass_pipeline(
    mut commands: Commands,
    fullscreen_shader: Res<FullscreenShader>,
    mesh_pipeline: Res<MeshPipeline>,
) {
    let vertex_state = fullscreen_shader.to_vertex_state();

    commands.insert_resource(FullscreenMaterialPassPipeline {
        vertex_state,
        mesh_pipeline: mesh_pipeline.clone(),
        fullscreen_layout: BindGroupLayoutDescriptor::new(
            "fullscreen_bind_group_layout",
            // visible nodes texture
            &[texture_2d(TextureSampleType::Uint).build(2, ShaderStages::VERTEX)],
        ),
    });
}
