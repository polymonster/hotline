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

cbuffer push_constants : register(b0, space0) {
    float4 my_rgba;
};

cbuffer double_constants : register(b1, space0) {
    float4 my_rgba2;
};

struct data {
    float4 rgba;
};

Texture2D texture0[10] : register(t0);
ConstantBuffer<data> cbuffer0[10] : register(b2);
SamplerState sampler0 : register(s0);

ps_input vs_main(vs_input input) {
    ps_input output;
    output.position = float4(input.position, 1.0);
    output.colour = input.colour;
    return output;
}

ps_output ps_main(ps_input input) {
    ps_output output;

    float4 final = float4(0.0, 0.0, 0.0, 0.0);
    float2 uv = input.colour.rg * float2(1.0, -1.0);

    float4 r0 = texture0[1].Sample(sampler0, uv * 2.0);
    float4 r1 = texture0[2].Sample(sampler0, uv * 2.0);
    float4 r2 = texture0[3].Sample(sampler0, uv * 2.0);
    float4 r3 = texture0[4].Sample(sampler0, uv * 2.0);
    r3 *= my_rgba;

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
    rwtex[6][did.xy] = float4(0.0, 0.0, 0.0, 1.0);
}