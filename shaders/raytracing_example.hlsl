// This is ported from the Direct3D12 samples, with some slight modifications: 
// Original: https://github.com/microsoft/directx-graphics-samples/tree/master/Samples/Desktop/D3D12Raytracing

struct Viewport
{
    float left;
    float top;
    float right;
    float bottom;
};

struct RayGenConstantBuffer
{
    Viewport viewport;
    Viewport stencil;
};

struct RayPayload
{
    float4 color;
};

RaytracingAccelerationStructure         scene               : register(t0, space0);
RWTexture2D<float4>                     output_target       : register(u0);
ConstantBuffer<RayGenConstantBuffer>    raygen_constants    : register(b0);

bool inside_viewport(float2 p, Viewport viewport)
{
    return (p.x >= viewport.left && p.x <= viewport.right)
        && (p.y >= viewport.top && p.y <= viewport.bottom);
}

[shader("raygeneration")]
void raygen_shader()
{
    float2 lerp_values = (float2)DispatchRaysIndex() / (float2)DispatchRaysDimensions();

    // Orthographic projection since we're raytracing in screen space.
    float3 ray_dir = float3(0.0, 0.0, 1.0);
    float3 origin = float3(
        lerp(
            raygen_constants.viewport.left, 
            raygen_constants.viewport.right, 
            lerp_values.x
        ),
        lerp(
            raygen_constants.viewport.top, 
            raygen_constants.viewport.bottom, 
            lerp_values.y
        ),
        0.0f
    );


    if (inside_viewport(origin.xy, raygen_constants.stencil))
    {
        // Trace the ray.
        // Set the ray's extents.
        RayDesc ray;
        ray.Origin = origin;
        ray.Direction = ray_dir;
        
        // Set TMin to a non-zero small value to avoid aliasing issues due to floating - point errors.
        // TMin should be kept small to prevent missing geometry at close contact areas.
        ray.TMin = 0.001;
        ray.TMax = 10000.0;
        RayPayload payload = { float4(0.0, 0.0, 1.0, 0.0) };
        TraceRay(scene, RAY_FLAG_NONE, ~0, 0.0, 1.0, 0.0, ray, payload);

        // Write the raytraced color to the output texture.
        output_target[DispatchRaysIndex().xy] = payload.color;
    }
    else
    {
        // Render interpolated DispatchRaysIndex outside the stencil window
        output_target[DispatchRaysIndex().xy] = float4(lerp_values, 1.0, 1.0);
    }
}

[shader("closesthit")]
void closest_hit_shader(inout RayPayload payload, in BuiltInTriangleIntersectionAttributes attr)
{
    float3 barycentrics = float3(1 - attr.barycentrics.x - attr.barycentrics.y, attr.barycentrics.x, attr.barycentrics.y);
    payload.color = float4(float3(1.0, 1.0, 1.0) - barycentrics, 1.0);
}

[shader("miss")]
void miss_shader(inout RayPayload payload)
{
    payload.color = float4(0.5, 0.5, 0.5, 1.0);
}

//
// inline raytracing version of raygen_shader, rays are traced with RayQuery from a compute shader instead of a
// raytracing pipeline, so it also runs on metal which only supports inline raytracing
//

[numthreads(8, 8, 1)]
void cs_raytrace_inline(uint2 tid : SV_DispatchThreadID)
{
    uint2 dims;
    output_target.GetDimensions(dims.x, dims.y);
    if (tid.x >= dims.x || tid.y >= dims.y)
    {
        return;
    }

    float2 lerp_values = (float2)tid / (float2)dims;

    // Orthographic projection since we're raytracing in screen space.
    float3 ray_dir = float3(0.0, 0.0, 1.0);
    float3 origin = float3(
        lerp(raygen_constants.viewport.left, raygen_constants.viewport.right, lerp_values.x),
        lerp(raygen_constants.viewport.top, raygen_constants.viewport.bottom, lerp_values.y),
        0.0f
    );

    if (inside_viewport(origin.xy, raygen_constants.stencil))
    {
        RayDesc ray;
        ray.Origin = origin;
        ray.Direction = ray_dir;
        ray.TMin = 0.001;
        ray.TMax = 10000.0;

        RayQuery<RAY_FLAG_NONE> ray_query;
        ray_query.TraceRayInline(scene, RAY_FLAG_NONE, ~0, ray);
        ray_query.Proceed();

        if (ray_query.CommittedStatus() == COMMITTED_TRIANGLE_HIT)
        {
            // closest hit
            float2 bary = ray_query.CommittedTriangleBarycentrics();
            float3 barycentrics = float3(1 - bary.x - bary.y, bary.x, bary.y);
            output_target[tid] = float4(float3(1.0, 1.0, 1.0) - barycentrics, 1.0);
        }
        else
        {
            // miss
            output_target[tid] = float4(0.5, 0.5, 0.5, 1.0);
        }
    }
    else
    {
        // Render interpolated thread index outside the stencil window
        output_target[tid] = float4(lerp_values, 1.0, 1.0);
    }
}

//
// fullscreen blit of the raytracing output to the back buffer
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
    int4 blit_srv_index;    // srv index of the raytracing output texture
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
