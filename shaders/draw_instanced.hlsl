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
// example using a cbuffer to lookup instance info from SV_InstanceID
//

struct cbuffer_instance_data {
    row_major float3x4 cbuffer_world_matrix[1024];
};

ConstantBuffer<cbuffer_instance_data> cbuffer_instance : register(b1);

vs_output vs_mesh_cbuffer_instanced(vs_input_mesh input, uint iid: SV_InstanceID) {
    vs_output output;

	float4 pos = float4(input.position.xyz, 1.0);
    pos.xyz = mul(cbuffer_instance.cbuffer_world_matrix[iid], pos);

    output.position = mul(view_projection_matrix, pos);
    output.world_pos = pos;
    output.texcoord = float4(input.texcoord, 0.0, 0.0);
    output.colour = float4(input.normal.xyz * 0.5 + 0.5, 1.0);
    output.normal = input.normal.xyz;

    return output;
}