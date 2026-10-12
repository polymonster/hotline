//
// gpu_dbg: gpu driven debug rendering
// call the gpu_dbg_ functions from any shader to append debug draw commands, the commands are then rendered with
// a single `execute_indirect` of `ms_gpu_dbg` / `ps_gpu_dbg` where each command dispatches 1 mesh group per quad
//

// command types
uint gpu_dbg_cmd_point() { return 0; }
uint gpu_dbg_cmd_line_strip_3d() { return 1; }
uint gpu_dbg_cmd_line_list_3d() { return 2; }
uint gpu_dbg_cmd_line_loop_3d() { return 3; }
uint gpu_dbg_cmd_line_strip_ndc() { return 4; }
uint gpu_dbg_cmd_line_list_ndc() { return 5; }
uint gpu_dbg_cmd_line_loop_ndc() { return 6; }
uint gpu_dbg_cmd_quad_2d() { return 7; }
uint gpu_dbg_cmd_text_2d() { return 8; }
uint gpu_dbg_cmd_text_3d() { return 9; }

// text flags
uint gpu_dbg_text_align_center() { return 1<<0; }
uint gpu_dbg_text_align_left() { return 1<<1; }
uint gpu_dbg_text_align_right() { return 1<<2; }

float gpu_dbg_pi() { return 3.14159265358979; }

struct GpuDbgCounters
{
    uint command_pos; // must be first, it is used as the count buffer for execute_indirect
    uint vertex_pos;
    uint data_pos;
    uint print_data_pos; // not reset each frame, wraps around the print ring
};

struct GpuDbgCommand
{
    uint    type;
    float3  pos;
    float3  size;
    float4  color;
    float4  alt_color;
    uint    flags;
    uint    data_offset;
    uint    data_length;
};

// root constant draw id + DispatchMesh args, consumed by execute_indirect
struct GpuDbgDrawIndirectArgs
{
    uint draw_id;
    uint group_count_x;
    uint group_count_y;
    uint group_count_z;
};

struct GpuDbgVertex
{
    float3 pos;
    float4 color;
};

// Description:
//      Generic vertex output structure for mesh shader to pixel shader for rendering gpu_dbg primitives
struct GpuDbgVertexOutput
{
    float4 position : SV_POSITION;
    float2 uv : TEXCOORD0;
    float4 color : TEXCOORD1;
    float4 alt_color : TEXCOORD2;
    nointerpolation uint type : TEXCOORD3;
};

RWStructuredBuffer<GpuDbgCommand>          gpu_dbg_commands : register(u0);
RWStructuredBuffer<GpuDbgVertex>           gpu_dbg_vertices : register(u1);
RWStructuredBuffer<GpuDbgDrawIndirectArgs> gpu_dbg_draw_indirect_args : register(u2);
RWStructuredBuffer<GpuDbgCounters>         gpu_dbg_counters : register(u3);
RWStructuredBuffer<uint>                    gpu_dbg_data : register(u4); // 1 char per uint
globallycoherent RWStructuredBuffer<uint>   gpu_dbg_print_data : register(u5); // ring of (lap << 8) | char, cpu mapped
Texture2D                                   gpu_dbg_font_atlas : register(t0);
SamplerState                                gpu_dbg_linear_sampler : register(s0);

cbuffer gpu_dbg_view : register(b0)
{
    row_major float4x4 gpu_dbg_view_projection_matrix;
    float2   gpu_dbg_screen_size;
};

// set per command by execute_indirect
cbuffer gpu_dbg_draw : register(b1)
{
    uint gpu_dbg_draw_id;
};

//
// utils
//

// Description:
//      Construct a rotation matrix from axis / angle
// Arguments:
//      axis - the axis to rotate about
//      angle - radian angle to rotate about the axis
float4x4 gpu_dbg_create_matrix_rotation(float3 axis, float angle)
{
    axis = normalize(axis);

    float s = sin(angle);
    float c = cos(angle);
    float oc = 1.0 - c;

    return float4x4(
        oc * axis.x * axis.x + c,           oc * axis.x * axis.y - axis.z * s,  oc * axis.z * axis.x + axis.y * s,  0.0,
        oc * axis.x * axis.y + axis.z * s,  oc * axis.y * axis.y + c,           oc * axis.y * axis.z - axis.x * s,  0.0,
        oc * axis.z * axis.x - axis.y * s,  oc * axis.y * axis.z + axis.x * s,  oc * axis.z * axis.z + c,           0.0,
        0.0,                                0.0,                                0.0,                                1.0
    );
}

// Description:
//      Construct a scale matrix
// Arguments:
//      scale - scale value for x, y and z axis
float4x4 gpu_dbg_create_matrix_scale(float3 scale)
{
    return float4x4(
        scale.x, 0.0, 0.0, 0.0,
        0.0, scale.y, 0.0, 0.0,
        0.0, 0.0, scale.z, 0.0,
        0.0, 0.0, 0.0, 1.0
    );
}

// Description:
//      Construct a translation matrix
// Arguments:
//      translation - translation value for x, y and z axis
float4x4 gpu_dbg_create_matrix_translation(float3 translation)
{
    return float4x4(
        1.0, 0.0, 0.0, translation.x,
        0.0, 1.0, 0.0, translation.y,
        0.0, 0.0, 1.0, translation.z,
        0.0, 0.0, 0.0, 1.0
    );
}

// Description:
//      Construct a perspective projection matrix
// Arguments:
//      fov_y - field of view in radians
//      aspect - aspect ratio
//      near_z - near plane distance
//      far_z - far plane distance
float4x4 gpu_dbg_create_matrix_perspective_projection(float fov_y, float aspect, float near_z, float far_z)
{
    float y_scale = 1.0 / tan(fov_y / 2.0);
    float x_scale = y_scale / aspect;
    float z_range = far_z - near_z;
    float z_scale = -(far_z + near_z) / z_range;
    float wz_scale = -2.0 * far_z * near_z / z_range;

    return float4x4(
        x_scale, 0.0,    0.0,     0.0,
        0.0,    y_scale, 0.0,     0.0,
        0.0,    0.0,    z_scale, -1.0,
        0.0,    0.0,    wz_scale, 0.0
    );
}

// Description:
//      Returns the inverse of a 4x4 matrix
float4x4 gpu_dbg_inverse(float4x4 m)
{
    float a00 = m[0][0], a01 = m[0][1], a02 = m[0][2], a03 = m[0][3];
    float a10 = m[1][0], a11 = m[1][1], a12 = m[1][2], a13 = m[1][3];
    float a20 = m[2][0], a21 = m[2][1], a22 = m[2][2], a23 = m[2][3];
    float a30 = m[3][0], a31 = m[3][1], a32 = m[3][2], a33 = m[3][3];

    float b00 = a00 * a11 - a01 * a10;
    float b01 = a00 * a12 - a02 * a10;
    float b02 = a00 * a13 - a03 * a10;
    float b03 = a01 * a12 - a02 * a11;
    float b04 = a01 * a13 - a03 * a11;
    float b05 = a02 * a13 - a03 * a12;
    float b06 = a20 * a31 - a21 * a30;
    float b07 = a20 * a32 - a22 * a30;
    float b08 = a20 * a33 - a23 * a30;
    float b09 = a21 * a32 - a22 * a31;
    float b10 = a21 * a33 - a23 * a31;
    float b11 = a22 * a33 - a23 * a32;

    float det = b00 * b11 - b01 * b10 + b02 * b09 + b03 * b08 - b04 * b07 + b05 * b06;

    return float4x4(
        a11 * b11 - a12 * b10 + a13 * b09,
        a02 * b10 - a01 * b11 - a03 * b09,
        a31 * b05 - a32 * b04 + a33 * b03,
        a22 * b04 - a21 * b05 - a23 * b03,
        a12 * b08 - a10 * b11 - a13 * b07,
        a00 * b11 - a02 * b08 + a03 * b07,
        a32 * b02 - a30 * b05 - a33 * b01,
        a20 * b05 - a22 * b02 + a23 * b01,
        a10 * b10 - a11 * b08 + a13 * b06,
        a01 * b08 - a00 * b10 - a03 * b06,
        a30 * b04 - a31 * b02 + a33 * b00,
        a21 * b02 - a20 * b04 - a23 * b00,
        a11 * b07 - a10 * b09 - a12 * b06,
        a00 * b09 - a01 * b07 + a02 * b06,
        a31 * b01 - a30 * b03 - a32 * b00,
        a20 * b03 - a21 * b01 + a22 * b00) / det;
}

