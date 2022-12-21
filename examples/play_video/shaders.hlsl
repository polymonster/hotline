struct PSInput
{
    float4 position : SV_POSITION;
    float4 uv : TEXCOORD0;
};

cbuffer PushConstants : register(b0, space0)
{
    float4 values;
};

PSInput VSMain(float4 position : POSITION, float4 uv : TEXCOORD0)
{
    PSInput result;

    result.position = position;
    result.uv = uv;

    return result;
}

Texture2D texture0[6] : register(t0);
SamplerState sampler0 : register(s0);

float4 PSMain(PSInput input) : SV_TARGET
{
    return texture0[int(values.x)].Sample(sampler0, input.uv);
}