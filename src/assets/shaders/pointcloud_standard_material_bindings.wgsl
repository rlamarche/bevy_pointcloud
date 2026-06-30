#define_import_path bevy_pointcloud::pointcloud_standard_material_bindings

#import bevy_pointcloud::pointcloud_types::StandardPointCloudMaterial

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: StandardPointCloudMaterial;
// @group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> point_size: f32;


// #ifdef BINDLESS

// struct StandardMaterialBindings {
//     material: u32,                      // 0
//     base_color_texture: u32,            // 1
//     base_color_sampler: u32,            // 2
//     emissive_texture: u32,              // 3
//     emissive_sampler: u32,              // 4
//     metallic_roughness_texture: u32,    // 5
//     metallic_roughness_sampler: u32,    // 6
//     occlusion_texture: u32,             // 7
//     occlusion_sampler: u32,             // 8
//     normal_map_texture: u32,            // 9
//     normal_map_sampler: u32,            // 10
//     depth_map_texture: u32,             // 11
//     depth_map_sampler: u32,             // 12
//     anisotropy_texture: u32,            // 13
//     anisotropy_sampler: u32,            // 14
//     specular_transmission_texture: u32, // 15
//     specular_transmission_sampler: u32, // 16
//     thickness_texture: u32,             // 17
//     thickness_sampler: u32,             // 18
//     diffuse_transmission_texture: u32,  // 19
//     diffuse_transmission_sampler: u32,  // 20
//     clearcoat_texture: u32,             // 21
//     clearcoat_sampler: u32,             // 22
//     clearcoat_roughness_texture: u32,   // 23
//     clearcoat_roughness_sampler: u32,   // 24
//     clearcoat_normal_texture: u32,      // 25
//     clearcoat_normal_sampler: u32,      // 26
//     specular_texture: u32,              // 27
//     specular_sampler: u32,              // 28
//     specular_tint_texture: u32,         // 29
//     specular_tint_sampler: u32,         // 30
// }

// @group(#{MATERIAL_BIND_GROUP}) @binding(0) var<storage> material_indices: array<StandardMaterialBindings>;
// @group(#{MATERIAL_BIND_GROUP}) @binding(10) var<storage> material_array: array<StandardMaterial>;

// #else   // BINDLESS

// @group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: StandardPointCloudMaterial;

// #endif // BINDLESS