// Description:
//      Construct an orthonormal basis from vector V using the Hughes Moeller algorithm
// Arguments:
//      v - vector construct basis from (this is the at / forward vector)
//      bt - output bitangent of the basis
//      t - output tangent of the basis
void gpu_dbg_ortho_basis_from_vector(float3 v, out float3 bt, out float3 t)
{
    // choose a vector orthogonal to cv as the direction of b2.
    bt = float3(0.0, -v.z, v.y);
    if(abs(v.x) > abs(v.z))
    {
        bt = float3(-v.y, v.x, 0.0);
    }

    // normalise b2 and construct t
    bt = bt * rsqrt(dot(bt, bt));
    t = cross(bt, v);
}

// Description:
//      Perform chebyshev normalization, projecting onto the unit cube
// Arguments:
//      v - vector to normalize
float3 gpu_dbg_chebyshev_normalize(float3 v)
{
    return (v.xyz / max(max(abs(v.x), abs(v.y)), abs(v.z)));
}

// Description:
//      Extracts the frustum corners groped as 4 near, 4 far where the winding order is the same for near and far
void gpu_dbg_frustum_corners_from_matrix(float4x4 mat, float near_t, float far_t, out float3 corners[8])
{
    float4x4 inv = gpu_dbg_inverse(mat);

    float4 ndc[] = {
        float4(-1.0, -1.0, 0.0, 1.0),
        float4( 1.0, -1.0, 0.0, 1.0),
        float4( 1.0,  1.0, 0.0, 1.0),
        float4(-1.0,  1.0, 0.0, 1.0),
        float4(-1.0, -1.0, 1.0, 1.0),
        float4( 1.0, -1.0, 1.0, 1.0),
        float4( 1.0,  1.0, 1.0, 1.0),
        float4(-1.0,  1.0, 1.0, 1.0),
    };

    // unproject corners
    for(int i = 0; i < 8; ++i)
    {
        float4 unproj = mul(ndc[i], inv);
        corners[i] = unproj.xyz / unproj.w;
    }

    if(near_t != 0.0 || far_t != 1.0)
    {
        // linearly interpolate with near and far to get a slice of frustum
        for(int j = 0; j < 4; ++j)
        {
            float3 v = corners[j + 4] - corners[j];
            corners[j] = corners[j] + v * near_t;
            corners[j + 4] = corners[j] + v * far_t;
        }
    }
}

//
// text utils
//

// Description:
//      Count the number of chars in 'val' and return the result, includes a '-' sign for negative numbers
int gpu_dbg_count_int_chars(int val)
{
    int minus = 0;

    if(val == 0)
    {
        return 1;
    }
    else if(val < 0)
    {
        minus = 1;
    }

    return ((int)(floor(log10(abs((float)val))))) + 1 + minus;
}

// Description:
//      Extract the frac (decimal) part of a float and return the digits as an integer
int gpu_dbg_frac_to_int(float val, out int len, out int zeros)
{
    zeros = 0;
    len = 0;

    if(val == 0.0)
    {
        len = 1;
        return 0;
    }

    float f = abs(val) - (int)(abs(val));
    for(int i = 0; i < 8; ++i)
    {
        // if we set a digit, then truncate some of the trailing precision
        if(floor(f) > 0.0 || floor(val) > 0.0)
        {
            if((i > 0 && frac(f) < 0.001) || f < 0.0001)
            {
                if(i == 0)
                {
                    f = 0;
                    len = 1;
                }

                break;
            }
        }

        f *= 10.0;
        len++;

        if((int)f == 0 && (int)val == 0)
        {
            zeros++;
        }
    }

    return (int)f;
}

// Description:
//      Count the number of chars in 'val' and return the result, includes a '-' sign for negative numbers and '.' for decimal place
int gpu_dbg_count_float_chars(float val)
{
    if(val == 0)
    {
        return 3;
    }

    int ipart = gpu_dbg_count_int_chars((int)val);
    int fpart = 1; // '.'

    int flen = 0;
    int fzeros = 0;
    gpu_dbg_frac_to_int(val, flen, fzeros);
    fpart += flen;

    return ipart + fpart;
}

// Description:
//      Targets for formatted chars, the text data buffer for gpu_dbg_text or the print ring for gpu_dbg_printf
uint gpu_dbg_target_text() { return 0; }
uint gpu_dbg_target_print() { return 1; }

// Description:
//      Number of chars in the print ring, must match PRINT_RING_SIZE in examples/gpu_dbg/main.rs which reads it back
uint gpu_dbg_print_ring_size() { return 1 << 16; }

// Description:
//      Write char 'c' to position 'pos' in 'target'. print chars are tagged with the lap of the ring they were written
//      in so the cpu can tell new chars from stale ones without syncing on the counter
void gpu_dbg_put_char(uint target, uint pos, uint c)
{
    if(target == gpu_dbg_target_text())
    {
        gpu_dbg_data[pos] = c;
    }
    else
    {
        uint size = gpu_dbg_print_ring_size();
        gpu_dbg_print_data[pos % size] = (((pos / size) + 1) << 8) | c;
    }
}

// Description:
//      Write the digits of 'val' into 'target' at 'cp' and return the new write position
int gpu_dbg_itoc_to_data(int val, int cp, int leading_zeros, uint target)
{
    // count chars
    int char_count = gpu_dbg_count_int_chars(val);

    // add minus sign
    if(val < 0)
    {
        gpu_dbg_put_char(target, cp++, '-');
        char_count--;
    }

    // digit lookup
    static const int lut_digit[] = {
        1,
        10,
        100,
        1000,
        10000,
        100000,
        1000000,
        10000000,
        100000000,
        1000000000
    };

    // divide and iterate
    int numer = abs(val);
    int digit_iter = char_count-1;
    while(digit_iter >= 0)
    {
        if(leading_zeros > 0)
        {
            gpu_dbg_put_char(target, cp++, '0');
            leading_zeros--;
            continue;
        }

        int denom = lut_digit[digit_iter];
        int digit = numer / denom;

        gpu_dbg_put_char(target, cp++, '0' + digit);

        numer -= (digit * denom);
        digit_iter--;
    }

    return cp;
}

int gpu_dbg_ftoc_to_data(float val, int cp, uint target)
{
    // int part
    cp = gpu_dbg_itoc_to_data((int)val, cp, 0, target);

    // decimal place
    gpu_dbg_put_char(target, cp++, '.');

    // frac part
    int flen;
    int fzeros;
    int fint = gpu_dbg_frac_to_int(val, flen, fzeros);
    cp = gpu_dbg_itoc_to_data(fint, cp, fzeros, target);

    return cp;
}

//
// api
//

// Description:
//      Allocate a command and its indirect args, `groups` is the number of mesh shader groups (quads) to dispatch
void gpu_dbg_push_command(uint type, float3 pos, float3 size, float4 color, float4 alt_color, uint flags, uint data_offset, uint data_length, uint groups)
{
    uint p = 0;
    InterlockedAdd(gpu_dbg_counters[0].command_pos, 1, p);

    GpuDbgCommand cmd;
    cmd.type = type;
    cmd.pos = pos;
    cmd.size = size;
    cmd.color = color;
    cmd.alt_color = alt_color;
    cmd.flags = flags;
    cmd.data_offset = data_offset;
    cmd.data_length = data_length;
    gpu_dbg_commands[p] = cmd;

    GpuDbgDrawIndirectArgs args;
    args.draw_id = p;
    args.group_count_x = groups;
    args.group_count_y = 1;
    args.group_count_z = 1;
    gpu_dbg_draw_indirect_args[p] = args;
}

