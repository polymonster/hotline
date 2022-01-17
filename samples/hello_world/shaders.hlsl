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

Texture2D texture0 : register(t0);
SamplerState sampler0 : register(s0);

float4 PSMain(PSInput input) : SV_TARGET
{
    float4 res = texture0.Sample(sampler0, input.color.rg) * my_rgba;
    return res;
}
