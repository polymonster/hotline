//
// 2d texture with bindless lookup
//

float4 ps_texture2d(vs_output input) : SV_Target {
    float2 tc = input.texcoord.xy;
    float4 albedo = textures[draw_indices.x].Sample(sampler_wrap_linear, tc);
    albedo *= albedo.a;

    return albedo;
}

//
// cubemap with bindless lookup
// 

float4 ps_cubemap(vs_output input) : SV_Target {
    float4 col = cubemaps[draw_indices.x]
        .SampleLevel(sampler_wrap_linear, input.normal, draw_indices.y);

    col.a = 1.0;
    return col;
}

//
// texture 2d array with bindless lookup
//

float4 ps_texture2d_array(vs_output input) : SV_Target {
    float2 tc = float2(input.texcoord.x, input.texcoord.y);

    float4 col = texture_arrays[draw_indices.x]
        .Sample(sampler_wrap_linear, float3(tc, draw_indices.y));

    if(col.a < 0.2) {
        discard;
    }

    return col;
}

//
// texture 3d with bindless lookup and ray marching
//

vs_output vs_texture3d(vs_input_mesh input) {
    vs_output output;

    float3x4 wm = world_matrix;
    
    float4 pos = float4(input.position.xyz, 1.0);
    pos.xyz = mul(wm, pos);

    output.position = mul(view_projection_matrix, pos);
    output.world_pos = pos;
    output.texcoord = float4(input.position, 0.0);
    output.colour = float4(input.normal.xyz, 1.0);
    output.normal = input.normal.xyz;
    
    return output;
}


ps_output ps_volume_texture_ray_march_sdf(vs_output input) {
    ps_output output;

    float3 v = input.texcoord.xyz;
    float3 chebyshev_norm = chebyshev_normalize(v);
    float3 uvw = chebyshev_norm * 0.5 + 0.5;
    
    float max_samples = 64.0;

    float3x3 inv_rot;
    inv_rot[0] = world_matrix[0].xyz;
    inv_rot[1] = world_matrix[1].xyz;
    inv_rot[2] = world_matrix[2].xyz;
    inv_rot = transpose(inv_rot);

    float3 ray_dir = normalize(input.world_pos.xyz - view_position.xyz);
                    
    ray_dir = mul(inv_rot, ray_dir);
    ray_dir = normalize(ray_dir);
                    
    float3 vddx = ddx( uvw );
    float3 vddy = ddy( uvw );
    
    float3 scale = float3(
        length(world_matrix[0].xyz), 
        length(world_matrix[1].xyz), 
        length(world_matrix[2].xyz)
    ) * 2.0;
        
    float d = volume_textures[draw_indices.x].SampleGrad(sampler_wrap_linear, uvw, vddx, vddy).r;
    
    float3 col = float3( 0.0, 0.0, 0.0 );
    float3 ray_pos = input.world_pos.xyz;
    float taken = 0.0;
    float3 min_step = (scale / max_samples); 
    
    for( int s = 0; s < int(max_samples); ++s )
    {        
        taken += 1.0 / max_samples;
                
        d = volume_textures[draw_indices.x].SampleGrad(sampler_wrap_linear, uvw, vddx, vddy).r;
            
        float3 step = ray_dir.xyz * float3(d / scale) * 0.5;
        
        uvw += step;
 
        if(uvw.x >= 1.0 || uvw.x <= 0.0)
            discard;
        
        if(uvw.y >= 1.0 || uvw.y <= 0.0)
            discard;
        
        if(uvw.z >= 1.0 || uvw.z <= 0.0)
            discard;
            
        if( d <= 0.3 )
            break;
    }
    float vd = (1.0 - d);
    output.colour.rgb = float3(vd*vd,vd*vd, vd*vd);
    output.colour.rgb = float3(taken, taken, taken);
    output.colour.a = 1.0;

    return output;
}

ps_output ps_volume_texture_ray_march(vs_output input) {
    ps_output output;
    
    float depth = 1.0;
    float max_samples = 256.0;
        
    float3 v = input.texcoord.xyz;
    float3 chebyshev_norm = chebyshev_normalize(v);
    float3 uvw = chebyshev_norm * 0.5 + 0.5;
    
    float3x3 inv_rot;
    inv_rot[0] = world_matrix[0].xyz;
    inv_rot[1] = world_matrix[1].xyz;
    inv_rot[2] = world_matrix[2].xyz;
    inv_rot = transpose(inv_rot);
        
    float3 ray_dir = normalize(input.world_pos.xyz - view_position.xyz);    
    ray_dir = mul( inv_rot, ray_dir );
    
    float3 ray_step = chebyshev_normalize(ray_dir.xyz) / max_samples;
                
    float depth_step = 1.0 / max_samples;
    
    float3 vddx = ddx( uvw );
    float3 vddy = ddy( uvw );
    
    for(int s = 0; s < int(max_samples); ++s )
    {
        output.colour = 
            volume_textures[draw_indices.x].SampleGrad(sampler_wrap_linear, uvw, vddx, vddy);
        
        if(output.colour.a != 0.0)
            break;
        
        depth -= depth_step;
        uvw += ray_step;
        
        if(uvw.x > 1.0 || uvw.x < 0.0)
            discard;
            
        if(uvw.y > 1.0 || uvw.y < 0.0)
            discard;
            
        if(uvw.z > 1.0 || uvw.z < 0.0)
            discard;
        
        if(s == int(max_samples)-1)
            discard;
    }
    
    output.colour.rgb *= lerp( 0.5, 1.0, depth );
            
    return output;
}

//
// compute shader writes noise to a 3D texture
//

[numthreads(8, 8, 8)]
void cs_write_texture3d(uint3 did : SV_DispatchThreadID) {
    float3 dim = float3(64.0, 64.0, 64.0);
    float3 grid_pos = did.xyz * 2.0 - float3(64.0, 64.0, 64.0);

    float4 sphere;
    float d = 1.0;

    float nxz = voronoise(did.xz / 8.0, 1.0, 0.0);
    float nxy = voronoise(did.xy / 8.0, 1.0, 0.0);
    float nyz = voronoise(did.yz / 8.0, 1.0, 0.0);

    float3 n = normalize(grid_pos);

    float nn = 
        abs(dot(n, float3(0.0, 1.0, 0.0))) * nxz 
        + abs(dot(n, float3(0.0, 0.0, 1.0))) * nxy
        + abs(dot(n, float3(1.0, 0.0, 0.0))) * nyz;

    rw_volume_textures[resources.input0.index][did.xyz] = float4(nn, 0.0, 0.0, nn < 0.9 ? 0.0 : 1.0);
}

//
// reflection
//

float4 ps_cubemap_reflect(vs_output input) : SV_Target {
    // cubemap space z is inverted, so we must invert z of the ray and also the z of the normal
    float3 rd = normalize(input.world_pos.xyz - view_position.xyz) * float3(1.0, 1.0, -1.0);
    float3 n = normalize(input.normal.xyz * float3(1.0, 1.0, -1.0));
    float3 r = reflect(rd, n);

    float4 col = cubemaps[draw_indices.x].SampleLevel(sampler_wrap_linear, r, draw_indices.y);

    col.a = 1.0;
    return col;
}