void gpu_dbg_set_vertex(uint index, float3 pos, float4 color)
{
    GpuDbgVertex v;
    v.pos = pos;
    v.color = color;
    gpu_dbg_vertices[index] = v;
}

// Description:
//      Append a draw call for a point drawn as a quad projected at the 3D point 'pos', with screen space size and colour
// Arguments:
//      pos - 3D position to project the point from
//      size - 2D screen size
//      color - RGBA color in 0-1 range
void gpu_dbg_point_3d(float3 pos, float2 size, float4 color)
{
    gpu_dbg_push_command(gpu_dbg_cmd_point(), pos, float3(size, 1.0), color, (float4)0.0, 0, 0, 0, 1);
}

// Description:
//      Append a screen space quad with 'pos' and 'size' in pixels
void gpu_dbg_quad_2d(float2 pos, float2 size, float4 color)
{
    gpu_dbg_push_command(gpu_dbg_cmd_quad_2d(), float3(pos, 0.0), float3(size, 1.0), color, (float4)0.0, 0, 0, 0, 1);
}

// Description:
//      Returns the number of chars 'text' formats to, %i and %f format specifiers take values from 'vars' in order
//      char literals are not supported in dxc templates, so '%' = 37, 'i' = 105, 'f' = 102
template<uint N>
uint gpu_dbg_format_length(uint text[N], float4 vars)
{
    uint var_index = 0;
    uint char_count = 0;
    for(uint i = 0; i < N; ++i)
    {
        if(text[i] == 37 && i + 1 < N)
        {
            if(text[i + 1] == 105)
            {
                char_count += gpu_dbg_count_int_chars((int)vars[var_index]);
            }
            else if(text[i + 1] == 102)
            {
                char_count += gpu_dbg_count_float_chars(vars[var_index]);
            }

            var_index++;
            i += 1;
            continue;
        }

        char_count++;
    }

    return char_count;
}

// Description:
//      Write the formatted chars of 'text' into 'target' starting at 'cp', returns the next write position
template<uint N>
int gpu_dbg_format_write(uint text[N], float4 vars, uint target, int cp)
{
    uint var_index = 0;
    for(uint i = 0; i < N; ++i)
    {
        if(text[i] == 37 && i + 1 < N)
        {
            if(text[i + 1] == 105)
            {
                cp = gpu_dbg_itoc_to_data((int)vars[var_index], cp, 0, target);
            }
            else if(text[i + 1] == 102)
            {
                cp = gpu_dbg_ftoc_to_data(vars[var_index], cp, target);
            }

            var_index++;
            i += 1;
            continue;
        }

        gpu_dbg_put_char(target, cp++, text[i]);
    }

    return cp;
}

// Description:
//      Append text at the 3D point 'pos' with screen space char 'size'. 'text' is an array of char literals, %i and %f
//      format specifiers take values from 'vars' in order (up to 4). 'flags' controls alignment (gpu_dbg_k_text_align*)
//      and 'outline_color' draws an outline around the glyphs
//      uint label[] = { 'x', ' ', '=', ' ', '%', 'f' };
//      gpu_dbg_textf(label, pos, 50.0, color, float4(x, 0.0, 0.0, 0.0));
template<uint N>
void gpu_dbg_textf_ex(uint text[N], float3 pos, float size, float4 color, float4 vars, uint flags, float4 outline_color)
{
    uint char_count = gpu_dbg_format_length(text, vars);

    uint dp = 0;
    InterlockedAdd(gpu_dbg_counters[0].data_pos, char_count, dp);
    gpu_dbg_format_write(text, vars, gpu_dbg_target_text(), dp);

    // push the draw, 1 quad per char
    gpu_dbg_push_command(gpu_dbg_cmd_text_3d(), pos, float3(size, size, 1.0), color, outline_color, flags, dp, char_count, char_count);
}

// Description:
//      Print a line to the cpu console (stdout), with the same formatting as gpu_dbg_textf. the cpu reads the print ring
//      persistently mapped, so lines arrive without waiting on the gpu
//      uint msg[] = { 'v', 'a', 'l', ' ', '%', 'i' };
//      gpu_dbg_printf(msg, float4(val, 0.0, 0.0, 0.0));
template<uint N>
void gpu_dbg_printf(uint text[N], float4 vars)
{
    uint char_count = gpu_dbg_format_length(text, vars);

    uint pp = 0;
    InterlockedAdd(gpu_dbg_counters[0].print_data_pos, char_count + 1, pp);
    int cp = gpu_dbg_format_write(text, vars, gpu_dbg_target_print(), pp);

    // terminate the line, '\n' = 10
    gpu_dbg_put_char(gpu_dbg_target_print(), cp, 10);
}

template<uint N>
void gpu_dbg_print(uint text[N])
{
    gpu_dbg_printf(text, (float4)0.0);
}

template<uint N>
void gpu_dbg_textf(uint text[N], float3 pos, float size, float4 color, float4 vars)
{
    gpu_dbg_textf_ex(text, pos, size, color, vars, 0, (float4)0.0);
}

template<uint N>
void gpu_dbg_text_ex(uint text[N], float3 pos, float size, float4 color, uint flags, float4 outline_color)
{
    gpu_dbg_textf_ex(text, pos, size, color, (float4)0.0, flags, outline_color);
}

template<uint N>
void gpu_dbg_text(uint text[N], float3 pos, float size, float4 color)
{
    gpu_dbg_textf_ex(text, pos, size, color, (float4)0.0, 0, (float4)0.0);
}

// Description:
//      Allocate 'vertex_count' vertices for a line primitive of type 'prim' and return the offset to write them to
uint gpu_dbg_lines(uint prim, uint vertex_count, float thickness)
{
    uint vp = 0;
    InterlockedAdd(gpu_dbg_counters[0].vertex_pos, vertex_count, vp);

    // 1 quad per line segment
    uint groups = vertex_count - 1;
    if(prim == gpu_dbg_cmd_line_list_3d() || prim == gpu_dbg_cmd_line_list_ndc())
    {
        groups = vertex_count / 2;
    }
    else if(prim == gpu_dbg_cmd_line_loop_3d() || prim == gpu_dbg_cmd_line_loop_ndc())
    {
        groups = vertex_count;
    }

    gpu_dbg_push_command(prim, (float3)0.0, float3(thickness, thickness, 1.0), (float4)0.0, (float4)0.0, 0, vp, vertex_count, groups);
    return vp;
}

void gpu_dbg_line(float3 start, float3 end, float4 color, float thickness)
{
    uint vp = gpu_dbg_lines(gpu_dbg_cmd_line_strip_3d(), 2, thickness);
    gpu_dbg_set_vertex(vp, start, color);
    gpu_dbg_set_vertex(vp + 1, end, color);
}

void gpu_dbg_triangle(float3 p0, float3 p1, float3 p2, float4 color, float thickness)
{
    uint vp = gpu_dbg_lines(gpu_dbg_cmd_line_loop_3d(), 3, thickness);
    gpu_dbg_set_vertex(vp, p0, color);
    gpu_dbg_set_vertex(vp + 1, p1, color);
    gpu_dbg_set_vertex(vp + 2, p2, color);
}

