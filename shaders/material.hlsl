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

struct RayPayload
{
    float4  col;
    int     bounce_count;
    bool    has_bounce_ray;
    RayDesc bounce_ray;
};

RayPayload default_payload()
{
    RayPayload payload;
    payload.col = float4(0.0, 0.0, 0.0, 0.0);
    payload.bounce_count = 0;
    payload.has_bounce_ray = false;

    return payload;
}

cbuffer ray_tracing_constants : register(b0) {
    float4x4    inverse_wvp;
    int4        resource_indices; // x = uav output, y = scene_tlas
};

// basic ray traced shadow
bool is_occluded(float3 origin, float3 direction, float tmin, float tmax)
{
    RayQuery<RAY_FLAG_CULL_BACK_FACING_TRIANGLES | RAY_FLAG_ACCEPT_FIRST_HIT_AND_END_SEARCH> ray_query;

    RayDesc desc;
    desc.Origin = origin;
    desc.TMin = tmin;
    desc.Direction = direction;
    desc.TMax = tmax;

    ray_query.TraceRayInline(
        scene_tlas[world_buffer_info.user_data.x],
        RAY_FLAG_NONE,
        0xff,
        desc
    );

    ray_query.Proceed();

    if (ray_query.CommittedStatus() == COMMITTED_TRIANGLE_HIT)
    {
        return true;
    }

    return false;
}

[shader("raygeneration")]
void scene_raygen_shader()
{
    //uv and ndc from dispatch dim
    float2 uv = (float2)DispatchRaysIndex() / (float2)DispatchRaysDimensions();
    float2 ndc = uv * 2.0 - 1.0;

    float2 output_location = DispatchRaysIndex();
    output_location.y = DispatchRaysDimensions().y - output_location.y;

    // unproject ray
    float4 near = float4(ndc.x, ndc.y, 0.0, 1.0);
    float4 far = float4(ndc.x, ndc.y, 1.0, 1.0);
    
    float4 wnear = mul(inverse_wvp, near);
    wnear /= wnear.w;
    
    float4 wfar = mul(inverse_wvp, far);
    wfar /= wfar.w;

    // ray desc
    RayDesc ray;
    ray.Origin = wnear.xyz;
    ray.Direction = normalize(wfar.xyz - wnear.xyz);
    ray.TMin = 0.001;
    ray.TMax = 10000.0;

    RayPayload payload = default_payload();
    TraceRay(
        scene_tlas[resource_indices.y], 
        RAY_FLAG_NONE, 
        0xff, 
        0,
        2,
        0, 
        ray, 
        payload
    );

    for(int i = 0; i < 10; ++i)
    {
        if(payload.has_bounce_ray)
        {
            RayDesc bounce_ray = payload.bounce_ray;
            int bounce_count = payload.bounce_count;

            payload = default_payload();
            payload.bounce_count = bounce_count;
            TraceRay(
                scene_tlas[resource_indices.y], 
                RAY_FLAG_NONE, 
                0xff, 
                0,
                2,
                0, 
                bounce_ray, 
                payload
            );

            // payload.col = float4(bounce_ray.Direction * 0.5 + 0.5, 1.0);
        }
        else
        {
            break;
        }
    }

    rw_textures[resource_indices.x][output_location] = payload.col;
}

struct GeometryLookup
{
    uint ib_srv;
    uint vb_srv;
    uint ib_stride;
    uint material_type;
};

StructuredBuffer<GeometryLookup> instance_geometry_lookups : register(t1, space0);
StructuredBuffer<uint> instance_index_buffers[] : register(t0, space13);
StructuredBuffer<vs_input_mesh> instance_vertex_buffers[] : register(t0, space14);

