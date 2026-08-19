use bevy::{
    asset::{load_embedded_asset, AssetServer, Handle},
    core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT,
    ecs::{
        resource::Resource,
        system::Commands,
        world::{FromWorld, World},
    },
    material::{
        descriptor::{
            BindGroupLayoutDescriptor, FragmentState, RenderPipelineDescriptor, VertexState,
        },
        specialize::SpecializedMeshPipelineError,
    },
    mesh::{Mesh, MeshVertexBufferLayoutRef},
    pbr::{
        setup_morph_and_skinning_defs, MeshPipeline, MeshPipelineKey,
        TONEMAPPING_LUT_SAMPLER_BINDING_INDEX, TONEMAPPING_LUT_TEXTURE_BINDING_INDEX,
    },
    render::render_resource::{
        binding_types::{texture_2d, uniform_buffer},
        BindGroupLayoutEntries, BlendComponent, BlendFactor, BlendOperation, BlendState,
        ColorTargetState, ColorWrites, CompareFunction, DepthBiasState, DepthStencilState, Face,
        MultisampleState, PrimitiveState, ShaderStages, StencilFaceState, StencilState,
        VertexStepMode,
    },
    shader::{Shader, ShaderDefVal},
    utils::default,
};
use wgpu::TextureSampleType;

use crate::{
    render::pipeline_specializer::SpecializedPointCloudPipeline, PointCloudUniform,
    SplatPipelineKey,
};

pub(crate) const IRRADIANCE_VOLUMES_ARE_USABLE: bool = cfg!(not(target_arch = "wasm32"));

pub fn init_point_cloud_pipeline(mut commands: Commands) {
    commands.init_resource::<PointCloudPipeline>();
}

#[derive(Resource, Clone)]
pub struct PointCloudPipeline {
    shader: Handle<Shader>,
    mesh_pipeline: MeshPipeline,
    pub empty_layout: BindGroupLayoutDescriptor,
    pub point_cloud_uniform_layout: BindGroupLayoutDescriptor,
    pub point_cloud_octree_visible_nodes_layout: BindGroupLayoutDescriptor,
}

impl FromWorld for PointCloudPipeline {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>();
        let mesh_pipeline = world.resource::<MeshPipeline>();

        Self {
            shader: load_embedded_asset!(asset_server, "pointcloud.wgsl"),
            mesh_pipeline: mesh_pipeline.clone(),
            empty_layout: BindGroupLayoutDescriptor::new("pointcloud_empty_layout", &[]),
            point_cloud_uniform_layout: BindGroupLayoutDescriptor::new(
                "point_cloud_uniform_layout",
                &BindGroupLayoutEntries::single(
                    ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                    uniform_buffer::<PointCloudUniform>(false),
                ),
            ),
            point_cloud_octree_visible_nodes_layout: BindGroupLayoutDescriptor {
                label: "point_cloud_octree_visible_nodes_layout".into(),
                entries: BindGroupLayoutEntries::single(
                    ShaderStages::VERTEX,
                    texture_2d(TextureSampleType::Uint),
                )
                .to_vec(),
            },
        }
    }
}

impl SpecializedPointCloudPipeline for PointCloudPipeline {
    type Key = (MeshPipelineKey, SplatPipelineKey);