void gpu_dbg_disc(float3 pos, float3 radius, float3 axis, float4 color, float thickness, uint segments)
{
    axis = normalize(axis);

    // ortho basis for axis
    float3 right, up;
    gpu_dbg_ortho_basis_from_vector(axis, right, up);

    // make a circle in the plane where axis is the normal
    float angle = -gpu_dbg_pi();
    float angle_step = (gpu_dbg_pi() * 2.0) / (segments-1);

    uint vp = gpu_dbg_lines(gpu_dbg_cmd_line_strip_3d(), segments, thickness);

    [loop]
    for(uint i = 0; i < segments; ++i)
    {
        float3 v1 = right * cos(angle) - up * sin(angle);
        gpu_dbg_set_vertex(vp + i, pos + v1 * radius, color);
        angle += angle_step;
    }
}

void gpu_dbg_sphere(float3 pos, float radius, float4 color, float thickness, uint segments)
{
    // alloc space
    uint vp = gpu_dbg_lines(gpu_dbg_cmd_line_strip_3d(), segments * segments, thickness);

    float angle_step = (gpu_dbg_pi() * 2.0) / (segments-1);
    float hangle = -gpu_dbg_pi();

    // series of discs
    [loop]
    for(uint j = 0; j < segments; ++j)
    {
        float3 right, up;

        // vertical discs
        float3 vaxis = float3(cos(hangle), -sin(hangle), 0.0);
        gpu_dbg_ortho_basis_from_vector(vaxis, right, up);

        float angle = -gpu_dbg_pi();
        uint slice_offset = j * segments;

        [loop]
        for(uint i = 0; i < segments; ++i)
        {
            float3 v1 = right * cos(angle) - up * sin(angle);
            gpu_dbg_set_vertex(vp + i + slice_offset, pos + v1 * radius, color);
            angle += angle_step;
        }

        hangle += angle_step;
    }
}

void gpu_dbg_hemisphere(float3 pos, float3 axis, float radius, float4 color, float thickness, uint segments)
{
    // ortho basis for axis
    float3 right, up;
    gpu_dbg_ortho_basis_from_vector(axis, right, up);

    // alloc space
    uint quad_segments = max(segments / 4, 4);
    uint vp = gpu_dbg_lines(gpu_dbg_cmd_line_list_3d(), segments * 2 + (segments * quad_segments * 2), thickness);

    // make a circle in the plane where axis is the normal
    float angle = -gpu_dbg_pi();
    float angle_step = (gpu_dbg_pi() * 2.0) / (segments-1);
    float qangle_step = (gpu_dbg_pi() * 0.5) / quad_segments;

    [loop]
    for(uint i = 0; i < segments; ++i)
    {
        float3 v1 = right * cos(angle) - up * sin(angle);
        float3 v2 = right * cos(angle + angle_step) - up * sin(angle + angle_step);

        uint v = vp + (i * 2);

        // this forms the base
        gpu_dbg_set_vertex(v, pos + v1 * radius, color);
        gpu_dbg_set_vertex(v + 1, pos + v2 * radius, color);

        // add a line quadant to form the hemisphere
        float qangle = 0.0;

        for(uint j = 0; j < quad_segments; ++j)
        {
            float3 qv1 = v1 * cos(qangle) - axis * sin(qangle);
            float3 qv2 = v1 * cos(qangle - qangle_step) - axis * sin(qangle - qangle_step);

            v = vp + (segments * 2) + (j * 2) + (i * quad_segments * 2);

            gpu_dbg_set_vertex(v, pos + qv1 * radius, color);
            gpu_dbg_set_vertex(v + 1, pos + qv2 * radius, color);

            qangle -= qangle_step;
        }

        angle += angle_step;
    }
}

// Description:
//      Write 12 box edges as a line list from 8 corners ordered as 4 bottom, 4 top with the same winding
void gpu_dbg_box_edges(float3 corners[8], float4 color, float thickness)
{
    uint vp = gpu_dbg_lines(gpu_dbg_cmd_line_list_3d(), 24, thickness);

    for(uint i = 0; i < 4; ++i)
    {
        uint n = (i + 1) % 4;

        // bottom
        gpu_dbg_set_vertex(vp++, corners[i], color);
        gpu_dbg_set_vertex(vp++, corners[n], color);

        // top
        gpu_dbg_set_vertex(vp++, corners[i + 4], color);
        gpu_dbg_set_vertex(vp++, corners[n + 4], color);

        // sides
        gpu_dbg_set_vertex(vp++, corners[i], color);
        gpu_dbg_set_vertex(vp++, corners[i + 4], color);
    }
}

void gpu_dbg_aabb(float3 aabbmin, float3 aabbmax, float4 color, float thickness)
{
    float3 corners[8] = {
        float3(aabbmin.x, aabbmin.y, aabbmin.z),
        float3(aabbmax.x, aabbmin.y, aabbmin.z),
        float3(aabbmax.x, aabbmin.y, aabbmax.z),
        float3(aabbmin.x, aabbmin.y, aabbmax.z),
        float3(aabbmin.x, aabbmax.y, aabbmin.z),
        float3(aabbmax.x, aabbmax.y, aabbmin.z),
        float3(aabbmax.x, aabbmax.y, aabbmax.z),
        float3(aabbmin.x, aabbmax.y, aabbmax.z)
    };

    gpu_dbg_box_edges(corners, color, thickness);
}

void gpu_dbg_obb(float4x4 world_matrix, float4 color, float thickness)
{
    // unit aabb corners
    float3 corners[8] = {
        float3(-1.0, -1.0, -1.0),
        float3( 1.0, -1.0, -1.0),
        float3( 1.0,  1.0, -1.0),
        float3(-1.0,  1.0, -1.0),
        float3(-1.0, -1.0, 1.0),
        float3( 1.0, -1.0, 1.0),
        float3( 1.0,  1.0, 1.0),
        float3(-1.0,  1.0, 1.0),
    };

    // transform corners
    for(uint i = 0; i < 8; ++i)
    {
        corners[i] = mul(world_matrix, float4(corners[i], 1.0)).xyz;
    }

    gpu_dbg_box_edges(corners, color, thickness);
}

void gpu_dbg_grid(float3 center, float3 axis, float2 dimension, float2 divisions, float4 color, float thickness)
{
    float3 right, up;
    gpu_dbg_ortho_basis_from_vector(axis, right, up);

    float3 corner = center - (right * (float)(dimension.x / 2.0)) - (up * (float)(dimension.y / 2.0));
    uint vp = gpu_dbg_lines(gpu_dbg_cmd_line_list_3d(), (uint)(divisions.x + 1 + divisions.y + 1) * 2, thickness);

    float2 cell_step = dimension / divisions;

    float3 pos = corner;
    for(int i = 0; i < (int)(divisions.x + 1); ++i)
    {
        gpu_dbg_set_vertex(vp++, pos, color);
        gpu_dbg_set_vertex(vp++, pos + dimension.x * right, color);
        pos += up * cell_step.y;
    }

    pos = corner;
    for(int j = 0; j < (int)(divisions.y + 1); ++j)
    {
        gpu_dbg_set_vertex(vp++, pos, color);
        gpu_dbg_set_vertex(vp++, pos + dimension.y * up, color);
        pos += right * cell_step.x;
    }
}

