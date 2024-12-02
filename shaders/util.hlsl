//
// utilties to compile into the core hotline engine
//

cbuffer mip_info : register(b0) {
    uint read;
    uint write;
};

RWTexture2D<float4> rw_texture[] : register(u0, space0);
groupshared uint4 group_accumulated[5];

[numthreads(32, 32, 1)]
void cs_mip_chain_texture2d(uint2 did: SV_DispatchThreadID) {
    uint2 offsets[9];
    offsets[0] = uint2( 0,  0);
    offsets[1] = uint2(-1, -1);
    offsets[2] = uint2(-1,  0);
    offsets[3] = uint2(-1,  1);
    offsets[4] = uint2( 0,  1);
    offsets[5] = uint2( 1,  1);
    offsets[6] = uint2( 1,  0);
    offsets[7] = uint2( 1, -1);
    offsets[8] = uint2( 0, -1);

    pmfx_touch(group_accumulated[0]);

    float4 level_up = float4(0.0, 0.0, 0.0, 0.0);
    
    for(int i = 0; i < 9; ++i) {
        level_up += rw_texture[read][did.xy * 2];
    }
    
    rw_texture[write][did.xy] = level_up / 9.0;
}

//
// clear cubemap background
//

struct vs_input_2d_texcoord {
    float2 position : POSITION;
    float2 texcoord: TEXCOORD;
};

struct vs_output_ndc {
    float4 position: SV_POSITION0;
    float2 ndc: TEXCOORD;
};

vs_output_ndc vs_ndc(vs_input_2d_texcoord input) {
    vs_output_ndc output;
    output.position = float4(input.position.xy, 0.0, 1.0);
    output.ndc = input.position.xy;
    return output;
}

cbuffer cubemap_clear_constants : register(b0) {
    float4x4 inverse_wvp;
};

TextureCube cubemap : register(t0);
SamplerState sampler_wrap_linear : register(s0); 

float4 ps_cubemap_clear(vs_output_ndc input) : SV_Target {
    // unproject ray
    float2 ndc = input.ndc;
    float4 near = float4(ndc.x, ndc.y, 0.0, 1.0);
    float4 far = float4(ndc.x, ndc.y, 1.0, 1.0);
    
    float4 wnear = mul(inverse_wvp, near);
    wnear /= wnear.w;
    
    float4 wfar = mul(inverse_wvp, far);
    wfar /= wfar.w;

    float3 rd = normalize(wfar.xyz - wnear.xyz);

    return cubemap.Sample(sampler_wrap_linear, rd);
}