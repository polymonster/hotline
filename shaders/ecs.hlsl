//
// contains core descriptor layout to be used among different / shared ecs systems
//

// generic (fat) + non-skinned mesh vertex layout 
struct vs_input_mesh {
    float3 position: POSITION;
    float2 texcoord: TEXCOORD0;
    float3 normal: TEXCOORD1;
    float3 tangent: TEXCOORD2;
    float3 bitangent: TEXCOORD3;
}

// generic single target pixel shader output
struct ps_output {
    float4 colour: SV_Target;
}

// per view constants with basic camera transforms
cbuffer view_push_constants : register(b0) {
    float4x4 view_projection_matrix;
    float4   view_position;
}

// per entity draw constants used in CPU draw calls
cbuffer draw_push_constants : register(b1) {
    float3x4 world_matrix;
    float4   material_colour;
    uint4    draw_indices;
}

// per indirect draw, indirect_ids.x = entity_id, they alias slot b1 because they are an alternative to `draw_push_constants`
cbuffer indirect_push_constants : register(b1) {
    uint4 indirect_ids;
}

// world id's bind on slot b2... below
// containing ids of world buffers within the striuctured buffer arrays
// cameras bind on slot b3

// resource lookup indices
// these indices of resources used in compute shaders, then can specified in pmfx and passed as push constants
// the names are references as uses: ["texure_name1", "texure_name2"] and passes through as srv indices
struct resource_use {
    uint  index;
    uint3 dimension;
}

struct resource_uses {
    resource_use input0;
    resource_use input1;
    resource_use input2;
    resource_use input3;
    resource_use input4;
    resource_use input5;
    resource_use input6;
    resource_use input7;
}

ConstantBuffer<resource_uses> resources: register(b0, space1);

// bindless draw data for entites to look up by ID
struct draw_data {
    float3x4 world_matrix;
}

// bindless material ID's which can be looked up into textures array
struct material_data {
    uint albedo_id;
    uint normal_id;
    uint roughness_id;
    uint padding;
}

// the x value holds the srv index to look up in materials[] etc and the y component holds the count in the buffer
struct world_buffer_info_data {
    uint2 draw;
    uint2 extent;
    uint2 material;
    uint2 point_light;
    uint2 spot_light;
    uint2 directional_light;
    uint2 camera;
    uint2 shadow_matrix;
}

// info to lookup shadow srv and matrix
struct shadow_map_info {
    uint srv_index;
    uint matrix_index;
}

// point light parameters
struct point_light_data {
    float3          pos;
    float           radius;
    float4          colour;
    shadow_map_info shadow_map;
}

// spot light parameters
struct spot_light_data {
    float3          pos;
    float           cutoff;
    float3          dir;
    float           falloff;
    float4          colour;
    shadow_map_info shadow_map;
}

// directional light data
struct directional_light_data {
    float3          dir;
    float4          colour;
    shadow_map_info shadow_map;
}

// camera data
struct camera_data {
    float4x4 view_projection_matrix;
    float4   view_position;
    float4   planes[6];
}

// extent data
struct extent_data {
    float3 pos;
    float3 extent;
}

// structures of arrays for indriect / bindless lookups
StructuredBuffer<draw_data> draws[] : register(t0, space0);
StructuredBuffer<extent_data> extents[] : register(t0, space1);
StructuredBuffer<material_data> materials[] : register(t0, space2);
StructuredBuffer<point_light_data> point_lights[] : register(t0, space3);
StructuredBuffer<spot_light_data> spot_lights[] : register(t0, space4);
StructuredBuffer<directional_light_data> directional_lights[] : register(t0, space5);
StructuredBuffer<float4x4> shadow_matrices[] : register(t0, space6);

// textures 
Texture2D textures[] : register(t0, space7);
Texture2DMS<float4, 8> msaa8x_textures[] : register(t0, space8);
TextureCube cubemaps[] : register(t0, space9);
Texture2DArray texture_arrays[] : register(t0, space10);
Texture3D volume_textures[] : register(t0, space11);

// uav textures
RWTexture2D<float4> rw_textures[] : register(u0, space0);
RWTexture3D<float4> rw_volume_textures[] : register(u0, space1);

// main constants to obtain the indices of the buffer types
ConstantBuffer<world_buffer_info_data> world_buffer_info : register(b2);

// camera data for bindless camera lookups
ConstantBuffer<camera_data> cameras[] : register(b3);

// samplers
SamplerState sampler0 : register(s0);
SamplerState sampler_wrap_linear : register(s1);
SamplerState sampler_clamp_point : register(s2);
SamplerComparisonState sampler_shadow_compare : register(s3);

// utility functions to lookup entity draw data
draw_data get_draw_data(uint entity_index) {
    return draws[world_buffer_info.draw.x][entity_index];
}

// utility functions to lookup entity extent data used for culling
extent_data get_extent_data(uint entity_index) {
    return extents[world_buffer_info.extent.x][entity_index];
}

// utility functions to lookup material data
material_data get_material_data(uint material_index) {
    return materials[world_buffer_info.material.x][material_index];
}

// utility functions to lookup camera data
camera_data get_camera_data() {
    return cameras[world_buffer_info.camera.x];
}

// utility to return a shadow matrix by index
float4x4 get_shadow_matrix(uint shadow_index) {
    return shadow_matrices[world_buffer_info.shadow_matrix.x][shadow_index];
}