struct vs_input_entity_ids {
    uint4 ids: TEXCOORD4;
};

struct vs_output_material {
    float4 position: SV_POSITION0;
    float4 world_pos: TEXCOORD0;
    float4 texcoord: TEXCOORD1;
    float4 colour: TEXCOORD2;
    float3 normal: TEXCOORD3;
    float3 tangent: TEXCOORD4;
    float3 bitangent: TEXCOORD5;
    uint4  ids: TEXCOORD6;
};

vs_output_material vs_mesh_material(vs_input_mesh input, vs_input_entity_ids entity_input) {
    vs_output_material output;

    // get draw call info and transform world matrix
    draw_data draw = get_draw_data(entity_input.ids[0]);
    float4 pos = float4(input.position.xyz, 1.0);    
    pos.xyz = mul(draw.world_matrix, pos);

    output.position = mul(view_projection_matrix, pos);
    output.world_pos = pos;
    output.texcoord = float4(input.texcoord, 0.0, 0.0);
    
    float3x3 rot = (float3x3)draw.world_matrix;
    output.normal = normalize(mul(rot, input.normal));
    output.tangent = normalize(mul(rot, input.tangent));
    output.bitangent = normalize(mul(rot, input.bitangent));
    
    // mat
    material_data mat = get_material_data(entity_input.ids[1]);
    output.ids = uint4(mat.albedo_id, mat.normal_id, mat.roughness_id, mat.padding);
    
    return output;
}

vs_output_material vs_mesh_material_indirect(vs_input_mesh input) {
    vs_output_material output;

    // get draw call info and transform world matrix
    draw_data draw = get_draw_data(indirect_ids.x);
    float4 pos = float4(input.position.xyz, 1.0);    
    pos.xyz = mul(draw.world_matrix, pos);

    // get camera data and transform projection matrix
    camera_data main_camera = get_camera_data();

    output.position = mul(main_camera.view_projection_matrix, pos);
    output.world_pos = pos;
    output.texcoord = float4(input.texcoord, 0.0, 0.0);
    output.colour = float4(1.0, 1.0, 1.0, 1.0);

    float3x3 rot = (float3x3)draw.world_matrix;
    output.normal = normalize(mul(rot, input.normal));
    output.tangent = normalize(mul(rot, input.tangent));
    output.bitangent = normalize(mul(rot, input.bitangent));

    material_data mat = get_material_data(indirect_ids.y);
    output.ids = uint4(mat.albedo_id, mat.normal_id, mat.roughness_id, mat.padding);
    
    return output;
}

vs_output_material vs_mesh_lit(vs_input_mesh input) {
    vs_output_material output;

    float3x4 wm = world_matrix;
    float4 pos = float4(input.position.xyz, 1.0);
    pos.xyz = mul(wm, pos);

    output.position = mul(view_projection_matrix, pos);
    output.world_pos = pos;
    output.texcoord = float4(input.texcoord, 0.0, 0.0);

    float3x3 rot = (float3x3)wm;
    output.normal = normalize(mul(rot, input.normal));
    output.tangent = normalize(mul(rot, input.tangent));
    output.bitangent = normalize(mul(rot, input.bitangent));
    output.ids = uint4(0, 0, 0, 0);

    return output;
}

ps_output ps_mesh_debug_tangent_space(vs_output_material input) {
    ps_output output;
    output.colour = float4(0.0, 0.0, 0.0, 0.0);

    float3 ts_normal = textures[draw_indices.x].Sample(sampler_wrap_linear, input.texcoord.xy).xyz;
    ts_normal = ts_normal * 2.0 - 1.0;

    float3x3 tbn;
    tbn[0] = input.tangent.xyz;
    tbn[1] = input.bitangent.xyz;
    tbn[2] = input.normal.xyz;

    float3 normal = mul(ts_normal, tbn);

    output.colour.rgb = normal;
    output.colour.rgb = output.colour.rgb * 0.5 + 0.5;

    return output;
}

