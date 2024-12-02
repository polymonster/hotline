float4 vs_depth_only(vs_input_mesh input) : SV_POSITION {
    float4 pos = float4(input.position.xyz, 1.0);
    pos.xyz = mul(world_matrix, pos);

    float4 output = mul(view_projection_matrix, pos);
    return output;
}

struct vs_output_world_pos {
    float4 pos : SV_POSITION;
    float3 world_pos : TEXCOORD0;
};

vs_output_world_pos vs_depth_world_pos(vs_input_mesh input) {
    vs_output_world_pos output;

    float4 pos = float4(input.position.xyz, 1.0);
    pos.xyz = mul(world_matrix, pos);

    output.world_pos = pos.xyz;
    output.pos = mul(view_projection_matrix, pos);

    return output;
}

float ps_omni_shadow_depth(vs_output_world_pos input) : SV_Depth {
    uint point_lights_id = world_buffer_info.point_light.x;
    uint point_lights_count = world_buffer_info.point_light.y;
    point_light_data light = point_lights[point_lights_id][0];
    float d = length(input.world_pos - light.pos) / light.radius;
    return d / 2.0; // divide by 2 because the ortho far plane is light radius * 2.0
}


float sample_shadow_pcf_9(float3 sp, uint sm_index, float2 sm_size) {
    float2 samples[9];
    float2 inv_sm_size = 1.0 / sm_size;
    samples[0] = float2(-1.0, -1.0) * inv_sm_size;
    samples[1] = float2(-1.0, 0.0) * inv_sm_size;
    samples[2] = float2(-1.0, 1.0) * inv_sm_size;
    samples[3] = float2(0.0, -1.0) * inv_sm_size;
    samples[4] = float2(0.0, 0.0) * inv_sm_size;
    samples[5] = float2(0.0, 1.0) * inv_sm_size;
    samples[6] = float2(1.0, -1.0) * inv_sm_size;
    samples[7] = float2(1.0, 0.0) * inv_sm_size;
    samples[8] = float2(1.0, 1.0) * inv_sm_size;
    
    float shadow = 0.0;

    [unroll]
    for(int j = 0; j < 9; ++j) {
        shadow += textures[sm_index].SampleCmp(sampler_shadow_compare, sp.xy + samples[j], 0.0);
    }
    shadow /= 9.0;

    shadow = textures[sm_index].SampleCmp(sampler_shadow_compare, sp.xy, sp.z);
    return shadow;
}

float sample_shadow_cube_pcf_9(float3 cv, float d, uint sm_index, float sm_size) {

    float3 b2, t;
    construct_orthonormal_basis_hughes_moeller(cv, b2, t);

    float3 samples[9];
    float inv_sm_size = 1.0 / (sm_size.x * d) + (1.0 / (sm_size.x * d)); // scale offset by distance
    samples[0] = (b2 * -1.0 + t * -1.0) * inv_sm_size;
    samples[1] = (b2 * -1.0 + t *  0.0) * inv_sm_size;
    samples[2] = (b2 * -1.0 + t *  1.0) * inv_sm_size;
    samples[3] = (b2 *  0.0 + t * -1.0) * inv_sm_size;
    samples[4] = (b2 *  0.0 + t *  0.0) * inv_sm_size;
    samples[5] = (b2 *  0.0 + t *  1.0) * inv_sm_size;
    samples[6] = (b2 *  1.0 + t * -1.0) * inv_sm_size;
    samples[7] = (b2 *  1.0 + t *  0.0) * inv_sm_size;
    samples[8] = (b2 *  1.0 + t * -1.0) * inv_sm_size;
    
    float shadow = 0.0;

    [unroll]
    for(int j = 0; j < 9; ++j) {
        shadow += cubemaps[shadow_map_index].SampleCmp(sampler_shadow_compare, cv + samples[j], d).r;
    }
    shadow /= 9.0;

    // shadow = cubemaps[shadow_map_index].SampleCmp(sampler_shadow_compare, cv, d).r;
    return shadow;
}

