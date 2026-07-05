//
// example vertex buffer instancing
//

struct vs_input_instance {
    float4 row0: TEXCOORD4;
    float4 row1: TEXCOORD5;
    float4 row2: TEXCOORD6;
};

vs_output vs_mesh_vertex_buffer_instanced(vs_input_mesh input, vs_input_instance instance_input) {
    vs_output output;

	float4 pos = float4(input.position.xyz, 1.0);

    float3 transformed;
    transformed.x = dot(instance_input.row0, pos);
    transformed.y = dot(instance_input.row1, pos);
    transformed.z = dot(instance_input.row2, pos);
    pos.xyz = transformed;

    output.position = mul(view_projection_matrix, float4(pos.xyz, 1.0));
    output.world_pos = pos;
    output.texcoord = float4(input.texcoord, 0.0, 0.0);
    output.colour = float4(input.normal.xyz * 0.5 + 0.5, 1.0);
    output.normal = input.normal.xyz;

    return output;
}

//
// example using a structured buffer to lookup instance info from SV_InstanceID
//

StructuredBuffer<row_major float3x4> instance_world_matrices : register(t0);

vs_output vs_mesh_structured_buffer_instanced(vs_input_mesh input, uint iid: SV_InstanceID) {
    vs_output output;

	float4 pos = float4(input.position.xyz, 1.0);
    pos.xyz = mul(instance_world_matrices[iid], pos);

    output.position = mul(view_projection_matrix, pos);
    output.world_pos = pos;
    output.texcoord = float4(input.texcoord, 0.0, 0.0);
    output.colour = float4(input.normal.xyz * 0.5 + 0.5, 1.0);
    output.normal = input.normal.xyz;

    return output;
}