ps_output ps_mesh_material(vs_output_material input) {
    ps_output output;
    output.colour = float4(0.0, 0.0, 0.0, 0.0);

    float2 tc = input.texcoord.xy;
    
    // sample maps

    // albedo
    float4 albedo = textures[input.ids.x].Sample(sampler_wrap_linear, tc);

    // normal
    float3 ts_normal = textures[input.ids.y].Sample(sampler_wrap_linear, tc).xyz;
    ts_normal = ts_normal * 2.0 - 1.0;

    float3x3 tbn;
    tbn[0] = input.tangent.xyz;
    tbn[1] = input.bitangent.xyz;
    tbn[2] = input.normal.xyz;
    float3 n = mul(ts_normal, tbn);

    // roughness
    float roughness = textures[input.ids.z].Sample(sampler_wrap_linear, tc).r;

    float k = 0.3;
    float3 v = normalize(input.world_pos.xyz - view_position.xyz);

    // point lights
    uint point_lights_id = world_buffer_info.point_light.x;
    uint point_lights_count = world_buffer_info.point_light.y;

    if(point_lights_id != 0) {
        int i = 0;
        for(i = 0; i < point_lights_count; ++i) {
            point_light_data light = point_lights[point_lights_id][i];

            float3 l = normalize(input.world_pos.xyz - light.pos);

            float diffuse = lambert(l, n);
            float specular = cook_torrance(l, n, v, roughness, k);

            float atteniuation = point_light_attenuation(
                light.pos,
                light.radius,
                input.world_pos.xyz
            );
            
            output.colour += atteniuation * light.colour * diffuse * albedo;
            output.colour += atteniuation * light.colour * specular;
        }
    }

    return output;
}

ps_output ps_mesh_lit(vs_output input) {
    ps_output output;
    output.colour = input.colour;

    int i = 0;
    float ks = 2.0;
    float shininess = 32.0;
    float roughness = 0.1;
    float k = 0.3;

    float3 v = normalize(input.world_pos.xyz - view_position.xyz);
    float3 n = input.normal;

    // point lights
    uint point_lights_id = world_buffer_info.point_light.x;
    uint point_lights_count = world_buffer_info.point_light.y;
    for(i = 0; i < point_lights_count; ++i) {
        point_light_data light = point_lights[point_lights_id][i];

        float3 l = normalize(input.world_pos.xyz - light.pos);

        float diffuse = lambert(l, n);
        float specular = cook_torrance(l, n, v, roughness, k);

        float atteniuation = point_light_attenuation(
            light.pos,
            light.radius,
            input.world_pos.xyz
        );
        
        output.colour += atteniuation * light.colour * diffuse;
        output.colour += atteniuation * light.colour * specular;
    }

    // spot lights
    uint spot_lights_id = world_buffer_info.spot_light.x;
    uint spot_lights_count = world_buffer_info.spot_light.y;
    for(i = 0; i < spot_lights_count; ++i) {
        spot_light_data light = spot_lights[spot_lights_id][i];

        float3 l = normalize(input.world_pos.xyz - light.pos);

        float diffuse = lambert(l, n);
        float specular = cook_torrance(l, n, v, roughness, k);

        float atteniuation = spot_light_attenuation(
            l,
            light.dir,
            light.cutoff,
            light.falloff
        );
        
        output.colour += atteniuation * light.colour * diffuse;
        output.colour += atteniuation * light.colour * specular;
    }

    // directional lights
    uint directional_lights_id = world_buffer_info.directional_light.x;
    uint directional_lights_count = world_buffer_info.directional_light.y;
    for(i = 0; i < directional_lights_count; ++i) {
        directional_light_data light = directional_lights[directional_lights_id][i];

        float3 l = light.dir.xyz;
        float diffuse = lambert(l, n);
        float specular = cook_torrance(l, n, v, roughness, k);

        output.colour += light.colour * diffuse;
        output.colour += light.colour * specular;
    }

    return output;
}

