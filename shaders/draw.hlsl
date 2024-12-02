struct vs_output {
    float4 position: SV_POSITION0;
    float4 world_pos: TEXCOORD0;
    float4 texcoord: TEXCOORD1;
    float4 colour: TEXCOORD2;
    float3 normal: TEXCOORD3;
};

//
// example drawing mesh with push constants for the camera view matrix
//

vs_output vs_mesh_identity(vs_input_mesh input) {
    float4 pos = float4(input.position.xyz, 1.0);

    vs_output output;
    output.position = mul(view_projection_matrix, pos);
    output.world_pos = pos;
    output.texcoord = float4(input.texcoord, 0.0, 0.0);
    output.colour = float4(1.0, 1.0, 1.0, 1.0);
    output.normal = input.normal.xyz;
    
    return output;
}

//
// example drawing mesh with push constants for the camera view matrix
// and push constants for the draw calls world matrix

vs_output vs_mesh(vs_input_mesh input) {
    float4 pos = float4(input.position.xyz, 1.0);
    pos.xyz = mul(world_matrix, pos);

    vs_output output;
    output.position = mul(view_projection_matrix, pos);
    output.world_pos = pos;
    output.texcoord = float4(input.texcoord, 0.0, 0.0);
    output.colour = material_colour;
    output.normal = input.normal.xyz;
    
    return output;
}

//
// textureles checkboard shader
// 

float4 ps_checkerboard(vs_output input) : SV_Target {
    float4 output = float4(input.normal.xyz * 0.5 + 0.5, 1.0);

    // checkerboard uv
    float u = (input.texcoord.x);
    float v = (input.texcoord.y);

    float size = 8.0;
    float x = u * size;
    float y = v * size;

    float ix;
    modf(x, ix);
    float rx = fmod(ix, 2.0) == 0.0 ? 0.0 : 1.0;

    float iy;
    modf(y, iy);
    float ry = fmod(iy, 2.0) == 0.0 ? 0.0 : 1.0;

    float rxy = rx + ry > 1.0 ? 0.0 : rx + ry;

    output.rgb *= rxy < 0.001 ? 0.66 : 1.0;

    // debug switches
    // u gradient
    //colour.rgb = uv_gradient(u % 1.0);
    
    // v gradient
    //colour.rgb = uv_gradient(v % 1.0);

    return output;
}

//
// constant colour for wireframe overlay
//

ps_output ps_wireframe(vs_output input) {
    ps_output output;
    output.colour = float4(0.2, 0.2, 0.2, 1.0);
    return output;
}

//
// constant colour with push constants specified colour pased from vs
//

ps_output ps_constant_colour(vs_output input) {
    ps_output output;
    output.colour = input.colour;
    return output;
}