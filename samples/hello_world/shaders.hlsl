struct PSInput
{
    float4 position : SV_POSITION;
    float4 color : COLOR;
};

cbuffer PushConstants : register(b0, space0)
{
    float4 my_rgba;
};

PSInput VSMain(float4 position : POSITION, float4 color : COLOR)
{
    PSInput result;

    result.position = position;
    result.color = color;

    return result;
}

Texture2D texture0[5] : register(t0);
SamplerState sampler0 : register(s0);

struct ccc
{
    float4 rgba;
};

ConstantBuffer<ccc> cbs[5] : register(b1);

float4 PSMain(PSInput input) : SV_TARGET
{
    float2 uv = input.color.rg * float2(1.0, -1.0);
    float4 r0 = texture0[0].Sample(sampler0, uv* 2.0);
    float4 r1 = texture0[1].Sample(sampler0, (uv * 2.0) + float2(0.0, 1.0));
    float4 r2 = texture0[2].Sample(sampler0, (uv * 2.0) + float2(1.0, 1.0));
    float4 r3 = texture0[5].Sample(sampler0, (input.color.rg * 2.0) + float2(1.0, 0.0));

    float4 final = float4(0.0, 0.0, 0.0, 0.0); 

    if(input.color.r < 0.5 && input.color.g < 0.5)
    {
        final = r0;
    }
    else if(input.color.r < 0.5 && input.color.g > 0.5)
    {
        final = r1;
    }
    else if(input.color.r > 0.5 && input.color.g > 0.5)
    {
        final = r2;
    }
    else if(input.color.r > 0.5 && input.color.g < 0.5)
    {
        final = r3;
    }

    final *= cbs[4].rgba;

    return final;
}

/*
RWTexture2D<float4> rwtex : register(u1);

[numthreads(16, 16, 1)]
void CSMain(uint2 gid : SV_DispatchThreadID) {
    float4 cur = texture0[1].Load(gid.x, gid.y);
    rwtex.Store()
}
*/

RWTexture2D<float> tex;

[numthreads(16, 16, 1)]
void CSMain(uint3 did : SV_DispatchThreadID)
{
    tex [did.xy] = float4(1.0, 1.0, 0.0, 1.0);
}