// basic test
float4 ps_mesh_pbr_ibl(vs_output input) : SV_TARGET {
    float roughness = material_colour.x;
    float metalness = material_colour.y;

    float3 v = normalize(input.world_pos.xyz - view_position.xyz);
    float3 n = input.normal;
    
    float3 albedo = float3(1.0, 0.5, 0.0);

    float3 f0 = lerp(float3(0.04, 0.04, 0.04), albedo, metalness);
    float3 f = fresnel_schlick_roughness(max(dot(n, -v), 0.0), f0, roughness);

    float3 rd = normalize(input.world_pos.xyz - view_position.xyz) * float3(1.0, 1.0, -1.0);
    float3 nd = normalize(input.normal.xyz * float3(1.0, 1.0, -1.0));
    float3 r = reflect(rd, nd); 
    r.z *= -1.0;

    // irradiance / diffuse
    float irradiance_lod = 8.0;
    float3 irradiance = cubemaps[draw_indices.x].SampleLevel(sampler_wrap_linear, n.xyz, irradiance_lod).rgb;
    float3 diffuse = irradiance * albedo;

    // specular / reflection
    float spec_lod = 16.0;

    float3 ks = f;
    float3 kd = (1.0 - ks) * (1.0 - metalness);
    float3 prefilter = cubemaps[draw_indices.x].SampleLevel(sampler_wrap_linear, r.xyz, roughness * spec_lod).rgb;
    float2 brdf = textures[draw_indices.y].Sample(sampler_wrap_linear, float2(saturate(dot(n, v)), roughness)).rg;
    float3 specular = prefilter * (f * brdf.x + brdf.y);

    return float4(((kd * max(diffuse, 0.0) + max(specular, 0.0))), 1.0);
}

// basic ray traced shadow
bool is_occluded(float3 origin, float3 direction, float tMin, float tMax)
{
    RayQuery<RAY_FLAG_SKIP_PROCEDURAL_PRIMITIVES> rayQuery;

    RayDesc desc;
    desc.Origin = origin;
    desc.TMin = tMin;
    desc.Direction = direction;
    desc.TMax = tMax;

    rayQuery.TraceRayInline(
        scene_tlas,
        RAY_FLAG_ACCEPT_FIRST_HIT_AND_END_SEARCH,
        0xFF,
        desc
    );

    while (rayQuery.Proceed()) // Traverse BVH
    {
        if (rayQuery.CommittedStatus() == COMMITTED_TRIANGLE_HIT)
        {
            return true; // Something is occluding
        }
    }
    return false; // No occlusion
}

struct RayPayload
{
    float4 color;
};

[shader("raygeneration")]
void scene_raygen_shader()
{
    /*
    float2 lerp_values = (float2)DispatchRaysIndex() / (float2)DispatchRaysDimensions();

    // Orthographic projection since we're raytracing in screen space.
    float3 ray_dir = float3(0.0, 0.0, 1.0);
    float3 origin = float3(
        lerp(
            raygen_constants.viewport.left, 
            raygen_constants.viewport.right, 
            lerp_values.x
        ),
        lerp(
            raygen_constants.viewport.top, 
            raygen_constants.viewport.bottom, 
            lerp_values.y
        ),
        0.0f
    );


    if (inside_viewport(origin.xy, raygen_constants.stencil))
    {
        // Trace the ray.
        // Set the ray's extents.
        RayDesc ray;
        ray.Origin = origin;
        ray.Direction = ray_dir;
        
        // Set TMin to a non-zero small value to avoid aliasing issues due to floating - point errors.
        // TMin should be kept small to prevent missing geometry at close contact areas.
        ray.TMin = 0.001;
        ray.TMax = 10000.0;
        RayPayload payload = { float4(0.0, 0.0, 1.0, 0.0) };
        TraceRay(scene, RAY_FLAG_NONE, ~0, 0.0, 1.0, 0.0, ray, payload);

        // Write the raytraced color to the output texture.
        output_target[DispatchRaysIndex().xy] = payload.color;
    }
    else
    {
        // Render interpolated DispatchRaysIndex outside the stencil window
        output_target[DispatchRaysIndex().xy] = float4(lerp_values, 1.0, 1.0);
    }
    */
}

[shader("closesthit")]
void scene_closest_hit_shader(inout RayPayload payload, in BuiltInTriangleIntersectionAttributes attr)
{
    float3 barycentrics = float3(1 - attr.barycentrics.x - attr.barycentrics.y, attr.barycentrics.x, attr.barycentrics.y);
    payload.color = float4(float3(1.0, 1.0, 1.0) - barycentrics, 1.0);
}

[shader("miss")]
void scene_miss_shader(inout RayPayload payload)
{
    payload.color = float4(0.5, 0.5, 0.5, 1.0);
}