float4 ps_single_directional_shadow(vs_output input) : SV_Target {
    float4 output = float4(0.0, 0.0, 0.0, 0.0);

    int i = 0;
    float ks = 2.0;
    float shininess = 32.0;
    float roughness = 0.1;
    float k = 0.3;

    float3 v = normalize(input.world_pos.xyz - view_position.xyz);
    float3 n = input.normal;

    // single directional light
    uint directional_lights_id = world_buffer_info.directional_light.x;
    directional_light_data light = directional_lights[directional_lights_id][0];

    int shadow_map_index = light.shadow_map.srv_index;
    float4x4 shadow_matrix = get_shadow_matrix(light.shadow_map.matrix_index);
    
    // project shadow coord
    float4 offset_pos = float4(input.world_pos.xyz, 1.0);

    float4 sp = mul(shadow_matrix, offset_pos);
    sp.xyz /= sp.w;
    sp.y *= -1.0;
    sp.xy = sp.xy * 0.5 + 0.5;

    float shadow_sample = textures[shadow_map_index].Sample(sampler_clamp_point, sp.xy).r;
    float shadow = sp.z >= shadow_sample ? 0.0 : 1.0;

    shadow = sample_shadow_pcf_9(sp, shadow_map_index, float2(4096, 4096));

    float3 l = light.dir.xyz;
    float diffuse = lambert(l, n);
    float specular = cook_torrance(l, n, v, roughness, k);

    if(dot(n, l) >= 0.0) {
        shadow = 0.0;
    }

    float4 lit_colour = light.colour * diffuse + light.colour * specular;
    output = lit_colour * shadow + light.colour * 0.2;

    return output;
}

float4 ps_single_omni_shadow(vs_output input) : SV_Target {
    
    int i = 0;
    float ks = 2.0;
    float roughness = 0.9;
    float k = 0.3;
    float4 output = float4(0.0, 0.0, 0.0, 0.0);

    float3 v = normalize(input.world_pos.xyz - view_position.xyz);
    float3 n = input.normal;

    // point lights
    uint point_lights_id = world_buffer_info.point_light.x;
    uint point_lights_count = world_buffer_info.point_light.y;
    point_light_data light = point_lights[point_lights_id][0];

    float3 l = normalize(input.world_pos.xyz - light.pos);

    float diffuse = lambert(l, n);
    float specular = cook_torrance(l, n, v, roughness, k);

    float atteniuation = point_light_attenuation_cutoff(
        light.pos,
        light.radius,
        input.world_pos.xyz
    );
    
    output += atteniuation * light.colour * diffuse;
    output += atteniuation * light.colour * specular;

    // omni directional shadow
    float3 to_light = input.world_pos.xyz - light.pos;
    float d = length(to_light) / light.radius / 2.0; // omni shadow space far plane is radius * 2.0
    float3 cv = l * float3(1.0, 1.0, -1.0);

    // shadow map info
    int shadow_map_index = light.shadow_map.srv_index;
    float sm_size = 2048.0;

    //
    float3 b2, t;

    // choose a vector orthogonal to cv as the direction of b2.
    b2 = float3(0.0, -cv.z, cv.y);
    if(abs(n.x) > abs(n.z))
    {
        b2 = float3(-cv.y, cv.x, 0.0);
    }

    // normalise b2 and construct t
    b2 = b2 * rsqrt(dot(b2, b2));
    t = cross(b2, n);

    float3 samples[9];
    float inv_sm_size = 1.0 / (sm_size.x * d) + (1.0 / (sm_size.x * d)); // scale offset by distance
    samples[0] = (b2 * -1.0 + t * -1.0) * inv_sm_size;
    samples[1] = (b2 * -1.0 + t *  0.0) * inv_sm_size;
    samples[2] = (b2 * -1.0 + t *  1.0) * inv_sm_size;
    samples[3] = (b2 *  0.0 + t * -1.0) * inv_sm_size;
    samples[4] = (b2 *  0.0 + t *  0.0) * inv_sm_size;
    samples[5] = (b2 *  0.0 + t *  1.0) * inv_sm_size;
    samples[6] = (b2 *  1.0 + t * -1.0) * inv_sm_size;
    samples[7] = (b2 *  1.0 + t *  0.0) * inv_sm_size;
    samples[8] = (b2 *  1.0 + t * -1.0) * inv_sm_size;
    
    float shadow = 0.0;

    [unroll]
    for(int j = 0; j < 9; ++j) {
        shadow += cubemaps[shadow_map_index].SampleCmp(sampler_shadow_compare, cv + samples[j], d).r;
    }
    shadow /= 9.0;

    if(dot(n, l) >= 0.0) {
        shadow = 0.0;
    }
    
    return output * shadow;
}