    fn specialize(
        &self,
        (key, splat_key): Self::Key,
        splat_layout: &MeshVertexBufferLayoutRef,
        instance_layout: &MeshVertexBufferLayoutRef,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut shader_defs = Vec::new();

        // Let the shader code know that it's running in a mesh pipeline.
        shader_defs.push("MESH_PIPELINE".into());

        shader_defs.push("VERTEX_OUTPUT_INSTANCE_INDEX".into());

        // Splat settings

        // Evaluate multi-bit point size mode using the mask
        match splat_key & SplatPipelineKey::POINT_SIZE_MODE_MASK {
            SplatPipelineKey::POINT_SIZE_MODE_SCREEN_PIXELS => {
                shader_defs.push("POINT_SIZE_MODE_SCREEN_PIXELS".into());
            }
            SplatPipelineKey::POINT_SIZE_MODE_SCREEN_LOCAL => {
                shader_defs.push("POINT_SIZE_MODE_SCREEN_LOCAL".into());
            }
            SplatPipelineKey::POINT_SIZE_MODE_WORLD => {
                shader_defs.push("POINT_SIZE_MODE_WORLD".into());
            }
            SplatPipelineKey::POINT_SIZE_MODE_LOCAL => {
                shader_defs.push("POINT_SIZE_MODE_LOCAL".into());
            }
            _ => unreachable!("Invalid point size mode bits state encountered in pipeline key."),
        }

        // Evaluate single boolean flags
        for (flags, shader_def) in [
            (SplatPipelineKey::ADAPTIVE_POINT_SIZE, "ADAPTIVE_POINT_SIZE"),
            (SplatPipelineKey::SPLAT_RADIUS, "SPLAT_RADIUS"),
            (
                SplatPipelineKey::SPLAT_ORIENTATION_FACE_NORMAL,
                "SPLAT_ORIENTATION_FACE_NORMAL",
            ),
            (SplatPipelineKey::UV_TRANSFORM, "SPLAT_UV_TRANSFORM"),
            (SplatPipelineKey::IS_OCTREE, "IS_OCTREE"),
        ] {
            if splat_key.intersects(flags) {
                shader_defs.push(shader_def.into());
            }
        }

        // Evaluate multi-bit point size mode using the mask
        match splat_key & SplatPipelineKey::UV_MAPPING_MASK {
            SplatPipelineKey::UV_MAPPING_COMBINED => {
                shader_defs.push("UV_MAPPING_COMBINED".into());
            }
            SplatPipelineKey::UV_MAPPING_PLANAR => {
                shader_defs.push("UV_MAPPING_PLANAR".into());
            }
            SplatPipelineKey::UV_MAPPING_POINT_CLOUD => {
                shader_defs.push("UV_MAPPING_POINT_CLOUD".into());
            }
            SplatPipelineKey::UV_MAPPING_POINT_SHAPE => {
                shader_defs.push("UV_MAPPING_POINT_SHAPE".into());
            }
            _ => unreachable!("Invalid uv mapping mode bits state encountered in pipeline key."),
        }

        // construct the shape layout vertex buffer layout

        let mut shape_vertex_attributes = Vec::new();
        if splat_layout.0.contains(Mesh::ATTRIBUTE_POSITION) {
            shader_defs.push("SHAPE_POSITIONS".into());
            shape_vertex_attributes.push(Mesh::ATTRIBUTE_POSITION.at_shader_location(0));
        }
        if splat_layout.0.contains(Mesh::ATTRIBUTE_NORMAL) {
            shader_defs.push("VERTEX_NORMALS".into());
            shader_defs.push("SHAPE_NORMALS".into());
            shape_vertex_attributes.push(Mesh::ATTRIBUTE_NORMAL.at_shader_location(1));
        }

        if splat_layout.0.contains(Mesh::ATTRIBUTE_UV_0) {
            shader_defs.push("SHAPE_UVS".into());
            shader_defs.push("SHAPE_UVS_A".into());
            shape_vertex_attributes.push(Mesh::ATTRIBUTE_UV_0.at_shader_location(2));
        }

        let vertex_buffer_layout = splat_layout.0.get_layout(&shape_vertex_attributes)?;

        // Now the mesh (instances)
        let mut vertex_attributes = Vec::new();

        if instance_layout.0.contains(Mesh::ATTRIBUTE_POSITION) {
            shader_defs.push("VERTEX_POSITIONS".into());
            vertex_attributes.push(Mesh::ATTRIBUTE_POSITION.at_shader_location(3));
        }

        if instance_layout.0.contains(Mesh::ATTRIBUTE_NORMAL) {
            shader_defs.push("VERTEX_NORMALS".into());
            shader_defs.push("INSTANCE_NORMALS".into());
            vertex_attributes.push(Mesh::ATTRIBUTE_NORMAL.at_shader_location(4));
        }

        // we always want an output uv
        shader_defs.push("VERTEX_UVS".into());
        shader_defs.push("VERTEX_UVS_A".into());

        if instance_layout.0.contains(Mesh::ATTRIBUTE_UV_0) {
            shader_defs.push("VERTEX_UVS".into());
            shader_defs.push("INSTANCE_UVS_A".into());
            vertex_attributes.push(Mesh::ATTRIBUTE_UV_0.at_shader_location(5));
        }

        if instance_layout.0.contains(Mesh::ATTRIBUTE_UV_1) {
            shader_defs.push("VERTEX_UVS".into());
            shader_defs.push("VERTEX_UVS_B".into());
            shader_defs.push("INSTANCE_UVS_B".into());
            vertex_attributes.push(Mesh::ATTRIBUTE_UV_1.at_shader_location(6));
        }

        if instance_layout.0.contains(Mesh::ATTRIBUTE_TANGENT) {
            shader_defs.push("VERTEX_TANGENTS".into());
            vertex_attributes.push(Mesh::ATTRIBUTE_TANGENT.at_shader_location(7));
        }

        if instance_layout.0.contains(Mesh::ATTRIBUTE_COLOR) {
            shader_defs.push("VERTEX_COLORS".into());
            vertex_attributes.push(Mesh::ATTRIBUTE_COLOR.at_shader_location(8));
        }

        // if cfg!(feature = "pbr_transmission_textures") {
        //     shader_defs.push("PBR_TRANSMISSION_TEXTURES_SUPPORTED".into());
        // }
        // if cfg!(feature = "pbr_multi_layer_material_textures") {
        //     shader_defs.push("PBR_MULTI_LAYER_MATERIAL_TEXTURES_SUPPORTED".into());
        // }
        // if cfg!(feature = "pbr_anisotropy_texture") {
        //     shader_defs.push("PBR_ANISOTROPY_TEXTURE_SUPPORTED".into());
        // }
        // if cfg!(feature = "pbr_specular_textures") {
        //     shader_defs.push("PBR_SPECULAR_TEXTURES_SUPPORTED".into());
        // }
        // if cfg!(feature = "bluenoise_texture") {
        //     shader_defs.push("BLUE_NOISE_TEXTURE".into());
        // }
        // if cfg!(feature = "dfg_lut") {
        //     shader_defs.push("DFG_LUT".into());
        // }
        // if cfg!(feature = "area_light_luts") {
        //     shader_defs.push("AREA_LIGHT_LUTS".into());
        // }

        let bind_group_layout = self.mesh_pipeline.get_view_layout(key.into());
        let mut bind_group_layout = vec![
            bind_group_layout.main_layout.clone(),
            bind_group_layout.binding_array_layout.clone(),
        ];

        if key.msaa_samples() > 1 {
            shader_defs.push("MULTISAMPLED".into());
        };

        bind_group_layout.push(setup_morph_and_skinning_defs(
            &self.mesh_pipeline.mesh_layouts,
            instance_layout,
            6,
            &key,
            &mut shader_defs,
            &mut vertex_attributes,
            self.mesh_pipeline.skins_use_uniform_buffers,
        ));

        bind_group_layout.push(self.point_cloud_uniform_layout.clone());

        if splat_key.contains(SplatPipelineKey::IS_OCTREE) {
            bind_group_layout.push(self.point_cloud_octree_visible_nodes_layout.clone());
        } else {
            bind_group_layout.push(self.empty_layout.clone());
        }

        if key.contains(MeshPipelineKey::SCREEN_SPACE_AMBIENT_OCCLUSION) {
            shader_defs.push("SCREEN_SPACE_AMBIENT_OCCLUSION".into());
        }

        if key.contains(MeshPipelineKey::CONTACT_SHADOWS) {
            shader_defs.push("CONTACT_SHADOWS".into());
        }

        let mut instance_buffer_layout = instance_layout.0.get_layout(&vertex_attributes)?;
        // don't forget to set step_mode mode to instance
        instance_buffer_layout.step_mode = VertexStepMode::Instance;

        let (label, blend, depth_write_enabled);
        let pass = key.intersection(MeshPipelineKey::BLEND_RESERVED_BITS);
        let (mut is_opaque, mut alpha_to_coverage_enabled) = (false, false);
        if key.contains(MeshPipelineKey::OIT_ENABLED) && pass == MeshPipelineKey::BLEND_ALPHA {
            label = "oit_pointcloud_pipeline".into();
            // TODO tail blending would need alpha blending
            blend = None;
            shader_defs.push("OIT_ENABLED".into());
            // TODO it should be possible to use this to combine MSAA and OIT
            // alpha_to_coverage_enabled = true;
            depth_write_enabled = false;
        } else if pass == MeshPipelineKey::BLEND_ALPHA {
            label = "alpha_blend_pointcloud_pipeline".into();
            blend = Some(BlendState::ALPHA_BLENDING);
            // For the transparent pass, fragments that are closer will be alpha blended
            // but their depth is not written to the depth buffer
            depth_write_enabled = false;
        } else if pass == MeshPipelineKey::BLEND_PREMULTIPLIED_ALPHA {
            label = "premultiplied_alpha_pointcloud_pipeline".into();
            blend = Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING);
            shader_defs.push("PREMULTIPLY_ALPHA".into());
            shader_defs.push("BLEND_PREMULTIPLIED_ALPHA".into());
            // For the transparent pass, fragments that are closer will be alpha blended
            // but their depth is not written to the depth buffer
            depth_write_enabled = false;
        } else if pass == MeshPipelineKey::BLEND_MULTIPLY {
            label = "multiply_pointcloud_pipeline".into();
            blend = Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::Dst,
                    dst_factor: BlendFactor::OneMinusSrcAlpha,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent::OVER,
            });
            shader_defs.push("PREMULTIPLY_ALPHA".into());
            shader_defs.push("BLEND_MULTIPLY".into());
            // For the multiply pass, fragments that are closer will be alpha blended
            // but their depth is not written to the depth buffer
            depth_write_enabled = false;
        } else if pass == MeshPipelineKey::BLEND_ALPHA_TO_COVERAGE {
            label = "alpha_to_coverage_pointcloud_pipeline".into();
            // BlendState::REPLACE is not needed here, and None will be potentially much faster in
            // some cases
            blend = None;
            // For the opaque and alpha mask passes, fragments that are closer will replace
            // the current fragment value in the output and the depth is written to the
            // depth buffer
            depth_write_enabled = true;
            is_opaque = !key.contains(MeshPipelineKey::READS_VIEW_TRANSMISSION_TEXTURE);
            alpha_to_coverage_enabled = true;
            shader_defs.push("ALPHA_TO_COVERAGE".into());
        } else {
            label = "opaque_pointcloud_pipeline".into();
            // BlendState::REPLACE is not needed here, and None will be potentially much faster in
            // some cases
            blend = None;
            // For the opaque and alpha mask passes, fragments that are closer will replace
            // the current fragment value in the output and the depth is written to the
            // depth buffer
            depth_write_enabled = true;
            is_opaque = !key.contains(MeshPipelineKey::READS_VIEW_TRANSMISSION_TEXTURE);
        }

        if key.contains(MeshPipelineKey::NORMAL_PREPASS) {
            shader_defs.push("NORMAL_PREPASS".into());
        }

        if key.contains(MeshPipelineKey::DEPTH_PREPASS) {
            shader_defs.push("DEPTH_PREPASS".into());
        }

        if key.contains(MeshPipelineKey::MOTION_VECTOR_PREPASS) {
            shader_defs.push("MOTION_VECTOR_PREPASS".into());
        }

        if key.contains(MeshPipelineKey::HAS_PREVIOUS_SKIN) {
            shader_defs.push("HAS_PREVIOUS_SKIN".into());
        }

        if key.contains(MeshPipelineKey::HAS_PREVIOUS_MORPH) {
            shader_defs.push("HAS_PREVIOUS_MORPH".into());
        }

        if key.contains(MeshPipelineKey::DEFERRED_PREPASS) {
            shader_defs.push("DEFERRED_PREPASS".into());
        }

        if key.contains(MeshPipelineKey::NORMAL_PREPASS) && key.msaa_samples() == 1 && is_opaque {
            shader_defs.push("LOAD_PREPASS_NORMALS".into());
        }

        let view_projection = key.intersection(MeshPipelineKey::VIEW_PROJECTION_RESERVED_BITS);
        if view_projection == MeshPipelineKey::VIEW_PROJECTION_NONSTANDARD {
            shader_defs.push("VIEW_PROJECTION_NONSTANDARD".into());
        } else if view_projection == MeshPipelineKey::VIEW_PROJECTION_PERSPECTIVE {
            shader_defs.push("VIEW_PROJECTION_PERSPECTIVE".into());
        } else if view_projection == MeshPipelineKey::VIEW_PROJECTION_ORTHOGRAPHIC {
            shader_defs.push("VIEW_PROJECTION_ORTHOGRAPHIC".into());
        }

        #[cfg(all(feature = "webgl", target_arch = "wasm32", not(feature = "webgpu")))]
        shader_defs.push("WEBGL2".into());

        #[cfg(feature = "experimental_pbr_pcss")]
        shader_defs.push("PCSS_SAMPLERS_AVAILABLE".into());

        if key.contains(MeshPipelineKey::TONEMAP_IN_SHADER) {
            shader_defs.push("TONEMAP_IN_SHADER".into());
            shader_defs.push(ShaderDefVal::UInt(
                "TONEMAPPING_LUT_TEXTURE_BINDING_INDEX".into(),
                TONEMAPPING_LUT_TEXTURE_BINDING_INDEX,
            ));
            shader_defs.push(ShaderDefVal::UInt(
                "TONEMAPPING_LUT_SAMPLER_BINDING_INDEX".into(),
                TONEMAPPING_LUT_SAMPLER_BINDING_INDEX,
            ));

            let method = key.intersection(MeshPipelineKey::TONEMAP_METHOD_RESERVED_BITS);

            if method == MeshPipelineKey::TONEMAP_METHOD_NONE {
                shader_defs.push("TONEMAP_METHOD_NONE".into());
            } else if method == MeshPipelineKey::TONEMAP_METHOD_REINHARD {
                shader_defs.push("TONEMAP_METHOD_REINHARD".into());
            } else if method == MeshPipelineKey::TONEMAP_METHOD_REINHARD_LUMINANCE {
                shader_defs.push("TONEMAP_METHOD_REINHARD_LUMINANCE".into());
            } else if method == MeshPipelineKey::TONEMAP_METHOD_ACES_FITTED {
                shader_defs.push("TONEMAP_METHOD_ACES_FITTED".into());
            } else if method == MeshPipelineKey::TONEMAP_METHOD_AGX {
                shader_defs.push("TONEMAP_METHOD_AGX".into());
            } else if method == MeshPipelineKey::TONEMAP_METHOD_SOMEWHAT_BORING_DISPLAY_TRANSFORM {
                shader_defs.push("TONEMAP_METHOD_SOMEWHAT_BORING_DISPLAY_TRANSFORM".into());
            } else if method == MeshPipelineKey::TONEMAP_METHOD_BLENDER_FILMIC {
                shader_defs.push("TONEMAP_METHOD_BLENDER_FILMIC".into());
            } else if method == MeshPipelineKey::TONEMAP_METHOD_TONY_MC_MAPFACE {
                shader_defs.push("TONEMAP_METHOD_TONY_MC_MAPFACE".into());
            } else if method == MeshPipelineKey::TONEMAP_METHOD_PBR_NEUTRAL {
                shader_defs.push("TONEMAP_METHOD_PBR_NEUTRAL".into());
            }

            // Debanding is tied to tonemapping in the shader, cannot run without it.
            if key.contains(MeshPipelineKey::DEBAND_DITHER) {
                shader_defs.push("DEBAND_DITHER".into());
            }
        }

        if key.contains(MeshPipelineKey::MAY_DISCARD) {
            shader_defs.push("MAY_DISCARD".into());
        }

        if key.contains(MeshPipelineKey::ENVIRONMENT_MAP) {
            shader_defs.push("ENVIRONMENT_MAP".into());
        }

        if key.contains(MeshPipelineKey::IRRADIANCE_VOLUME) && IRRADIANCE_VOLUMES_ARE_USABLE {
            shader_defs.push("IRRADIANCE_VOLUME".into());
        }

        if key.contains(MeshPipelineKey::LIGHTMAPPED) {
            shader_defs.push("LIGHTMAP".into());
        }
        if key.contains(MeshPipelineKey::LIGHTMAP_BICUBIC_SAMPLING) {
            shader_defs.push("LIGHTMAP_BICUBIC_SAMPLING".into());
        }

        if key.contains(MeshPipelineKey::TEMPORAL_JITTER) {
            shader_defs.push("TEMPORAL_JITTER".into());
        }

        let shadow_filter_method =
            key.intersection(MeshPipelineKey::SHADOW_FILTER_METHOD_RESERVED_BITS);
        if shadow_filter_method == MeshPipelineKey::SHADOW_FILTER_METHOD_HARDWARE_2X2 {
            shader_defs.push("SHADOW_FILTER_METHOD_HARDWARE_2X2".into());
        } else if shadow_filter_method == MeshPipelineKey::SHADOW_FILTER_METHOD_GAUSSIAN {
            shader_defs.push("SHADOW_FILTER_METHOD_GAUSSIAN".into());
        } else if shadow_filter_method == MeshPipelineKey::SHADOW_FILTER_METHOD_TEMPORAL {
            shader_defs.push("SHADOW_FILTER_METHOD_TEMPORAL".into());
        }

        let blur_quality =
            key.intersection(MeshPipelineKey::SCREEN_SPACE_SPECULAR_TRANSMISSION_RESERVED_BITS);

        shader_defs.push(ShaderDefVal::Int(
            "SCREEN_SPACE_SPECULAR_TRANSMISSION_BLUR_TAPS".into(),
            match blur_quality {
                MeshPipelineKey::SCREEN_SPACE_SPECULAR_TRANSMISSION_LOW => 4,
                MeshPipelineKey::SCREEN_SPACE_SPECULAR_TRANSMISSION_MEDIUM => 8,
                MeshPipelineKey::SCREEN_SPACE_SPECULAR_TRANSMISSION_HIGH => 16,
                MeshPipelineKey::SCREEN_SPACE_SPECULAR_TRANSMISSION_ULTRA => 32,
                _ => unreachable!(), /* Not possible, since the mask is 2 bits, and we've covered
                                      * all 4 cases */
            },
        ));

        if key.contains(MeshPipelineKey::VISIBILITY_RANGE_DITHER) {
            shader_defs.push("VISIBILITY_RANGE_DITHER".into());
        }

        if key.contains(MeshPipelineKey::DISTANCE_FOG) {
            shader_defs.push("DISTANCE_FOG".into());
        }

        if key.contains(MeshPipelineKey::ATMOSPHERE) {
            shader_defs.push("ATMOSPHERE".into());
        }

        if self.mesh_pipeline.binding_arrays_are_usable {
            shader_defs.push("MULTIPLE_LIGHT_PROBES_IN_ARRAY".into());
            shader_defs.push("MULTIPLE_LIGHTMAPS_IN_ARRAY".into());
        }

        if IRRADIANCE_VOLUMES_ARE_USABLE {
            shader_defs.push("IRRADIANCE_VOLUMES_ARE_USABLE".into());
        }

        if self.mesh_pipeline.clustered_decals_are_usable {
            shader_defs.push("CLUSTERED_DECALS_ARE_USABLE".into());
            if cfg!(feature = "pbr_light_textures") {
                shader_defs.push("LIGHT_TEXTURES".into());
            }
        }

        let format = key.target_format();

        // This is defined here so that custom shaders that use something other than
        // the mesh binding from bevy_pbr::mesh_bindings can easily make use of this
        // in their own shaders.
        if let Some(per_object_buffer_batch_size) = self.mesh_pipeline.per_object_buffer_batch_size
        {
            shader_defs.push(ShaderDefVal::UInt(
                "PER_OBJECT_BUFFER_BATCH_SIZE".into(),
                per_object_buffer_batch_size,
            ));
        }

        Ok(RenderPipelineDescriptor {
            vertex: VertexState {
                shader: self.shader.clone(),
                shader_defs: shader_defs.clone(),
                buffers: vec![vertex_buffer_layout, instance_buffer_layout],
                ..default()
            },
            fragment: Some(FragmentState {
                shader: self.shader.clone(),
                shader_defs,
                targets: vec![Some(ColorTargetState {
                    format,
                    blend,
                    write_mask: ColorWrites::ALL,
                })],
                ..default()
            }),
            layout: bind_group_layout,
            primitive: PrimitiveState {
                cull_mode: Some(Face::Back),
                unclipped_depth: false,
                // don't use [`key.primitive_topology()`] because we want a quad, not points
                topology: bevy::mesh::PrimitiveTopology::TriangleList,
                strip_index_format: key.strip_index_format(),
                ..default()
            },
            depth_stencil: Some(DepthStencilState {
                format: CORE_3D_DEPTH_FORMAT,
                depth_write_enabled: Some(depth_write_enabled),
                depth_compare: Some(CompareFunction::GreaterEqual),
                stencil: StencilState {
                    front: StencilFaceState::IGNORE,
                    back: StencilFaceState::IGNORE,
                    read_mask: 0,
                    write_mask: 0,
                },
                bias: DepthBiasState {
                    constant: 0,
                    slope_scale: 0.0,
                    clamp: 0.0,
                },
            }),
            multisample: MultisampleState {
                count: key.msaa_samples(),
                mask: !0,
                alpha_to_coverage_enabled,
            },
            label: Some(label),
            ..default()
        })
    }
}
