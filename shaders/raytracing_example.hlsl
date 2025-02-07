// This is ported from the Direct3D12 samples: https://github.com/microsoft/directx-graphics-samples/tree/master/Samples/Desktop/D3D12Raytracing

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
    float3 ray_dir = float3(0, 0, 1);
    float3 origin = float3(
        lerp(raygen_constants.viewport.left, raygen_constants.viewport.right, lerp_values.x),
        lerp(raygen_constants.viewport.top, raygen_constants.viewport.bottom, lerp_values.y),
        0.0f);


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
        RayPayload payload = { float4(0, 0, 1, 0) };
        TraceRay(scene, RAY_FLAG_NONE, ~0, 0, 1, 0, ray, payload);

        // Write the raytraced color to the output texture.
        output_target[DispatchRaysIndex().xy] = payload.color;
    }
    else
    {
        // Render interpolated DispatchRaysIndex outside the stencil window
        output_target[DispatchRaysIndex().xy] = float4(lerp_values, 0, 1);
    }
}

[shader("closesthit")]
void closest_hit_shader(inout RayPayload payload, in BuiltInTriangleIntersectionAttributes attr)
{
    float3 barycentrics = float3(1 - attr.barycentrics.x - attr.barycentrics.y, attr.barycentrics.x, attr.barycentrics.y);
    payload.color = float4(barycentrics, 1);
}

[shader("miss")]
void miss_shader(inout RayPayload payload)
{
    payload.color = float4(1, 0, 0, 1);
}