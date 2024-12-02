struct vs_input_2d {
    float2 position : POSITION;
    float4 colour: TEXCOORD;
};

struct vs_input_3d {
    float3 position : POSITION;
    float4 colour: TEXCOORD;
};

struct vs_output {
    float4 position : SV_POSITION0;
    float4 colour: TEXCOORD;
};

struct vs_input_2d_texcoord {
    float2 position : POSITION;
    float2 texcoord: TEXCOORD;
};

struct vs_output_texcoord {
    float4 position: SV_POSITION0;
    float2 texcoord: TEXCOORD;
};

struct ps_output {
    float4 colour : SV_Target;
};

cbuffer view_push_constants : register(b0) {
    float4x4 projection_matrix;
};

struct vs_input_mesh {
    float3 position : POSITION;
    float2 texcoord: TEXCOORD0;
    float3 normal : TEXCOORD1;
    float3 tangent : TEXCOORD2;
    float3 bitangent : TEXCOORD3;
};

vs_output vs_2d( vs_input_2d input ) {
    vs_output output;
    
    output.position = mul(projection_matrix, float4(input.position.xy, 0.0, 1.0));
    output.colour = input.colour;
    
    return output;
}

ps_output ps_main( vs_output input ) {
    ps_output output;
    
    output.colour = input.colour;

    return output;
}

vs_output vs_3d( vs_input_3d input )
{
    vs_output output;

	float4 pos = float4(input.position.xyz, 1.0);
    output.position = mul(projection_matrix, pos);
    output.colour = input.colour;
    
    return output;
}

Texture2D<float4> blit_texture : register(t1);

vs_output_texcoord vs_blit(vs_input_2d_texcoord input) {
    vs_output_texcoord output;
    output.position = float4(input.position.xy, 0.0, 1.0);
    output.texcoord = input.texcoord;
    return output;
}

cbuffer blit_push_constants : register(b0) {
    float2 blit_dimension;
};

ps_output ps_blit(vs_output_texcoord input) {
    ps_output output;
    output.colour = blit_texture.Load(int3(input.texcoord.x * blit_dimension.x, input.texcoord.y * blit_dimension.y, 0));
    return output;
}