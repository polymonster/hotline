//
// vertex shader applies heightmap
//

vs_output vs_heightmap(vs_input_mesh input) {
    vs_output output;

	float4 pos = float4(input.position.xyz, 1.0);
    pos.xyz = mul(world_matrix, pos);

    float step = 1024.0 / 10.0;
    float height = 200.0;

    float3 p1 = pos.xyz;
    
    float h = fbm(p1.xz + fbm(p1.xz + fbm(p1.xz, 6), 6), 6) * height;
    p1.y += h;

    // take a few pos to calculate a normal
    float3 p2 = pos.xyz + float3(step, 0.0, 0.0);
    float3 p3 = pos.xyz + float3(step, 0.0, step);

    p2.y += fbm(p2.xz + fbm(p2.xz + fbm(p2.xz, 6), 6), 6) * height;
    p3.y += fbm(p3.xz + fbm(p3.xz + fbm(p3.xz, 6), 6), 6) * height;

    float3 n = -cross(normalize(p2 - p1), normalize(p3 - p1));

    output.position = mul(view_projection_matrix, float4(p1, pos.w));
    output.colour = float4(h, h, h, 1.0) / float4(200.0, 200.0, 200.0, 1.0);
    output.world_pos = pos;
    output.texcoord = float4(input.texcoord, 0.0, 0.0);
    output.normal = n;

    return output;
}

//
// pixel shader writes mrt
//

struct ps_output_mrt {
    float4 albedo: SV_Target0;
    float4 normal: SV_Target1;
    float4 position: SV_Target2;
};

ps_output_mrt ps_heightmap_example_mrt(vs_output input) {
    ps_output_mrt output;
    output.albedo = float4(uv_gradient(input.colour.r), 1.0);
    output.normal = float4(input.normal.xyz * 0.5 + 0.5, 1.0);
    output.position = float4((input.position.xyz / float3(1024.0, 1024.0, 1024.0)) * 0.5 + 0.5, 1.0);
    return output;
}

//
// compute shader reads mrt msaa output and composites into 4 quadrants
//

[numthreads(32, 32, 1)]
void cs_heightmap_mrt_resolve(uint2 did: SV_DispatchThreadID, uint2 group_id: SV_GroupID) {
    // grab the output dimension from input0 (which we write to)
    uint2 half_dim = resources.input0.dimension.xy / 2;
    
    // render into 4 quadrants
    float4 final = float4(0.0, 0.0, 0.0, 0.0);
    if(did.x < half_dim.x && did.y < half_dim.y) {
        // albedo
        final = msaa8x_textures[resources.input1.index].Load(did * 2, 0);
    }
    else if (did.x >= half_dim.x && did.y < half_dim.y) {
        // normals
        uint2 sc = did;
        sc.x -= half_dim.x;
        final = msaa8x_textures[resources.input2.index].Load(sc * 2, 0);
    }
    else if (did.x < half_dim.x && did.y >= half_dim.y) {
        // normals
        uint2 sc = did;
        sc.y -= half_dim.y;
        final = msaa8x_textures[resources.input3.index].Load(sc * 2, 0);
    }
    else if (did.x >= half_dim.x && did.y >= half_dim.y) {
        // depth
        uint2 sc = did;
        sc -= half_dim;
        final = msaa8x_textures[resources.input4.index].Load(sc * 2, 0);
    }

    rw_textures[resources.input0.index][did] = final;
}

[numthreads(32, 32, 1)]
void cs_display_mips(uint2 did: SV_DispatchThreadID, uint2 group_id: SV_GroupID) {
    // start at black
    float4 output = float4(0.0, 0.0, 0.0, 0.0);

    //loop through mips
    int2 mip_coord = did.xy;
    int mipw = resources.input1.dimension.x;
    int miph = resources.input1.dimension.y;
    int mip_level = 0;
    while(mipw > 1 && miph > 1) {
        if(did.x <= mipw && did.y <= miph) {
            output = textures[resources.input1.index].Load(int3(did.xy, mip_level));
        }

        mip_level++;
        mipw = max(mipw / 2, 1);
        miph = max(miph / 2, 1);
        mip_coord /= 2;
    }

    rw_textures[resources.input0.index][did] = output;
}