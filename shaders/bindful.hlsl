struct vs_input {
    float3 position : POSITION; 
    float4 colour : COLOR;
};

struct ps_input {
    float4 position : SV_POSITION;
    float4 uv : TEXCOORD0;
};

Texture2D texture0 : register(t0);
Texture2D texture1 : register(t1);
Texture2D texture2 : register(t2);
Texture2D texture3 : register(t3);

SamplerState sampler0 : register(s0);

ps_input vs_main(vs_input input) {
    ps_input output;
    output.position = float4(input.position, 1.0);
    output.uv = input.colour;
    return output;
}

float4 ps_main(ps_input input) : SV_Target {
    float4 final = float4(0.0, 0.0, 0.0, 1.0);
    float2 uv = input.uv.xy * float2(1.0, -1.0);

    float4 r0 = texture0.Sample(sampler0, uv * 2.0);
    float4 r1 = texture1.Sample(sampler0, uv * 2.0);
    float4 r2 = texture2.Sample(sampler0, uv * 2.0);
    float4 r3 = texture3.Sample(sampler0, uv * 2.0);

    if(input.uv.x < 0.5 && input.uv.y < 0.5)
    {
        final = r0 * r0.a;
    }
    else if(input.uv.x < 0.5 && input.uv.y > 0.5)
    {
        final = r1 * r1.a;
    }
    else if(input.uv.x > 0.5 && input.uv.y > 0.5)
    {
        final = r2 * r2.a;
    }
    else if(input.uv.x > 0.5 && input.uv.y < 0.5)
    {
        final = r3 * r3.a;
    }

    return final;
}