void gpu_dbg_cone(float3 apex, float3 axis, float cutoff, float height, float4 color, float thickness, uint sides)
{
    // axis must be normalized
    axis = normalize(axis);

    // ortho basis of cone from axis
    float3 bt, t;
    gpu_dbg_ortho_basis_from_vector(axis, bt, t);

    // rotate a vector to the edge of the cos cutoff
    float x = cutoff;
    float y = sqrt(1.0 - pow(cutoff, 2.0));
    float3 v1 = t * x + axis * y;

    // flatten the vector on to the base of the code and take the difference to the axis to get the unit radius
    float rad = length(gpu_dbg_chebyshev_normalize(v1) - axis);
    float len = length(gpu_dbg_chebyshev_normalize(v1));

    // allocate space for lines
    uint basevp = gpu_dbg_lines(gpu_dbg_cmd_line_loop_3d(), sides, thickness);
    uint corevp = gpu_dbg_lines(gpu_dbg_cmd_line_list_3d(), sides * 2 + 2, thickness);

    // add a line for the axis
    gpu_dbg_set_vertex(corevp + 0, apex, color);
    gpu_dbg_set_vertex(corevp + 1, apex + axis * height, color);

    // offset to put vertices for the sides
    uint sidesvp = corevp + 2;

    // iterate around the base adding lines
    float angle = -gpu_dbg_pi();
    float angle_step = (gpu_dbg_pi() * 2.0) / (float)sides;

    [loop]
    for(uint i = 0; i < sides; ++i)
    {
        float xr = cos(angle);
        float yr = -sin(angle);

        float3 ve = (apex + axis) + normalize(t * xr + bt * yr) * rad;
        float3 edge = apex + (normalize(ve - apex)) * height * len;

        // side
        gpu_dbg_set_vertex(sidesvp + i * 2, apex, color);
        gpu_dbg_set_vertex(sidesvp + i * 2 + 1, edge, color);

        // base
        gpu_dbg_set_vertex(basevp + i, edge, color);

        angle += angle_step;
    }
}

// Description:
//      Cylinder with optional hemisphere caps (capsule), 'cap_segments' is the number of segments in each cap quadrant or 0 for a cylinder
void gpu_dbg_cylinder_internal(float3 pos, float3 axis, float3 radius, float height, float4 color, float thickness, uint sides, uint cap_segments)
{
    // axis must be normalized
    axis = normalize(axis);

    // ortho basis for axis
    float3 right, up;
    gpu_dbg_ortho_basis_from_vector(axis, right, up);

    // make a circle in the plane where axis is the normal
    float angle = -gpu_dbg_pi();
    float angle_step = (gpu_dbg_pi() * 2.0) / (sides-1);
    float qangle_step = (gpu_dbg_pi() * 0.5) / max(cap_segments, 1);

    // add a line per side for the top and bottom circles and the joining sides, plus the caps
    uint vp = gpu_dbg_lines(gpu_dbg_cmd_line_list_3d(), sides * 6 + (cap_segments * sides * 4), thickness);

    // calc top and bottom pos of cylinder
    float3 bottom_pos = pos - axis * height * 0.5;
    float3 top_pos = pos + axis * height * 0.5;

    [loop]
    for(uint i = 0; i < sides; ++i)
    {
        float3 v1 = right * cos(angle) - up * sin(angle);
        float3 v2 = right * cos(angle + angle_step) - up * sin(angle + angle_step);

        uint v = vp + (i * 6);

        // bottom
        gpu_dbg_set_vertex(v, bottom_pos + v1 * radius, color);
        gpu_dbg_set_vertex(v + 1, bottom_pos + v2 * radius, color);

        // top
        gpu_dbg_set_vertex(v + 2, top_pos + v1 * radius, color);
        gpu_dbg_set_vertex(v + 3, top_pos + v2 * radius, color);

        // join sides
        gpu_dbg_set_vertex(v + 4, bottom_pos + v1 * radius, color);
        gpu_dbg_set_vertex(v + 5, top_pos + v1 * radius, color);

        // add a line quadrant to form the hemispheres at the top and bottom
        float qangle = 0.0;
        for(uint j = 0; j < cap_segments; ++j)
        {
            float3 qv1 = v1 * cos(qangle) - axis * sin(qangle);
            float3 qv2 = v1 * cos(qangle - qangle_step) - axis * sin(qangle - qangle_step);

            v = vp + (sides * 6) + (j * 2) + (i * cap_segments * 2);
            gpu_dbg_set_vertex(v, top_pos + qv1 * radius, color);
            gpu_dbg_set_vertex(v + 1, top_pos + qv2 * radius, color);

            // mirror for the bottom
            v += cap_segments * 2 * sides;
            gpu_dbg_set_vertex(v, bottom_pos + (v1 * cos(qangle) + axis * sin(qangle)) * radius, color);
            gpu_dbg_set_vertex(v + 1, bottom_pos + (v1 * cos(qangle - qangle_step) + axis * sin(qangle - qangle_step)) * radius, color);

            qangle -= qangle_step;
        }

        angle += angle_step;
    }
}

void gpu_dbg_cylinder(float3 pos, float3 axis, float3 radius, float height, float4 color, float thickness, uint sides)
{
    gpu_dbg_cylinder_internal(pos, axis, radius, height, color, thickness, sides, 0);
}

void gpu_dbg_capsule(float3 pos, float3 axis, float3 radius, float height, float4 color, float thickness, uint sides)
{
    gpu_dbg_cylinder_internal(pos, axis, radius, height, color, thickness, sides, max(sides / 4, 4));
}

void gpu_dbg_frustum(float4x4 view_projection_matrix, float near_t, float far_t, float4 color, float thickness)
{
    float3 corners[8];
    gpu_dbg_frustum_corners_from_matrix(view_projection_matrix, near_t, far_t, corners);
    gpu_dbg_box_edges(corners, color, thickness);
}