[shader("closesthit")]
void scene_closest_hit_shader(inout RayPayload payload, in BuiltInTriangleIntersectionAttributes attr)
{
    uint iid = InstanceID();
    uint tid = PrimitiveIndex();
    GeometryLookup lookup = instance_geometry_lookups[iid];

    // lookup vertex attribs
    uint index = (tid * 3);

    uint i0, i1, i2;
    if(lookup.ib_stride == 2)
    {
        uint half_index = index / 2;
        uint shift = index % 2;

        if(shift == 0)
        {
            i0 = instance_index_buffers[lookup.ib_srv][half_index] & 0xffff;
            i1 = (instance_index_buffers[lookup.ib_srv][half_index]) >> 16;
            i2 = instance_index_buffers[lookup.ib_srv][half_index + 1] & 0xffff;
        }
        else
        {
            i0 = (instance_index_buffers[lookup.ib_srv][half_index] >> 16);
            i1 = instance_index_buffers[lookup.ib_srv][half_index + 1] & 0xffff;
            i2 = (instance_index_buffers[lookup.ib_srv][half_index + 1] >> 16);
        }
    }
    else if(lookup.ib_stride == 4)
    {
        i0 = instance_index_buffers[lookup.ib_srv][index];
        i1 = instance_index_buffers[lookup.ib_srv][index + 1];
        i2 = instance_index_buffers[lookup.ib_srv][index + 2];
    }

    float u = attr.barycentrics.x;
    float v = attr.barycentrics.y;
    float w = 1.0f - u - v;

    vs_input_mesh v0 = instance_vertex_buffers[lookup.vb_srv][i0];
    vs_input_mesh v1 = instance_vertex_buffers[lookup.vb_srv][i1];
    vs_input_mesh v2 = instance_vertex_buffers[lookup.vb_srv][i2];

    //float3 geo_normal = normalize(v0.normal * u + v1.normal * v + v2.normal * w);

    float3 geo_normal = normalize(v0.normal + (v1.normal - v0.normal) * u + (v2.normal - v0.normal) * v);

    if(lookup.material_type == 1)
    {
        // ray info
        float3 r0 = WorldRayOrigin();
        float3 rd = WorldRayDirection();
        float rt = RayTCurrent();

        // intersction point
        float3 ip = r0 + rd * rt;

        RayDesc ray;
        ray.Origin = ip + geo_normal * 0.001;
        ray.Direction = reflect(rd, geo_normal);
        ray.TMin = 0.001;
        ray.TMax = 10000.0;

        payload.has_bounce_ray = true;
        payload.bounce_ray = ray;
        payload.bounce_count++;

        return;
    }
    else if(lookup.material_type == 2)
    {
        // ray info
        float3 r0 = WorldRayOrigin();
        float3 rd = WorldRayDirection();
        float rt = RayTCurrent();

        // intersction point
        float3 ip = r0 + rd * rt;

        float refidx = 1.0003 / 1.52;
        if(payload.bounce_count == 1)
        {
            //refidx = 1.52 / 1.0003;
        }

        float3 ray_dir = refract(rd, geo_normal, refidx);
        float3 ray_start = ip + rd * 0.001;

        if(length(ray_dir) == 0.0)
        {
            ray_dir = reflect(rd, geo_normal);
            ray_start = ip + geo_normal * 0.001;
        }

        RayDesc ray;
        ray.Origin = ray_start;
        ray.Direction = ray_dir;
        ray.TMin = 0.001;
        ray.TMax = 10000.0;

        if(payload.bounce_count > 0)
        {
            ray.Direction = rd;
        }

        payload.has_bounce_ray = true;
        payload.bounce_ray = ray;
        payload.bounce_count++;

        return;
    }
    
    float2 tx = v0.texcoord * u + v1.texcoord * w + v2.texcoord * v;

    // checkerboard uv
    float tu = (tx.x);
    float tv = (tx.y);

    float size = 8.0;
    float x = tu * size;
    float y = tv * size;

    float ix;
    modf(x, ix);
    float rx = fmod(ix, 2.0) == 0.0 ? 0.0 : 1.0;

    float iy;
    modf(y, iy);
    float ry = fmod(iy, 2.0) == 0.0 ? 0.0 : 1.0;

    float rxy = rx + ry > 1.0 ? 0.0 : rx + ry;

    float3 checkerboard = rxy < 0.001 ? 0.66 : 1.0;
    
    payload.col = float4(geo_normal, 1.0);

    payload.col.xyz = payload.col.xyz * 0.5 + 0.5 * checkerboard;

}

[shader("anyhit")]
void shadow_any_hit_shader(inout RayPayload payload, in BuiltInTriangleIntersectionAttributes attr)
{
    float3 r0 = WorldRayOrigin();
    float3 rd = WorldRayDirection();
    float rt = RayTCurrent();

    // intersction point
    float3 ip = r0 + rd * rt;

    payload.col = float4(ip, 1.0);
}

[shader("closesthit")]
void shadow_closest_hit_shader(inout RayPayload payload, in BuiltInTriangleIntersectionAttributes attr)
{
    float3 r0 = WorldRayOrigin();
    float3 rd = WorldRayDirection();
    float rt = RayTCurrent();

    // intersction point
    float3 ip = r0 + rd * rt;

    payload.col = float4(ip, 1.0);
}

[shader("miss")]
void scene_miss_shader(inout RayPayload payload)
{
    payload.col = float4(0.0, 0.0, 0.0, 0.0);
}

ps_output ps_mesh_lit_rt_shadow(vs_output input) {
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
        float rl = length(light.pos - input.world_pos.xyz);

        float diffuse = lambert(l, n);
        float specular = cook_torrance(l, n, v, roughness, k);

        float atteniuation = point_light_attenuation(
            light.pos,
            light.radius,
            input.world_pos.xyz
        );

        float4 light_colour = atteniuation * light.colour * diffuse;
        light_colour += atteniuation * light.colour * specular;

        bool occluded = is_occluded(input.world_pos.xyz + input.normal * 0.1, -l, 0.1, rl + 0.1);
        
        if(!occluded) {
            output.colour += light_colour;
        }
    }

    return output;
}