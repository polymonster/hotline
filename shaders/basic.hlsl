struct vs_output {
    float4 position : SV_POSITION0;
    float4 colour: TEXCOORD0;
    float2 texcoord: TEXCOORD1;
};

struct ps_output {
    float4 colour : SV_Target;
};

cbuffer view_push_constants : register(b0) {
    float4x4 projection_matrix;
};

cbuffer draw_push_constants : register(b1) {
    float4x4 world_matrix;
};

struct vs_input_mesh {
    float3 position : POSITION;
    float2 texcoord: TEXCOORD0;
    float3 normal : TEXCOORD1;
    float3 tangent : TEXCOORD2;
    float3 bitangent : TEXCOORD3;
};

float random(float2 st) {
    return frac(sin(dot(st.xy, float2(12.9898,78.233))) * 43758.5453123);
}

float noise(float2 st) {
    // https://www.shadertoy.com/view/4dS3Wd
    float2 i = floor(st);
    float2 f = frac(st);

    // Four corners in 2D of a tile
    float a = random(i);
    float b = random(i + float2(1.0, 0.0));
    float c = random(i + float2(0.0, 1.0));
    float d = random(i + float2(1.0, 1.0));

    float2 u = f * f * (3.0 - 2.0 * f);

    return lerp(a, b, u.x) + (c - a)* u.y * (1.0 - u.x) + (d - b) * u.x * u.y;
}

#define OCTAVES 6
float fbm(float2 st) {
    // Initial values
    float value = 0.0;
    float amplitude = 0.3;
    float frequency = 0.0;

    // Loop of octaves
    for (int i = 0; i < OCTAVES; i++) {
        value += amplitude * noise(st);
        st *= 3.0;
        amplitude *= 0.8;
    }
    return value;
}

vs_output vs_mesh(vs_input_mesh input) {
    vs_output output;

	float4 pos = float4(input.position.xyz, 1.0);
    
    (world_matrix);
    pos = mul(pos, world_matrix);
    output.position = mul(pos, projection_matrix);

    output.colour = float4(input.normal.xyz * 0.5 + 0.5, 1.0);
    output.texcoord = input.texcoord;
    
    return output;
}

vs_output vs_heightmap(vs_input_mesh input) {
    vs_output output;

	float4 pos = float4(input.position.xyz, 1.0);

    float2 p = pos.xz;
    float h = fbm(p + fbm( p + fbm(p)));
    pos.y += h;

    (world_matrix);
    pos = mul(pos, world_matrix);
    output.position = mul(pos, projection_matrix);
    output.colour = float4(h, h, h, 1.0);
    
    return output;
}

vs_output vs_billboard(vs_input_mesh input) {
    vs_output output;

	float4 pos = float4(input.position.xyz, 1.0);
    
    (world_matrix);
    pos = mul(pos, world_matrix);

    float4x4 bbmatrix = projection_matrix;

    bbmatrix[0][0] = 1.0;
    bbmatrix[0][1] = 0.0;
    bbmatrix[0][2] = 0.0;

    bbmatrix[1][0] = 0.0;
    bbmatrix[1][1] = 1.0;
    bbmatrix[1][2] = 0.0;

    bbmatrix[2][0] = 0.0;
    bbmatrix[2][1] = 0.0;
    bbmatrix[2][2] = 1.0;

    output.position = mul(pos, bbmatrix);

    output.colour = float4(input.normal.xyz, 1.0);
    
    return output;
}

ps_output ps_main(vs_output input) {
    ps_output output;
    
    output.colour = input.colour;

    return output;
}

ps_output ps_wireframe(vs_output input) {
    ps_output output;
    output.colour = float4(0.2, 0.2, 0.2, 1.0);
    return output;
}

float3 uv_gradient(float x) {
    float3 rgb_uv = float3(0.0, 0.0, 0.0);
    float grad = x % 1.0;
    if (grad < 0.333) {
        rgb_uv = lerp(float3(1.0, 0, 0.0), float3(0.0, 1.0, 0.0), grad * 3.333);
    }
    else if (grad < 0.666) {
        rgb_uv = lerp(float3(0.0, 1.0, 0.0), float3(0.0, 0.0, 1.0), (grad - 0.333) * 3.333);
    }
    else {
        rgb_uv = lerp(float3(0.0, 0.0, 1.0), float3(1.0, 0.0, 0.0), (grad - 0.666) * 3.333);
    }
    return rgb_uv;
}

ps_output ps_checkerboard(vs_output input) {
    ps_output output;
    output.colour = input.colour;

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

    output.colour.rgb *= rxy < 0.001 ? 0.66 : 1.0;

    // u gradient
    // output.colour.rgb = uv_gradient(u % 1.0);

    // v gradient
    // output.colour.rgb = uv_gradient(v % 1.0);

    output.colour.a = 1.0;
    return output;
}