void gpu_dbg_demo()
{
    float section_size = 350.0f;
    float label_size = 55.0f;
    float3 label_offset = float3(0.0, 0.0, 150.0);
    float4 white = float4(1.0, 1.0, 1.0, 1.0);
    float types = 16;
    float3 pos = float3(0.0, 0.0, 0.0);
    uint2 type_loc = uint2(0, 0);
    uint type_pos = 0;
    float pi = gpu_dbg_pi();

    // colors
    float4 colors[] = {
        float4(0.3, 0.3, 0.3, 1.0), // Dark Gray
        float4(0.8, 0.2, 0.2, 1.0), // Soft Red
        float4(0.2, 0.8, 0.2, 1.0), // Soft Green
        float4(0.2, 0.2, 0.8, 1.0), // Soft Blue
        float4(0.8, 0.8, 0.2, 1.0), // Soft Yellow
        float4(0.8, 0.2, 0.8, 1.0), // Soft Magenta
        float4(0.2, 0.8, 0.8, 1.0), // Soft Cyan
        float4(0.6, 0.2, 0.2, 1.0), // Muted Red
        float4(0.2, 0.6, 0.2, 1.0), // Muted Green
        float4(0.2, 0.2, 0.6, 1.0), // Muted Blue
        float4(0.6, 0.6, 0.2, 1.0), // Muted Yellow
        float4(0.6, 0.2, 0.6, 1.0), // Muted Magenta
        float4(0.2, 0.6, 0.6, 1.0), // Muted Cyan
        float4(0.7, 0.7, 0.7, 1.0), // Light Gray
        float4(0.9, 0.5, 0.2, 1.0), // Soft Orange
        float4(0.6, 0.4, 0.2, 1.0)  // Soft Brown
    };

    // calculate a square grid to fit the number of primitive types
    uint irc = (uint)ceil(sqrt(types));

    // grid
    float3 grid_offset = float3(irc * section_size, irc * section_size, 0.0) * 0.5;
    gpu_dbg_grid(float3(0.0, 0.0, -250.0), float3(0.0, 0.0, 1.0), grid_offset.xy * 2.0, float2(10.0, 10.0), float4(1.0, 1.0, 1.0, 1.0), 1.0);

    // offset primitives to the centre of the section
    grid_offset.xy -= section_size * 0.5;

    // point
    type_loc = uint2(type_pos / irc, type_pos % irc);
    pos = float3(type_loc.xy * section_size, 0.0) - grid_offset;
    type_pos++;

    gpu_dbg_point_3d(pos, float2(50.0, 50.0), colors[type_pos]);

    uint label_point[] = { 'P', 'o', 'i', 'n', 't' };
    gpu_dbg_text(label_point, pos - label_offset, label_size, white);

    // line
    type_loc = uint2(type_pos / irc, type_pos % irc);
    pos = float3(type_loc.xy * section_size, 0.0) - grid_offset;
    type_pos++;

    gpu_dbg_line(pos - (float3)100.0, pos + (float3)100.0, colors[type_pos], 3.0);

    uint label_line[] = { 'L', 'i', 'n', 'e' };
    gpu_dbg_text(label_line, pos - label_offset, label_size, white);

    // triangle
    type_loc = uint2(type_pos / irc, type_pos % irc);
    pos = float3(type_loc.xy * section_size, 0.0) - grid_offset;
    type_pos++;

    gpu_dbg_triangle(
        pos - float3(100.0, 100.0, 100.0),
        pos + float3(100.0, -100.0, 100.0),
        pos + float3(100.0, 100.0, -100.0),
        colors[type_pos],
        1.0
    );

    uint label_triangle[] = { 'T', 'r', 'i', 'a', 'n', 'g', 'l', 'e' };
    gpu_dbg_text(label_triangle, pos - label_offset, label_size, white);

    // disc
    type_loc = uint2(type_pos / irc, type_pos % irc);
    pos = float3(type_loc.xy * section_size, 0.0) - grid_offset;
    type_pos++;

    gpu_dbg_disc(pos, float3(100.0, 100.0, 100.0), float3(-1.0, 1.0, 0.0), colors[type_pos], 1.0, 16);

    uint label_disc[] = { 'D', 'i', 's', 'c' };
    gpu_dbg_text(label_disc, pos - label_offset, label_size, white);

    // sphere
    type_loc = uint2(type_pos / irc, type_pos % irc);
    pos = float3(type_loc.xy * section_size, 0.0) - grid_offset;
    type_pos++;

    gpu_dbg_sphere(pos, 100.0, colors[type_pos], 1.0, 16);

    uint label_sphere[] = { 'S', 'p', 'h', 'e', 'r', 'e' };
    gpu_dbg_text(label_sphere, pos - label_offset, label_size, white);

    // hemi-sphere
    type_loc = uint2(type_pos / irc, type_pos % irc);
    pos = float3(type_loc.xy * section_size, 0.0) - grid_offset;
    type_pos++;

    gpu_dbg_hemisphere(pos, float3(0.0, 0.0, 1.0), 100.0, colors[type_pos], 1.0, 16);

    uint label_hemisphere[] = { 'H', 'e', 'm', 'i', 's', 'p', 'h', 'e', 'r', 'e' };
    gpu_dbg_text(label_hemisphere, pos - label_offset, label_size, white);

    // aabb
    type_loc = uint2(type_pos / irc, type_pos % irc);
    pos = float3(type_loc.xy * section_size, 0.0) - grid_offset;
    type_pos++;

    gpu_dbg_aabb(pos - float3(100.0, 100.0, 100.0), pos + float3(100.0, 100.0, 100.0), colors[type_pos], 1.0);

    uint label_aabb[] = { 'A', 'A', 'B', 'B' };
    gpu_dbg_text(label_aabb, pos - label_offset, label_size, white);

    // obb
    type_loc = uint2(type_pos / irc, type_pos % irc);
    pos = float3(type_loc.xy * section_size, 0.0) - grid_offset;
    type_pos++;

    float4x4 oob_rot = gpu_dbg_create_matrix_rotation(float3(1.0, 1.0, 1.0), pi * 0.25);
    float4x4 obb_scale = gpu_dbg_create_matrix_scale(float3(100.0, 100.0, 50.0));
    float4x4 obb_translate = gpu_dbg_create_matrix_translation(pos);
    float4x4 obb_mat = mul(mul(obb_translate, oob_rot), obb_scale);

    gpu_dbg_obb(obb_mat, colors[type_pos], 1.0);

    uint label_obb[] = { 'O', 'B', 'B' };
    gpu_dbg_text(label_obb, pos - label_offset, label_size, white);

    // cone
    type_loc = uint2(type_pos / irc, type_pos % irc);
    pos = float3(type_loc.xy * section_size, 0.0) - grid_offset;
    type_pos++;

    float3 cone_axis = normalize(float3(1.0, 1.0, 1.0));
    gpu_dbg_cone(pos - cone_axis * 50.0, cone_axis, 0.8, 100.0, colors[type_pos], 1.0, 16);

    uint label_cone[] = { 'C', 'o', 'n', 'e' };
    gpu_dbg_text(label_cone, pos - label_offset, label_size, white);

    // frustum
    type_loc = uint2(type_pos / irc, type_pos % irc);
    pos = float3(type_loc.xy * section_size, 0.0) - grid_offset;
    type_pos++;

    float4x4 proj_mat = gpu_dbg_create_matrix_perspective_projection(1.04, 1.0, 0.1, 10000.0);

    float4x4 view_translate = gpu_dbg_create_matrix_translation(pos - normalize(float3(1.0, 0.0, 1.0)) * 100.0);
    float4x4 view_rotate = gpu_dbg_create_matrix_rotation(float3(0.0, 1.0, 0.0), -pi * 0.75);

    float4x4 view_mat = transpose(gpu_dbg_inverse(mul(view_translate, view_rotate)));
    float4x4 view_proj_mat = mul(view_mat, proj_mat);

    gpu_dbg_frustum(view_proj_mat, 0.001, 0.01, colors[type_pos], 1.0);

    uint label_frustum[] = { 'F', 'r', 'u', 's', 't', 'u', 'm' };
    gpu_dbg_text(label_frustum, pos - label_offset, label_size, white);

    // cylinder
    type_loc = uint2(type_pos / irc, type_pos % irc);
    pos = float3(type_loc.xy * section_size, 0.0) - grid_offset;
    type_pos++;

    gpu_dbg_cylinder(pos, float3(1.0, 1.0, 1.0), 50.0, 100.0, colors[type_pos], 1.0, 16);

    uint label_cylinder[] = { 'C', 'y', 'l', 'i', 'n', 'd', 'e', 'r' };
    gpu_dbg_text(label_cylinder, pos - label_offset, label_size, white);

    // capsule
    type_loc = uint2(type_pos / irc, type_pos % irc);
    pos = float3(type_loc.xy * section_size, 0.0) - grid_offset;
    type_pos++;

    gpu_dbg_capsule(pos, float3(1.0, 1.0, 1.0), 75.0, 100.0, colors[type_pos], 1.0, 16);

    uint label_capsule[] = { 'C', 'a', 'p', 's', 'u', 'l', 'e' };
    gpu_dbg_text(label_capsule, pos - label_offset, label_size, white);

    // line list
    type_loc = uint2(type_pos / irc, type_pos % irc);
    pos = float3(type_loc.xy * section_size, 0.0) - grid_offset;
    type_pos++;

    {
        // reserve space for a line list with segments
        int line_segments = 32;
        uint lv = gpu_dbg_lines(gpu_dbg_cmd_line_list_3d(), line_segments * 2, 5.0);

        // sample cosine
        float list_length = 100.0;
        float line_length = 20.0;
        float xstep = list_length / (float)line_segments;
        float tstep = 8.0 * pi / (float)line_segments;

        float lx = 0.0;
        float lt = 0.0;

        for(int i = 0; i < line_segments; ++i)
        {
            // alternate vertex colour
            float4 col = i % 2 == 0 ? float4(1.0, 0.5, 0.0, 1.0) : float4(0.0, 0.5, 1.0, 1.0);

            // write the vertex pos and color
            float3 mid = pos + float3(lx, lx, cos(lt) * 25.0) - float3(list_length, list_length, 0.0) * 0.5;
            float3 perp = float3(0.0, 0.0, 1.0);

            gpu_dbg_set_vertex(lv++, mid + perp * line_length, col);
            gpu_dbg_set_vertex(lv++, mid - perp * line_length, col);

            lx += xstep;
            lt += tstep;
        }
    }

    uint label_line_list[] = { 'L', 'i', 'n', 'e', ' ', 'L', 'i', 's', 't' };
    gpu_dbg_text(label_line_list, pos - label_offset, label_size, white);

    // line strip
    type_loc = uint2(type_pos / irc, type_pos % irc);
    pos = float3(type_loc.xy * section_size, 0.0) - grid_offset;
    type_pos++;

    {
        // reserve space for a line strip with segments
        int line_strip_segments = 32;
        uint lv = gpu_dbg_lines(gpu_dbg_cmd_line_strip_3d(), line_strip_segments, 5.0);

        // sample cosine
        float strip_length = 100.0;
        float xstep = strip_length / (float)line_strip_segments;
        float tstep = 8.0 * pi / (float)line_strip_segments;

        float lx = 0.0;
        float lt = 0.0;

        for(int i = 0; i < line_strip_segments; ++i)
        {
            // alternate vertex colour
            float4 col = i % 2 == 0 ? float4(0.5, 1.0, 0.0, 1.0) : float4(0.5, 0.0, 1.0, 1.0);

            // write the vertex pos and color
            gpu_dbg_set_vertex(lv++, pos + float3(lx, lx, cos(lt) * 25.0) - float3(strip_length, strip_length, 0.0) * 0.5, col);

            lx += xstep;
            lt += tstep;
        }
    }

    uint label_line_strip[] = { 'L', 'i', 'n', 'e', ' ', 'S', 't', 'r', 'i', 'p' };
    gpu_dbg_text(label_line_strip, pos - label_offset, label_size, white);

    // line loop
    type_loc = uint2(type_pos / irc, type_pos % irc);
    pos = float3(type_loc.xy * section_size, 0.0) - grid_offset;
    type_pos++;

    {
        // reserve space for a line loop with segments
        int line_segments = 16;
        uint lv = gpu_dbg_lines(gpu_dbg_cmd_line_loop_3d(), line_segments, 5.0);

        float angle = 0.0;
        float angle_step = (pi * 2.0) / (line_segments-1);
        float rad = 100.0;

        for(int i = 0; i < line_segments; ++i)
        {
            float4 col = i == line_segments - 1 ? float4(1.0, 0.0, 0.5, 1.0) : float4(0.0, 1.0, 1.0, 1.0);

            float x = cos(angle);
            float y = -sin(angle);

            gpu_dbg_set_vertex(lv++, pos + float3(x, y, 0.0) * rad, col);

            angle += angle_step;
        }
    }

    uint label_line_loop[] = { 'L', 'i', 'n', 'e', ' ', 'L', 'o', 'o', 'p' };
    gpu_dbg_text(label_line_loop, pos - label_offset, label_size, white);

    // text vars
    type_loc = uint2(type_pos / irc, type_pos % irc);
    pos = float3(type_loc.xy * section_size, 0.0) - grid_offset;
    type_pos++;

    // currently only int or float are supported
    float4 text_col = float4(0.5, 1.0, 1.0, 1.0);

    uint small[] = { 's', 'm', 'a', 'l', 'l', ' ', '=', ' ', '%', 'f' };
    gpu_dbg_textf_ex(small, pos, label_size * 1.25, text_col, float4(0.000001, 0.0, 0.0, 0.0), gpu_dbg_text_align_left(), float4(1.0, 0.0, 0.0, 1.0));

    uint flt[] = { 'f', 'l', 'o', 'a', 't', ' ', '=', ' ', '%', 'f' };
    gpu_dbg_textf(flt, pos - float3(0.0, 0.0, label_size), label_size * 1.25, text_col, float4(10.23, 0.0, 0.0, 0.0));

    uint integer[] = { 'i', 'n', 't', ' ', '=', ' ', '%', 'i' };
    gpu_dbg_textf(integer, pos - float3(0.0, 0.0, label_size * 2), label_size * 1.25, text_col, float4(123.0, 0.0, 0.0, 0.0));

    uint label_text_vars[] = { 'T', 'e', 'x', 't', ' ', 'V', 'a', 'r', 's' };
    gpu_dbg_text(label_text_vars, pos - label_offset, label_size, white);

    // print to the cpu console
    uint msg[] = { 'g', 'p', 'u', '_', 'd', 'b', 'g', ' ', 'c', 'o', 'm', 'm', 'a', 'n', 'd', 's', ' ', '%', 'i', ' ', 'v', 'e', 'r', 't', 'i', 'c', 'e', 's', ' ', '%', 'i' };
    gpu_dbg_printf(msg, float4(gpu_dbg_counters[0].command_pos, gpu_dbg_counters[0].vertex_pos, 0.0, 0.0));
}

