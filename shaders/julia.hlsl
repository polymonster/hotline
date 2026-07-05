//
// animated julia set computed into a read-write texture, then blitted to the back buffer
//

// bindless read-write texture array - the compute kernel writes the fractal here
RWTexture2D<float4> rw_texture[] : register(u0, space0);

cbuffer julia_constants : register(b0) {
    uint  output_index;     // uav index of the rw_texture to write into
    uint  output_width;
    uint  output_height;
    float cr;               // animated complex constant (real)
    float ci;               // animated complex constant (imaginary)
};

// simple hue ramp for colouring iteration counts
float3 palette(float t) {
    return 0.5 + 0.5 * cos(6.28318 * (float3(1.0, 1.0, 1.0) * t + float3(0.0, 0.33, 0.67)));
}

[numthreads(8, 8, 1)]
void cs_julia(uint2 did : SV_DispatchThreadID) {
    if(did.x >= output_width || did.y >= output_height) {
        return;
    }

    // map pixel to complex plane [-1.5, 1.5] x [-1.0, 1.0]
    float aspect = float(output_width) / float(output_height);
    float2 uv = float2(did.xy) / float2(output_width, output_height);
    float2 z;
    z.x = (uv.x * 2.0 - 1.0) * 1.5 * aspect;
    z.y = (uv.y * 2.0 - 1.0) * 1.5;

    const int max_iter = 256;
    int i = 0;
    for(; i < max_iter; ++i) {
        float x = z.x * z.x - z.y * z.y + cr;
        float y = 2.0 * z.x * z.y + ci;
        z = float2(x, y);
        if(dot(z, z) > 4.0) {
            break;
        }
    }

    float t = float(i) / float(max_iter);
    float3 colour = (i == max_iter) ? float3(0.0, 0.0, 0.0) : palette(t);
    rw_texture[output_index][did.xy] = float4(colour, 1.0);
}

//
// fullscreen blit of the compute output to the back buffer (bindless texture sample)
//

struct vs_input {
    float2 position : POSITION;
    float2 texcoord : TEXCOORD;
};

struct ps_input {
    float4 position : SV_POSITION;
    float2 texcoord : TEXCOORD;
};

cbuffer blit_constants : register(b0) {
    int4 blit_srv_index;    // srv index of the compute output texture
};

Texture2D blit_textures[] : register(t0);
SamplerState blit_sampler : register(s0);

ps_input vs_blit(vs_input input) {
    ps_input output;
    output.position = float4(input.position, 0.0, 1.0);
    output.texcoord = input.texcoord;
    return output;
}

float4 ps_blit(ps_input input) : SV_Target {
    return blit_textures[blit_srv_index[0]].Sample(blit_sampler, input.texcoord);
}
