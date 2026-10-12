struct ms_output {
    float4 position : SV_POSITION0;
    float4 colour: COLOR;
};

float4 ps_main( ms_output input ) : SV_Target {
    return input.colour;
}

[numthreads(1, 1, 1)]
[outputtopology("triangle")]
void ms_main(
    uint gtid : SV_GroupThreadID,
    uint gid  : SV_GroupID,
    out indices uint3 tris[1],
    out vertices ms_output verts[3]
)
{
    float2 t[3];
    t[0] = float2(0.0, 0.25);
    t[1] = float2(0.25, -0.25);
    t[2] = float2(-0.25, -0.25);

    float4 cols[3];
    cols[0] = float4(1.0, 0.0, 0.0, 1.0);
    cols[1] = float4(0.0, 1.0, 0.0, 1.0);
    cols[2] = float4(0.0, 0.0, 1.0, 1.0);

    SetMeshOutputCounts(3, 1);

    // pos
    verts[0].position = float4(t[gid].x, t[gid].y, 0.0, 1.0);
    verts[0].colour = cols[gid];

    // half to next
    uint next = (gid + 1) % 3;
    float2 mid0 = (t[gid] + t[next]) * 0.5;
    verts[1].position = float4(mid0.x, mid0.y, 0.0, 1.0);
    verts[1].colour = cols[gid];

    // half to prev
    uint prev = (gid + 3 - 1) % 3;
    float2 mid1 = (t[gid] + t[prev]) * 0.5;
    verts[2].position = float4(mid1.x, mid1.y, 0.0, 1.0);
    verts[2].colour = cols[gid];

    if(gtid == 0)
        tris[0] = uint3(0, 1, 2);
}