//
// rendering
//

GpuDbgVertexOutput gpu_dbg_vertex_output_default()
{
    GpuDbgVertexOutput output;
    output.position = float4(0.0, 0.0, 0.0, 0.0);
    output.uv = float2(0.0, 0.0);
    output.color = float4(0.0, 0.0, 0.0, 0.0);
    output.alt_color = float4(0.0, 0.0, 0.0, 0.0);
    output.type = 0;
    return output;
}

// Description:
//      Scale from a size in pixels relative to a 2160p screen height, to a half extent in ndc space
float2 gpu_dbg_pixel_scale(float2 size)
{
    return size * (gpu_dbg_screen_size.y / 2160.0) / gpu_dbg_screen_size;
}

// unit quad corner for vertex 'i' (0-3)
float2 gpu_dbg_quad_corner(uint i)
{
    return float2(i&1, (i>>1)&1);
}

// Description:
//      Returns the font atlas uv for char 'c', the atlas is an 8x8 grid of ascii 32-95. lower case uses the upper case
//      glyphs and unsupported chars map to '?'
float2 gpu_dbg_char_to_uv(uint c, float2 unit_uv)
{
    if(c >= 'a' && c <= 'z')
    {
        c -= 32;
    }

    uint i = (c >= 32 && c <= 95) ? c - 32 : '?' - 32;
    return (float2(i % 8, i / 8) + unit_uv) / 8.0;
}

// Description:
//      Quad for char 'c' of a text command, projected from the command pos and aligned with the command flags
GpuDbgVertexOutput gpu_dbg_text_vertex_3d(uint i, uint c, GpuDbgCommand cmd)
{
    // project pos
    float4 ndc = mul(gpu_dbg_view_projection_matrix, float4(cmd.pos, 1.0));
    ndc /= ndc.w;

    // clip vertex behind the near plane
    if(ndc.z < 0.0)
    {
        return gpu_dbg_vertex_output_default();
    }

    // start centred
    float2 scale = gpu_dbg_pixel_scale(cmd.size.xy);
    float2 spos = ndc.xy;
    float len = cmd.data_length;

    // position char
    if(cmd.flags & gpu_dbg_text_align_left())
    {
        spos.x += (c * scale.x);                            // char pos
    }
    else if(cmd.flags & gpu_dbg_text_align_right())
    {
        spos.x -= ((scale.x * len) - scale.x) * 1.0;        // align right
        spos.x += (c * scale.x);                            // char pos
    }
    else
    {
        spos.x -= ((scale.x * len) - scale.x) * 0.5;        // align centre
        spos.x += (c * scale.x);                            // char pos
    }

    // construct quad
    float2 corner = gpu_dbg_quad_corner(i);

    GpuDbgVertexOutput output = gpu_dbg_vertex_output_default();
    output.position = float4(spos + (corner * 2.0 - 1.0) * scale, 0.0, 1.0);

    // read char data
    output.uv = gpu_dbg_char_to_uv(gpu_dbg_data[cmd.data_offset + c], float2(corner.x, 1.0 - corner.y));
    output.color = cmd.color;
    output.alt_color = cmd.alt_color;
    return output;
}

