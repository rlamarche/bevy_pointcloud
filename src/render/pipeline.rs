use bevy::{
    asset::{load_embedded_asset, AssetServer, Handle},
    ecs::{
        resource::Resource,
        system::Commands,
        world::{FromWorld, World},
    },
    material::descriptor::{FragmentState, RenderPipelineDescriptor, VertexState},
    mesh::{Mesh, MeshVertexBufferLayoutRef},
    pbr::{MeshPipeline, MeshPipelineViewLayoutKey},
    render::render_resource::{
        BlendState, ColorTargetState, ColorWrites, CompareFunction, DepthBiasState,
        DepthStencilState, MultisampleState, PrimitiveState, SpecializedRenderPipeline,
        StencilState, TextureFormat, VertexStepMode,
    },
    shader::Shader,
    utils::default,
};

pub fn init_point_cloud_pipeline(mut commands: Commands) {
    commands.init_resource::<PointCloudPipeline>();
}

#[derive(Resource)]
pub struct PointCloudPipeline {
    shader: Handle<Shader>,
    mesh_pipeline: MeshPipeline,
}

impl FromWorld for PointCloudPipeline {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>();
        let shader = load_embedded_asset!(asset_server, "shaders/point_cloud.wgsl");

        let mesh_pipeline = world.resource::<MeshPipeline>();

        Self {
            shader,
            mesh_pipeline: mesh_pipeline.clone(),
        }
    }
}

impl SpecializedRenderPipeline for PointCloudPipeline {
    type Key = (
        MeshVertexBufferLayoutRef,
        MeshVertexBufferLayoutRef,
        MeshPipelineViewLayoutKey,
    );

    fn specialize(&self, key: Self::Key) -> RenderPipelineDescriptor {
        let (quad_layout_ref, points_layout_ref, mesh_pipeline_view_layout_key) = key;

        let view_layout = self
            .mesh_pipeline
            .get_view_layout(mesh_pipeline_view_layout_key);

        let quad_layout = quad_layout_ref
            .0
            .get_layout(&[
                Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
                Mesh::ATTRIBUTE_UV_0.at_shader_location(1),
            ])
            .unwrap();

        let mut points_layout = points_layout_ref
            .0
            .get_layout(&[
                Mesh::ATTRIBUTE_POSITION.at_shader_location(10),
                Mesh::ATTRIBUTE_COLOR.at_shader_location(11),
            ])
            .unwrap();
        points_layout.step_mode = VertexStepMode::Instance; // On change le step_mode ici !

        let bind_group_layout = vec![
            view_layout.main_layout.clone(),
            view_layout.binding_array_layout.clone(),
        ];

        RenderPipelineDescriptor {
            label: Some("point_cloud_pipeline".into()),
            layout: bind_group_layout,
            vertex: VertexState {
                shader: self.shader.clone(),
                shader_defs: vec![],
                entry_point: Some("vertex".into()),
                buffers: vec![quad_layout, points_layout],
            },
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                shader_defs: vec![],
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState {
                    format: TextureFormat::Rgba8UnormSrgb, // Format HDR standard de Bevy
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState::default(),
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(CompareFunction::GreaterEqual),
                stencil: StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            multisample: MultisampleState::default(),
            ..default()
        }
    }
}
