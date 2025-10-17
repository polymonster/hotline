struct vs_input {
    float3 position : POSITION; 
    float4 colour : COLOR;
};

struct ps_input {
    float4 position : SV_POSITION;
    float4 colour : COLOR;
};

struct ps_output {
    float4 colour : SV_Target;
};

cbuffer letterbox : register(b0, space0) {
    float2 quad_scale;
};

cbuffer srv_indices : register(b1, space0) {
    int4 texture_handle;
};

Texture2D texture0[10] : register(t0);
SamplerState sampler0 : register(s0);

ps_input vs_main(vs_input input) {
    ps_input output;
    output.position = float4(input.position.xy * quad_scale, input.position.z, 1.0);
    output.colour = input.colour;
    return output;
}

ps_output ps_main(ps_input input) {
    ps_output output;

    float4 final = float4(0.0, 0.0, 0.0, 0.0);
    float2 uv = input.colour.rg * float2(1.0, -1.0);

    float4 r0 = texture0[texture_handle[0]].Sample(sampler0, uv * 2.0);
    float4 r1 = texture0[texture_handle[1]].Sample(sampler0, uv * 2.0);
    float4 r2 = texture0[texture_handle[2]].Sample(sampler0, uv * 2.0);
    float4 r3 = texture0[texture_handle[3]].Sample(sampler0, uv * 2.0);

    if(input.colour.r < 0.5 && input.colour.g < 0.5)
    {
        final = r0 * r0.a;
    }
    else if(input.colour.r < 0.5 && input.colour.g > 0.5)
    {
        final = r1 * r1.a;
    }
    else if(input.colour.r > 0.5 && input.colour.g > 0.5)
    {
        final = r2 * r2.a;
    }
    else if(input.colour.r > 0.5 && input.colour.g < 0.5)
    {
        final = r3 * r3.a;
    }

    output.colour = final;
    return output;
}

RWTexture2D<float4> rwtex[10] : register(u0);

[numthreads(16, 16, 1)]
void cs_main(uint3 did : SV_DispatchThreadID)
{
    rwtex[7][did.xy] = float4(0.0, 0.0, 0.0, 1.0);
}