struct PSInput
{
    float4 position : SV_POSITION;
    float4 color : COLOR;
};

PSInput VSMain(float4 position : POSITION, float4 color : COLOR)
{
    PSInput result;

    result.position = position;
    result.color = color;

    return result;
}

Texture2D texture0 : register(t0);

float4 PSMain(PSInput input) : SV_TARGET
{
    float4 res = texture0.Load(int3(input.color.r, input.color.g, 0));
    return res;
}
