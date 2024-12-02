//
// frustum cull entity aabb's and build indirect commands
//

struct buffer_view {
    uint2 location;
    uint  size_bytes;
    uint  stride_bytes;
};

struct draw_indexed_args {
    uint index_count_per_instance;
    uint instance_count;
    uint start_index_location;
    uint base_vertex_location;
    uint start_instance_location;
};

struct indirect_draw {
    buffer_view         vb;
    buffer_view         ib;
    uint4               ids;
    draw_indexed_args   args;
};

// potential draw calls we want to make
StructuredBuffer<indirect_draw> input_draws[] : register(t0, space11);

// draw calls to populate during the `cs_frustum_cull` dispatch
AppendStructuredBuffer<indirect_draw> output_draws[] : register(u0, space0);

[numthreads(128, 1, 1)]
void cs_frustum_cull(uint did : SV_DispatchThreadID) {
    uint index = did;

    pmfx_touch(resources);

    // grab entity draw data
    extent_data extents = get_extent_data(index);
    camera_data main_camera = get_camera_data();

    // grab potential draw call
    indirect_draw input = input_draws[resources.input1.index][index];

    bool use_aabb = true;
    bool no_cull = false;

    if(no_cull) {
        output_draws[resources.input0.index].Append(input);
    }
    else if(use_aabb) {
        if(aabb_vs_frustum(extents.pos, extents.extent, main_camera.planes)) {
            output_draws[resources.input0.index].Append(input);
        }
    }
    else {
        if(sphere_vs_frustum(extents.pos, length(extents.extent), main_camera.planes)) {
            output_draws[resources.input0.index].Append(input);
        }
    }
}