GpuDbgVertexOutput gpu_dbg_quad_vertex_2d(uint i, float2 pos, float2 size)
{
    float2 spos = (pos / gpu_dbg_screen_size) * 2.0 - 1.0;
    float2 scale = gpu_dbg_pixel_scale(size);

    GpuDbgVertexOutput output = gpu_dbg_vertex_output_default();
    float2 qpos = spos + (gpu_dbg_quad_corner(i) * 2.0 - 1.0) * (scale * 2.0);
    output.position = float4(qpos, 0.0, 1.0);
    return output;
}

GpuDbgVertexOutput gpu_dbg_point_vertex_3d(uint i, float3 pos, float2 size)
{
    // project pos
    float4 ndc = mul(gpu_dbg_view_projection_matrix, float4(pos, 1.0));
    ndc /= ndc.w;

    // clip vertex behind the near plane
    if(ndc.z < 0.0)
    {
        return gpu_dbg_vertex_output_default();
    }

    float2 scale = gpu_dbg_pixel_scale(size);

    GpuDbgVertexOutput output = gpu_dbg_vertex_output_default();
    float2 qpos = ndc.xy + (gpu_dbg_quad_corner(i) * 2.0 - 1.0) * scale;
    output.position = float4(qpos, 0.0, 1.0);
    return output;
}

// Description:
//      Expand a line segment from 'v0' to 'v1' in ndc space into a quad 'thickness' pixels wide
GpuDbgVertexOutput gpu_dbg_line_quad_vertex(uint i, float2 ndc0, float2 ndc1, float4 col0, float4 col1, float thickness)
{
    // projected 2D line dir, normal and perp
    float2 ll = ndc1 - ndc0;
    float2 ln = normalize(ll);
    float2 lp = float2(ln.y, -ln.x);

    // * 2.0 because it's applied in ndc space, clamped so lines stay at least 1 pixel wide at low resolution
    float2 scale = max(gpu_dbg_pixel_scale(thickness * 2.0), 1.0 / gpu_dbg_screen_size);
    float2 uv = gpu_dbg_quad_corner(i);

    GpuDbgVertexOutput output = gpu_dbg_vertex_output_default();
    float2 qpos = ndc0 + ((uv.x * 2.0 - 1.0) * lp * scale) + (ll * uv.y);
    output.position = float4(qpos, 0.0, 1.0);
    output.color = lerp(col0, col1, uv.y);
    return output;
}

GpuDbgVertexOutput gpu_dbg_line_vertex_3d(uint i, GpuDbgCommand cmd, uint gid)
{
    uint stride = cmd.type == gpu_dbg_cmd_line_list_3d() ? 2 : 1;
    uint offset = cmd.data_offset + (gid * stride);
    uint next = cmd.data_offset + (((gid * stride) + 1) % cmd.data_length);

    GpuDbgVertex v0 = gpu_dbg_vertices[offset];
    GpuDbgVertex v1 = gpu_dbg_vertices[next];

    // project positions
    float4 clip0 = mul(gpu_dbg_view_projection_matrix, float4(v0.pos, 1.0));
    float4 clip1 = mul(gpu_dbg_view_projection_matrix, float4(v1.pos, 1.0));

    // clip the line against the frustum planes in clip space, (w +/- x, w +/- y, z, w - z)
    float4 planes[6] = {
        float4( 1.0,  0.0,  0.0, 1.0),
        float4(-1.0,  0.0,  0.0, 1.0),
        float4( 0.0,  1.0,  0.0, 1.0),
        float4( 0.0, -1.0,  0.0, 1.0),
        float4( 0.0,  0.0,  1.0, 0.0),
        float4( 0.0,  0.0, -1.0, 1.0)
    };

    [unroll]
    for(uint p = 0; p < 6; ++p)
    {
        float d0 = dot(clip0, planes[p]);
        float d1 = dot(clip1, planes[p]);

        if(d0 <= 0.0 && d1 <= 0.0)
        {
            return gpu_dbg_vertex_output_default();
        }

        if(d0 <= 0.0)
        {
            clip0 = lerp(clip0, clip1, d0 / (d0 - d1));
        }
        else if(d1 <= 0.0)
        {
            clip1 = lerp(clip1, clip0, d1 / (d1 - d0));
        }
    }

    return gpu_dbg_line_quad_vertex(i, clip0.xy / clip0.w, clip1.xy / clip1.w, v0.color, v1.color, cmd.size.x);
}

GpuDbgVertexOutput gpu_dbg_line_vertex_2d(uint i, GpuDbgCommand cmd, uint gid)
{
    uint stride = cmd.type == gpu_dbg_cmd_line_list_ndc() ? 2 : 1;
    uint offset = cmd.data_offset + (gid * stride);
    uint next = cmd.data_offset + (((gid * stride) + 1) % cmd.data_length);

    GpuDbgVertex v0 = gpu_dbg_vertices[offset];
    GpuDbgVertex v1 = gpu_dbg_vertices[next];

    return gpu_dbg_line_quad_vertex(i, v0.pos.xy, v1.pos.xy, v0.color, v1.color, cmd.size.x);
}

// 1 group per quad (line segment or point), the command index is set per indirect dispatch in gpu_dbg_draw_id
[numthreads(4, 1, 1)]
[outputtopology("triangle")]
void ms_gpu_dbg(
    uint gid : SV_GroupID,
    uint tid : SV_GroupThreadID,
    out indices uint3 tris[2],
    out vertices GpuDbgVertexOutput verts[4]
)
{
    // fetch associated debug draw command
    GpuDbgCommand cmd = gpu_dbg_commands[gpu_dbg_draw_id];

    SetMeshOutputCounts(4, 2);

    if(tid < 2)
    {
        tris[tid] = tid == 0 ? uint3(0, 1, 2) : uint3(2, 1, 3);
    }

    GpuDbgVertexOutput v;
    if(cmd.type == gpu_dbg_cmd_point())
    {
        v = gpu_dbg_point_vertex_3d(tid, cmd.pos, cmd.size.xy);
        v.color = cmd.color;
    }
    else if(cmd.type == gpu_dbg_cmd_quad_2d())
    {
        v = gpu_dbg_quad_vertex_2d(tid, cmd.pos.xy, cmd.size.xy);
        v.color = cmd.color;
    }
    else if(cmd.type == gpu_dbg_cmd_text_3d())
    {
        v = gpu_dbg_text_vertex_3d(tid, gid, cmd);
    }
    else if(cmd.type >= gpu_dbg_cmd_line_strip_ndc())
    {
        v = gpu_dbg_line_vertex_2d(tid, cmd, gid);
    }
    else
    {
        v = gpu_dbg_line_vertex_3d(tid, cmd, gid);
    }

    v.type = cmd.type;
    verts[tid] = v;
}

float4 ps_gpu_dbg(GpuDbgVertexOutput input) : SV_Target
{
    if(input.type == gpu_dbg_cmd_text_3d())
    {
        // sdf font with optional outline
        float edge_threshold = 0.5;
        float outline_threshold = 0.4;

        float text = gpu_dbg_font_atlas.Sample(gpu_dbg_linear_sampler, input.uv).r;

        float alpha = smoothstep(outline_threshold - 0.01, outline_threshold + 0.01, text);
        float inner_alpha = smoothstep(edge_threshold - 0.01, edge_threshold + 0.01, text);

        float4 color = lerp(input.alt_color, input.color, inner_alpha);
        color = lerp(float4(input.alt_color.rgb, 0.0), color, alpha);

        if(color.a == 0.0)
        {
            discard;
        }

        return color;
    }

    return input.color;
}

[numthreads(1, 1, 1)]
void cs_gpu_dbg_reset()
{
    gpu_dbg_counters[0].command_pos = 0;
    gpu_dbg_counters[0].vertex_pos = 0;
    gpu_dbg_counters[0].data_pos = 0;
}

[numthreads(1, 1, 1)]
void cs_gpu_dbg_demo()
{
    gpu_dbg_demo();
}
