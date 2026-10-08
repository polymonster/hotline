//
// gpu_dbg: gpu driven debug rendering
// call the GpuDbg_ functions from any shader to append debug draw commands, the commands are then rendered with
// a single `execute_indirect` of `ms_gpu_dbg` / `ps_gpu_dbg` where each command dispatches 1 mesh group per quad
//

// command types
uint GpuDbg_kPoint() { return 0; }
uint GpuDbg_kLineStrip3D() { return 1; }
uint GpuDbg_kLineList3D() { return 2; }
uint GpuDbg_kLineLoop3D() { return 3; }
uint GpuDbg_kLineStripNDC() { return 4; }
uint GpuDbg_kLineListNDC() { return 5; }
uint GpuDbg_kLineLoopNDC() { return 6; }
uint GpuDbg_kQuad2D() { return 7; }
uint GpuDbg_kText2D() { return 8; }
uint GpuDbg_kText3D() { return 9; }

// text flags
uint GpuDbg_kTextAlignCenter() { return 1<<0; }
uint GpuDbg_kTextAlignLeft() { return 1<<1; }
uint GpuDbg_kTextAlignRight() { return 1<<2; }

float GpuDbg_kPi() { return 3.14159265358979; }

struct GpuDbg_Counters
{
    uint m_commandPos; // must be first, it is used as the count buffer for execute_indirect
    uint m_vertexPos;
    uint m_dataPos;
    uint m_printDataPos; // not reset each frame, wraps around the print ring
};

struct GpuDbg_Command
{
    uint    m_type;
    float3  m_pos;
    float3  m_size;
    float4  m_color;
    float4  m_altColor;
    uint    m_flags;
    uint    m_dataOffset;
    uint    m_dataLength;
};

// root constant draw id + DispatchMesh args, consumed by execute_indirect
struct GpuDbg_DrawIndirectArgs
{
    uint m_drawId;
    uint m_groupCountX;
    uint m_groupCountY;
    uint m_groupCountZ;
};

struct GpuDbg_Vertex
{
    float3 m_pos;
    float4 m_color;
};

// Description:
//      Generic vertex output structure for mesh shader to pixel shader for rendering GpuDbg primitives
struct GpuDbg_VertexOutput
{
    float4 m_position : SV_POSITION;
    float2 m_uv : TEXCOORD0;
    float4 m_color : TEXCOORD1;
    float4 m_altColor : TEXCOORD2;
    nointerpolation uint m_type : TEXCOORD3;
};

RWStructuredBuffer<GpuDbg_Command>          gpu_dbg_commands : register(u0);
RWStructuredBuffer<GpuDbg_Vertex>           gpu_dbg_vertices : register(u1);
RWStructuredBuffer<GpuDbg_DrawIndirectArgs> gpu_dbg_draw_indirect_args : register(u2);
RWStructuredBuffer<GpuDbg_Counters>         gpu_dbg_counters : register(u3);
RWStructuredBuffer<uint>                    gpu_dbg_data : register(u4); // 1 char per uint
globallycoherent RWStructuredBuffer<uint>   gpu_dbg_print_data : register(u5); // ring of (lap << 8) | char, cpu mapped
Texture2D                                   gpu_dbg_font_atlas : register(t0);
SamplerState                                gpu_dbg_linear_sampler : register(s0);

cbuffer gpu_dbg_view : register(b0)
{
    float4x4 gpu_dbg_view_projection_matrix;
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
float4x4 GpuDbgUtils_createMatrixRotation(float3 axis, float angle)
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
float4x4 GpuDbgUtils_createMatrixScale(float3 scale)
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
float4x4 GpuDbgUtils_createMatrixTranslation(float3 translation)
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
//      fovY - field of view in radians
//      aspect - aspect ratio
//      nearZ - near plane distance
//      farZ - far plane distance
float4x4 GpuDbgUtils_createMatrixPerspectiveProjection(float fovY, float aspect, float nearZ, float farZ)
{
    float yScale = 1.0 / tan(fovY / 2.0);
    float xScale = yScale / aspect;
    float zRange = farZ - nearZ;
    float zScale = -(farZ + nearZ) / zRange;
    float wzScale = -2.0 * farZ * nearZ / zRange;

    return float4x4(
        xScale, 0.0,    0.0,     0.0,
        0.0,    yScale, 0.0,     0.0,
        0.0,    0.0,    zScale, -1.0,
        0.0,    0.0,    wzScale, 0.0
    );
}

// Description:
//      Returns the inverse of a 4x4 matrix
float4x4 GpuDbgUtils_inverse(float4x4 m)
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
void GpuDbgUtils_orthoBasisFromVector(float3 v, out float3 bt, out float3 t)
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
float3 GpuDbgUtils_chebyshevNormalize(float3 v)
{
    return (v.xyz / max(max(abs(v.x), abs(v.y)), abs(v.z)));
}

// Description:
//      Extracts the frustum corners groped as 4 near, 4 far where the winding order is the same for near and far
void GpuDbgUtils_frustumCornersFromMatrix(float4x4 mat, float nearT, float farT, out float3 corners[8])
{
    float4x4 inv = GpuDbgUtils_inverse(mat);

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

    if(nearT != 0.0 || farT != 1.0)
    {
        // linearly interpolate with near and far to get a slice of frustum
        for(int j = 0; j < 4; ++j)
        {
            float3 v = corners[j + 4] - corners[j];
            corners[j] = corners[j] + v * nearT;
            corners[j + 4] = corners[j] + v * farT;
        }
    }
}

//
// text utils
//

// Description:
//      Count the number of chars in 'val' and return the result, includes a '-' sign for negative numbers
int GpuDbgTextUtils_countIntChars(int val)
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
int GpuDbgTextUtils_fracToInt(float val, out int len, out int zeros)
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
int GpuDbgTextUtils_countFloatChars(float val)
{
    if(val == 0)
    {
        return 3;
    }

    int ipart = GpuDbgTextUtils_countIntChars((int)val);
    int fpart = 1; // '.'

    int flen = 0;
    int fzeros = 0;
    GpuDbgTextUtils_fracToInt(val, flen, fzeros);
    fpart += flen;

    return ipart + fpart;
}

// Description:
//      Targets for formatted chars, the text data buffer for GpuDbg_text or the print ring for GpuDbg_printf
uint GpuDbg_kTargetText() { return 0; }
uint GpuDbg_kTargetPrint() { return 1; }

// Description:
//      Write char 'c' to position 'pos' in 'target'. print chars are tagged with the lap of the ring they were written
//      in so the cpu can tell new chars from stale ones without syncing on the counter
void GpuDbg_putChar(uint target, uint pos, uint c)
{
    if(target == GpuDbg_kTargetText())
    {
        gpu_dbg_data[pos] = c;
    }
    else
    {
        uint size, stride;
        gpu_dbg_print_data.GetDimensions(size, stride);
        gpu_dbg_print_data[pos % size] = (((pos / size) + 1) << 8) | c;
    }
}

// Description:
//      Write the digits of 'val' into 'target' at 'cp' and return the new write position
int GpuDbgTextUtils_itocToData(int val, int cp, int leadingZeros, uint target)
{
    // count chars
    int charCount = GpuDbgTextUtils_countIntChars(val);

    // add minus sign
    if(val < 0)
    {
        GpuDbg_putChar(target, cp++, '-');
        charCount--;
    }

    // digit lookup
    static const int lutDigit[] = {
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
    int digitIter = charCount-1;
    while(digitIter >= 0)
    {
        if(leadingZeros > 0)
        {
            GpuDbg_putChar(target, cp++, '0');
            leadingZeros--;
            continue;
        }

        int denom = lutDigit[digitIter];
        int digit = numer / denom;

        GpuDbg_putChar(target, cp++, '0' + digit);

        numer -= (digit * denom);
        digitIter--;
    }

    return cp;
}

int GpuDbgTextUtils_ftocToData(float val, int cp, uint target)
{
    // int part
    cp = GpuDbgTextUtils_itocToData((int)val, cp, 0, target);

    // decimal place
    GpuDbg_putChar(target, cp++, '.');

    // frac part
    int flen;
    int fzeros;
    int fint = GpuDbgTextUtils_fracToInt(val, flen, fzeros);
    cp = GpuDbgTextUtils_itocToData(fint, cp, fzeros, target);

    return cp;
}

//
// api
//

// Description:
//      Allocate a command and its indirect args, `groups` is the number of mesh shader groups (quads) to dispatch
void GpuDbg_pushCommand(uint type, float3 pos, float3 size, float4 color, float4 altColor, uint flags, uint dataOffset, uint dataLength, uint groups)
{
    uint p = 0;
    InterlockedAdd(gpu_dbg_counters[0].m_commandPos, 1, p);

    GpuDbg_Command cmd;
    cmd.m_type = type;
    cmd.m_pos = pos;
    cmd.m_size = size;
    cmd.m_color = color;
    cmd.m_altColor = altColor;
    cmd.m_flags = flags;
    cmd.m_dataOffset = dataOffset;
    cmd.m_dataLength = dataLength;
    gpu_dbg_commands[p] = cmd;

    GpuDbg_DrawIndirectArgs args;
    args.m_drawId = p;
    args.m_groupCountX = groups;
    args.m_groupCountY = 1;
    args.m_groupCountZ = 1;
    gpu_dbg_draw_indirect_args[p] = args;
}

void GpuDbg_setVertex(uint index, float3 pos, float4 color)
{
    GpuDbg_Vertex v;
    v.m_pos = pos;
    v.m_color = color;
    gpu_dbg_vertices[index] = v;
}

// Description:
//      Append a draw call for a point drawn as a quad projected at the 3D point 'pos', with screen space size and colour
// Arguments:
//      pos - 3D position to project the point from
//      size - 2D screen size
//      color - RGBA color in 0-1 range
void GpuDbg_point3D(float3 pos, float2 size, float4 color)
{
    GpuDbg_pushCommand(GpuDbg_kPoint(), pos, float3(size, 1.0), color, (float4)0.0, 0, 0, 0, 1);
}

// Description:
//      Append a screen space quad with 'pos' and 'size' in pixels
void GpuDbg_quad2D(float2 pos, float2 size, float4 color)
{
    GpuDbg_pushCommand(GpuDbg_kQuad2D(), float3(pos, 0.0), float3(size, 1.0), color, (float4)0.0, 0, 0, 0, 1);
}

// Description:
//      Returns the number of chars 'text' formats to, %i and %f format specifiers take values from 'vars' in order
//      char literals are not supported in dxc templates, so '%' = 37, 'i' = 105, 'f' = 102
template<uint N>
uint GpuDbg_formatLength(uint text[N], float4 vars)
{
    uint varIndex = 0;
    uint charCount = 0;
    for(uint i = 0; i < N; ++i)
    {
        if(text[i] == 37 && i + 1 < N)
        {
            if(text[i + 1] == 105)
            {
                charCount += GpuDbgTextUtils_countIntChars((int)vars[varIndex]);
            }
            else if(text[i + 1] == 102)
            {
                charCount += GpuDbgTextUtils_countFloatChars(vars[varIndex]);
            }

            varIndex++;
            i += 1;
            continue;
        }

        charCount++;
    }

    return charCount;
}

// Description:
//      Write the formatted chars of 'text' into 'target' starting at 'cp', returns the next write position
template<uint N>
int GpuDbg_formatWrite(uint text[N], float4 vars, uint target, int cp)
{
    uint varIndex = 0;
    for(uint i = 0; i < N; ++i)
    {
        if(text[i] == 37 && i + 1 < N)
        {
            if(text[i + 1] == 105)
            {
                cp = GpuDbgTextUtils_itocToData((int)vars[varIndex], cp, 0, target);
            }
            else if(text[i + 1] == 102)
            {
                cp = GpuDbgTextUtils_ftocToData(vars[varIndex], cp, target);
            }

            varIndex++;
            i += 1;
            continue;
        }

        GpuDbg_putChar(target, cp++, text[i]);
    }

    return cp;
}

// Description:
//      Append text at the 3D point 'pos' with screen space char 'size'. 'text' is an array of char literals, %i and %f
//      format specifiers take values from 'vars' in order (up to 4). 'flags' controls alignment (GpuDbg_kTextAlign*)
//      and 'outlineColor' draws an outline around the glyphs
//      uint label[] = { 'x', ' ', '=', ' ', '%', 'f' };
//      GpuDbg_textf(label, pos, 50.0, color, float4(x, 0.0, 0.0, 0.0));
template<uint N>
void GpuDbg_textfEx(uint text[N], float3 pos, float size, float4 color, float4 vars, uint flags, float4 outlineColor)
{
    uint charCount = GpuDbg_formatLength(text, vars);

    uint dp = 0;
    InterlockedAdd(gpu_dbg_counters[0].m_dataPos, charCount, dp);
    GpuDbg_formatWrite(text, vars, GpuDbg_kTargetText(), dp);

    // push the draw, 1 quad per char
    GpuDbg_pushCommand(GpuDbg_kText3D(), pos, float3(size, size, 1.0), color, outlineColor, flags, dp, charCount, charCount);
}

// Description:
//      Print a line to the cpu console (stdout), with the same formatting as GpuDbg_textf. the cpu reads the print ring
//      persistently mapped, so lines arrive without waiting on the gpu
//      uint msg[] = { 'v', 'a', 'l', ' ', '%', 'i' };
//      GpuDbg_printf(msg, float4(val, 0.0, 0.0, 0.0));
template<uint N>
void GpuDbg_printf(uint text[N], float4 vars)
{
    uint charCount = GpuDbg_formatLength(text, vars);

    uint pp = 0;
    InterlockedAdd(gpu_dbg_counters[0].m_printDataPos, charCount + 1, pp);
    int cp = GpuDbg_formatWrite(text, vars, GpuDbg_kTargetPrint(), pp);

    // terminate the line, '\n' = 10
    GpuDbg_putChar(GpuDbg_kTargetPrint(), cp, 10);
}

template<uint N>
void GpuDbg_print(uint text[N])
{
    GpuDbg_printf(text, (float4)0.0);
}

template<uint N>
void GpuDbg_textf(uint text[N], float3 pos, float size, float4 color, float4 vars)
{
    GpuDbg_textfEx(text, pos, size, color, vars, 0, (float4)0.0);
}

template<uint N>
void GpuDbg_textEx(uint text[N], float3 pos, float size, float4 color, uint flags, float4 outlineColor)
{
    GpuDbg_textfEx(text, pos, size, color, (float4)0.0, flags, outlineColor);
}

template<uint N>
void GpuDbg_text(uint text[N], float3 pos, float size, float4 color)
{
    GpuDbg_textfEx(text, pos, size, color, (float4)0.0, 0, (float4)0.0);
}

// Description:
//      Allocate 'vertexCount' vertices for a line primitive of type 'prim' and return the offset to write them to
uint GpuDbg_lines(uint prim, uint vertexCount, float thickness)
{
    uint vp = 0;
    InterlockedAdd(gpu_dbg_counters[0].m_vertexPos, vertexCount, vp);

    // 1 quad per line segment
    uint groups = vertexCount - 1;
    if(prim == GpuDbg_kLineList3D() || prim == GpuDbg_kLineListNDC())
    {
        groups = vertexCount / 2;
    }
    else if(prim == GpuDbg_kLineLoop3D() || prim == GpuDbg_kLineLoopNDC())
    {
        groups = vertexCount;
    }

    GpuDbg_pushCommand(prim, (float3)0.0, float3(thickness, thickness, 1.0), (float4)0.0, (float4)0.0, 0, vp, vertexCount, groups);
    return vp;
}

void GpuDbg_line(float3 start, float3 end, float4 color, float thickness)
{
    uint vp = GpuDbg_lines(GpuDbg_kLineStrip3D(), 2, thickness);
    GpuDbg_setVertex(vp, start, color);
    GpuDbg_setVertex(vp + 1, end, color);
}

void GpuDbg_triangle(float3 p0, float3 p1, float3 p2, float4 color, float thickness)
{
    uint vp = GpuDbg_lines(GpuDbg_kLineLoop3D(), 3, thickness);
    GpuDbg_setVertex(vp, p0, color);
    GpuDbg_setVertex(vp + 1, p1, color);
    GpuDbg_setVertex(vp + 2, p2, color);
}

void GpuDbg_disc(float3 pos, float3 radius, float3 axis, float4 color, float thickness, uint segments)
{
    axis = normalize(axis);

    // ortho basis for axis
    float3 right, up;
    GpuDbgUtils_orthoBasisFromVector(axis, right, up);

    // make a circle in the plane where axis is the normal
    float angle = -GpuDbg_kPi();
    float angleStep = (GpuDbg_kPi() * 2.0) / (segments-1);

    uint vp = GpuDbg_lines(GpuDbg_kLineStrip3D(), segments, thickness);

    [loop]
    for(uint i = 0; i < segments; ++i)
    {
        float3 v1 = right * cos(angle) - up * sin(angle);
        GpuDbg_setVertex(vp + i, pos + v1 * radius, color);
        angle += angleStep;
    }
}

void GpuDbg_sphere(float3 pos, float radius, float4 color, float thickness, uint segments)
{
    // alloc space
    uint vp = GpuDbg_lines(GpuDbg_kLineStrip3D(), segments * segments, thickness);

    float angleStep = (GpuDbg_kPi() * 2.0) / (segments-1);
    float hangle = -GpuDbg_kPi();

    // series of discs
    [loop]
    for(uint j = 0; j < segments; ++j)
    {
        float3 right, up;

        // vertical discs
        float3 vaxis = float3(cos(hangle), -sin(hangle), 0.0);
        GpuDbgUtils_orthoBasisFromVector(vaxis, right, up);

        float angle = -GpuDbg_kPi();
        uint sliceOffset = j * segments;

        [loop]
        for(uint i = 0; i < segments; ++i)
        {
            float3 v1 = right * cos(angle) - up * sin(angle);
            GpuDbg_setVertex(vp + i + sliceOffset, pos + v1 * radius, color);
            angle += angleStep;
        }

        hangle += angleStep;
    }
}

void GpuDbg_hemisphere(float3 pos, float3 axis, float radius, float4 color, float thickness, uint segments)
{
    // ortho basis for axis
    float3 right, up;
    GpuDbgUtils_orthoBasisFromVector(axis, right, up);

    // alloc space
    uint quadSegments = max(segments / 4, 4);
    uint vp = GpuDbg_lines(GpuDbg_kLineList3D(), segments * 2 + (segments * quadSegments * 2), thickness);

    // make a circle in the plane where axis is the normal
    float angle = -GpuDbg_kPi();
    float angleStep = (GpuDbg_kPi() * 2.0) / (segments-1);
    float qangleStep = (GpuDbg_kPi() * 0.5) / quadSegments;

    [loop]
    for(uint i = 0; i < segments; ++i)
    {
        float3 v1 = right * cos(angle) - up * sin(angle);
        float3 v2 = right * cos(angle + angleStep) - up * sin(angle + angleStep);

        uint v = vp + (i * 2);

        // this forms the base
        GpuDbg_setVertex(v, pos + v1 * radius, color);
        GpuDbg_setVertex(v + 1, pos + v2 * radius, color);

        // add a line quadant to form the hemisphere
        float qangle = 0.0;

        for(uint j = 0; j < quadSegments; ++j)
        {
            float3 qv1 = v1 * cos(qangle) - axis * sin(qangle);
            float3 qv2 = v1 * cos(qangle - qangleStep) - axis * sin(qangle - qangleStep);

            v = vp + (segments * 2) + (j * 2) + (i * quadSegments * 2);

            GpuDbg_setVertex(v, pos + qv1 * radius, color);
            GpuDbg_setVertex(v + 1, pos + qv2 * radius, color);

            qangle -= qangleStep;
        }

        angle += angleStep;
    }
}

// Description:
//      Write 12 box edges as a line list from 8 corners ordered as 4 bottom, 4 top with the same winding
void GpuDbg_boxEdges(float3 corners[8], float4 color, float thickness)
{
    uint vp = GpuDbg_lines(GpuDbg_kLineList3D(), 24, thickness);

    for(uint i = 0; i < 4; ++i)
    {
        uint n = (i + 1) % 4;

        // bottom
        GpuDbg_setVertex(vp++, corners[i], color);
        GpuDbg_setVertex(vp++, corners[n], color);

        // top
        GpuDbg_setVertex(vp++, corners[i + 4], color);
        GpuDbg_setVertex(vp++, corners[n + 4], color);

        // sides
        GpuDbg_setVertex(vp++, corners[i], color);
        GpuDbg_setVertex(vp++, corners[i + 4], color);
    }
}

void GpuDbg_aabb(float3 aabbmin, float3 aabbmax, float4 color, float thickness)
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

    GpuDbg_boxEdges(corners, color, thickness);
}

void GpuDbg_obb(float4x4 worldMatrix, float4 color, float thickness)
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
        corners[i] = mul(worldMatrix, float4(corners[i], 1.0)).xyz;
    }

    GpuDbg_boxEdges(corners, color, thickness);
}

void GpuDbg_grid(float3 center, float3 axis, float2 dimension, float2 divisions, float4 color, float thickness)
{
    float3 right, up;
    GpuDbgUtils_orthoBasisFromVector(axis, right, up);

    float3 corner = center - (right * (float)(dimension.x / 2.0)) - (up * (float)(dimension.y / 2.0));
    uint vp = GpuDbg_lines(GpuDbg_kLineList3D(), (uint)(divisions.x + 1 + divisions.y + 1) * 2, thickness);

    float2 cellStep = dimension / divisions;

    float3 pos = corner;
    for(int i = 0; i < (int)(divisions.x + 1); ++i)
    {
        GpuDbg_setVertex(vp++, pos, color);
        GpuDbg_setVertex(vp++, pos + dimension.x * right, color);
        pos += up * cellStep.y;
    }

    pos = corner;
    for(int j = 0; j < (int)(divisions.y + 1); ++j)
    {
        GpuDbg_setVertex(vp++, pos, color);
        GpuDbg_setVertex(vp++, pos + dimension.y * up, color);
        pos += right * cellStep.x;
    }
}

void GpuDbg_cone(float3 apex, float3 axis, float cutoff, float height, float4 color, float thickness, uint sides)
{
    // axis must be normalized
    axis = normalize(axis);

    // ortho basis of cone from axis
    float3 bt, t;
    GpuDbgUtils_orthoBasisFromVector(axis, bt, t);

    // rotate a vector to the edge of the cos cutoff
    float x = cutoff;
    float y = sqrt(1.0 - pow(cutoff, 2.0));
    float3 v1 = t * x + axis * y;

    // flatten the vector on to the base of the code and take the difference to the axis to get the unit radius
    float rad = length(GpuDbgUtils_chebyshevNormalize(v1) - axis);
    float len = length(GpuDbgUtils_chebyshevNormalize(v1));

    // allocate space for lines
    uint basevp = GpuDbg_lines(GpuDbg_kLineLoop3D(), sides, thickness);
    uint corevp = GpuDbg_lines(GpuDbg_kLineList3D(), sides * 2 + 2, thickness);

    // add a line for the axis
    GpuDbg_setVertex(corevp + 0, apex, color);
    GpuDbg_setVertex(corevp + 1, apex + axis * height, color);

    // offset to put vertices for the sides
    uint sidesvp = corevp + 2;

    // iterate around the base adding lines
    float angle = -GpuDbg_kPi();
    float angleStep = (GpuDbg_kPi() * 2.0) / (float)sides;

    [loop]
    for(uint i = 0; i < sides; ++i)
    {
        float xr = cos(angle);
        float yr = -sin(angle);

        float3 ve = (apex + axis) + normalize(t * xr + bt * yr) * rad;
        float3 edge = apex + (normalize(ve - apex)) * height * len;

        // side
        GpuDbg_setVertex(sidesvp + i * 2, apex, color);
        GpuDbg_setVertex(sidesvp + i * 2 + 1, edge, color);

        // base
        GpuDbg_setVertex(basevp + i, edge, color);

        angle += angleStep;
    }
}

// Description:
//      Cylinder with optional hemisphere caps (capsule), 'capSegments' is the number of segments in each cap quadrant or 0 for a cylinder
void GpuDbg_cylinderInternal(float3 pos, float3 axis, float3 radius, float height, float4 color, float thickness, uint sides, uint capSegments)
{
    // axis must be normalized
    axis = normalize(axis);

    // ortho basis for axis
    float3 right, up;
    GpuDbgUtils_orthoBasisFromVector(axis, right, up);

    // make a circle in the plane where axis is the normal
    float angle = -GpuDbg_kPi();
    float angleStep = (GpuDbg_kPi() * 2.0) / (sides-1);
    float qangleStep = (GpuDbg_kPi() * 0.5) / max(capSegments, 1);

    // add a line per side for the top and bottom circles and the joining sides, plus the caps
    uint vp = GpuDbg_lines(GpuDbg_kLineList3D(), sides * 6 + (capSegments * sides * 4), thickness);

    // calc top and bottom pos of cylinder
    float3 bottomPos = pos - axis * height * 0.5;
    float3 topPos = pos + axis * height * 0.5;

    [loop]
    for(uint i = 0; i < sides; ++i)
    {
        float3 v1 = right * cos(angle) - up * sin(angle);
        float3 v2 = right * cos(angle + angleStep) - up * sin(angle + angleStep);

        uint v = vp + (i * 6);

        // bottom
        GpuDbg_setVertex(v, bottomPos + v1 * radius, color);
        GpuDbg_setVertex(v + 1, bottomPos + v2 * radius, color);

        // top
        GpuDbg_setVertex(v + 2, topPos + v1 * radius, color);
        GpuDbg_setVertex(v + 3, topPos + v2 * radius, color);

        // join sides
        GpuDbg_setVertex(v + 4, bottomPos + v1 * radius, color);
        GpuDbg_setVertex(v + 5, topPos + v1 * radius, color);

        // add a line quadrant to form the hemispheres at the top and bottom
        float qangle = 0.0;
        for(uint j = 0; j < capSegments; ++j)
        {
            float3 qv1 = v1 * cos(qangle) - axis * sin(qangle);
            float3 qv2 = v1 * cos(qangle - qangleStep) - axis * sin(qangle - qangleStep);

            v = vp + (sides * 6) + (j * 2) + (i * capSegments * 2);
            GpuDbg_setVertex(v, topPos + qv1 * radius, color);
            GpuDbg_setVertex(v + 1, topPos + qv2 * radius, color);

            // mirror for the bottom
            v += capSegments * 2 * sides;
            GpuDbg_setVertex(v, bottomPos + (v1 * cos(qangle) + axis * sin(qangle)) * radius, color);
            GpuDbg_setVertex(v + 1, bottomPos + (v1 * cos(qangle - qangleStep) + axis * sin(qangle - qangleStep)) * radius, color);

            qangle -= qangleStep;
        }

        angle += angleStep;
    }
}

void GpuDbg_cylinder(float3 pos, float3 axis, float3 radius, float height, float4 color, float thickness, uint sides)
{
    GpuDbg_cylinderInternal(pos, axis, radius, height, color, thickness, sides, 0);
}

void GpuDbg_capsule(float3 pos, float3 axis, float3 radius, float height, float4 color, float thickness, uint sides)
{
    GpuDbg_cylinderInternal(pos, axis, radius, height, color, thickness, sides, max(sides / 4, 4));
}

void GpuDbg_frustum(float4x4 viewProjectionMatrix, float nearT, float farT, float4 color, float thickness)
{
    float3 corners[8];
    GpuDbgUtils_frustumCornersFromMatrix(viewProjectionMatrix, nearT, farT, corners);
    GpuDbg_boxEdges(corners, color, thickness);
}

void GpuDbg_demo()
{
    float sectionSize = 350.0f;
    float labelSize = 55.0f;
    float3 labelOffset = float3(0.0, 0.0, 150.0);
    float4 white = float4(1.0, 1.0, 1.0, 1.0);
    float types = 16;
    float3 pos = float3(0.0, 0.0, 0.0);
    uint2 typeLoc = uint2(0, 0);
    uint typePos = 0;
    float kPi = GpuDbg_kPi();

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
    float3 gridOffset = float3(irc * sectionSize, irc * sectionSize, 0.0) * 0.5;
    GpuDbg_grid(float3(0.0, 0.0, -250.0), float3(0.0, 0.0, 1.0), gridOffset.xy * 2.0, float2(10.0, 10.0), float4(1.0, 1.0, 1.0, 1.0), 1.0);

    // offset primitives to the centre of the section
    gridOffset.xy -= sectionSize * 0.5;

    // point
    typeLoc = uint2(typePos / irc, typePos % irc);
    pos = float3(typeLoc.xy * sectionSize, 0.0) - gridOffset;
    typePos++;

    GpuDbg_point3D(pos, float2(50.0, 50.0), colors[typePos]);

    uint labelPoint[] = { 'P', 'o', 'i', 'n', 't' };
    GpuDbg_text(labelPoint, pos - labelOffset, labelSize, white);

    // line
    typeLoc = uint2(typePos / irc, typePos % irc);
    pos = float3(typeLoc.xy * sectionSize, 0.0) - gridOffset;
    typePos++;

    GpuDbg_line(pos - (float3)100.0, pos + (float3)100.0, colors[typePos], 3.0);

    uint labelLine[] = { 'L', 'i', 'n', 'e' };
    GpuDbg_text(labelLine, pos - labelOffset, labelSize, white);

    // triangle
    typeLoc = uint2(typePos / irc, typePos % irc);
    pos = float3(typeLoc.xy * sectionSize, 0.0) - gridOffset;
    typePos++;

    GpuDbg_triangle(
        pos - float3(100.0, 100.0, 100.0),
        pos + float3(100.0, -100.0, 100.0),
        pos + float3(100.0, 100.0, -100.0),
        colors[typePos],
        1.0
    );

    uint labelTriangle[] = { 'T', 'r', 'i', 'a', 'n', 'g', 'l', 'e' };
    GpuDbg_text(labelTriangle, pos - labelOffset, labelSize, white);

    // disc
    typeLoc = uint2(typePos / irc, typePos % irc);
    pos = float3(typeLoc.xy * sectionSize, 0.0) - gridOffset;
    typePos++;

    GpuDbg_disc(pos, float3(100.0, 100.0, 100.0), float3(-1.0, 1.0, 0.0), colors[typePos], 1.0, 16);

    uint labelDisc[] = { 'D', 'i', 's', 'c' };
    GpuDbg_text(labelDisc, pos - labelOffset, labelSize, white);

    // sphere
    typeLoc = uint2(typePos / irc, typePos % irc);
    pos = float3(typeLoc.xy * sectionSize, 0.0) - gridOffset;
    typePos++;

    GpuDbg_sphere(pos, 100.0, colors[typePos], 1.0, 16);

    uint labelSphere[] = { 'S', 'p', 'h', 'e', 'r', 'e' };
    GpuDbg_text(labelSphere, pos - labelOffset, labelSize, white);

    // hemi-sphere
    typeLoc = uint2(typePos / irc, typePos % irc);
    pos = float3(typeLoc.xy * sectionSize, 0.0) - gridOffset;
    typePos++;

    GpuDbg_hemisphere(pos, float3(0.0, 0.0, 1.0), 100.0, colors[typePos], 1.0, 16);

    uint labelHemisphere[] = { 'H', 'e', 'm', 'i', 's', 'p', 'h', 'e', 'r', 'e' };
    GpuDbg_text(labelHemisphere, pos - labelOffset, labelSize, white);

    // aabb
    typeLoc = uint2(typePos / irc, typePos % irc);
    pos = float3(typeLoc.xy * sectionSize, 0.0) - gridOffset;
    typePos++;

    GpuDbg_aabb(pos - float3(100.0, 100.0, 100.0), pos + float3(100.0, 100.0, 100.0), colors[typePos], 1.0);

    uint labelAABB[] = { 'A', 'A', 'B', 'B' };
    GpuDbg_text(labelAABB, pos - labelOffset, labelSize, white);

    // obb
    typeLoc = uint2(typePos / irc, typePos % irc);
    pos = float3(typeLoc.xy * sectionSize, 0.0) - gridOffset;
    typePos++;

    float4x4 oobRot = GpuDbgUtils_createMatrixRotation(float3(1.0, 1.0, 1.0), kPi * 0.25);
    float4x4 obbScale = GpuDbgUtils_createMatrixScale(float3(100.0, 100.0, 50.0));
    float4x4 obbTranslate = GpuDbgUtils_createMatrixTranslation(pos);
    float4x4 obbMat = mul(mul(obbTranslate, oobRot), obbScale);

    GpuDbg_obb(obbMat, colors[typePos], 1.0);

    uint labelOBB[] = { 'O', 'B', 'B' };
    GpuDbg_text(labelOBB, pos - labelOffset, labelSize, white);

    // cone
    typeLoc = uint2(typePos / irc, typePos % irc);
    pos = float3(typeLoc.xy * sectionSize, 0.0) - gridOffset;
    typePos++;

    float3 coneAxis = normalize(float3(1.0, 1.0, 1.0));
    GpuDbg_cone(pos - coneAxis * 50.0, coneAxis, 0.8, 100.0, colors[typePos], 1.0, 16);

    uint labelCone[] = { 'C', 'o', 'n', 'e' };
    GpuDbg_text(labelCone, pos - labelOffset, labelSize, white);

    // frustum
    typeLoc = uint2(typePos / irc, typePos % irc);
    pos = float3(typeLoc.xy * sectionSize, 0.0) - gridOffset;
    typePos++;

    float4x4 projMat = GpuDbgUtils_createMatrixPerspectiveProjection(1.04, 1.0, 0.1, 10000.0);

    float4x4 viewTranslate = GpuDbgUtils_createMatrixTranslation(pos - normalize(float3(1.0, 0.0, 1.0)) * 100.0);
    float4x4 viewRotate = GpuDbgUtils_createMatrixRotation(float3(0.0, 1.0, 0.0), -kPi * 0.75);

    float4x4 viewMat = transpose(GpuDbgUtils_inverse(mul(viewTranslate, viewRotate)));
    float4x4 viewProjMat = mul(viewMat, projMat);

    GpuDbg_frustum(viewProjMat, 0.001, 0.01, colors[typePos], 1.0);

    uint labelFrustum[] = { 'F', 'r', 'u', 's', 't', 'u', 'm' };
    GpuDbg_text(labelFrustum, pos - labelOffset, labelSize, white);

    // cylinder
    typeLoc = uint2(typePos / irc, typePos % irc);
    pos = float3(typeLoc.xy * sectionSize, 0.0) - gridOffset;
    typePos++;

    GpuDbg_cylinder(pos, float3(1.0, 1.0, 1.0), 50.0, 100.0, colors[typePos], 1.0, 16);

    uint labelCylinder[] = { 'C', 'y', 'l', 'i', 'n', 'd', 'e', 'r' };
    GpuDbg_text(labelCylinder, pos - labelOffset, labelSize, white);

    // capsule
    typeLoc = uint2(typePos / irc, typePos % irc);
    pos = float3(typeLoc.xy * sectionSize, 0.0) - gridOffset;
    typePos++;

    GpuDbg_capsule(pos, float3(1.0, 1.0, 1.0), 75.0, 100.0, colors[typePos], 1.0, 16);

    uint labelCapsule[] = { 'C', 'a', 'p', 's', 'u', 'l', 'e' };
    GpuDbg_text(labelCapsule, pos - labelOffset, labelSize, white);

    // line list
    typeLoc = uint2(typePos / irc, typePos % irc);
    pos = float3(typeLoc.xy * sectionSize, 0.0) - gridOffset;
    typePos++;

    {
        // reserve space for a line list with segments
        int lineSegments = 32;
        uint lv = GpuDbg_lines(GpuDbg_kLineList3D(), lineSegments * 2, 5.0);

        // sample cosine
        float listLength = 100.0;
        float lineLength = 20.0;
        float xstep = listLength / (float)lineSegments;
        float tstep = 8.0 * kPi / (float)lineSegments;

        float lx = 0.0;
        float lt = 0.0;

        for(int i = 0; i < lineSegments; ++i)
        {
            // alternate vertex colour
            float4 col = i % 2 == 0 ? float4(1.0, 0.5, 0.0, 1.0) : float4(0.0, 0.5, 1.0, 1.0);

            // write the vertex pos and color
            float3 mid = pos + float3(lx, lx, cos(lt) * 25.0) - float3(listLength, listLength, 0.0) * 0.5;
            float3 perp = float3(0.0, 0.0, 1.0);

            GpuDbg_setVertex(lv++, mid + perp * lineLength, col);
            GpuDbg_setVertex(lv++, mid - perp * lineLength, col);

            lx += xstep;
            lt += tstep;
        }
    }

    uint labelLineList[] = { 'L', 'i', 'n', 'e', ' ', 'L', 'i', 's', 't' };
    GpuDbg_text(labelLineList, pos - labelOffset, labelSize, white);

    // line strip
    typeLoc = uint2(typePos / irc, typePos % irc);
    pos = float3(typeLoc.xy * sectionSize, 0.0) - gridOffset;
    typePos++;

    {
        // reserve space for a line strip with segments
        int lineStripSegments = 32;
        uint lv = GpuDbg_lines(GpuDbg_kLineStrip3D(), lineStripSegments, 5.0);

        // sample cosine
        float stripLength = 100.0;
        float xstep = stripLength / (float)lineStripSegments;
        float tstep = 8.0 * kPi / (float)lineStripSegments;

        float lx = 0.0;
        float lt = 0.0;

        for(int i = 0; i < lineStripSegments; ++i)
        {
            // alternate vertex colour
            float4 col = i % 2 == 0 ? float4(0.5, 1.0, 0.0, 1.0) : float4(0.5, 0.0, 1.0, 1.0);

            // write the vertex pos and color
            GpuDbg_setVertex(lv++, pos + float3(lx, lx, cos(lt) * 25.0) - float3(stripLength, stripLength, 0.0) * 0.5, col);

            lx += xstep;
            lt += tstep;
        }
    }

    uint labelLineStrip[] = { 'L', 'i', 'n', 'e', ' ', 'S', 't', 'r', 'i', 'p' };
    GpuDbg_text(labelLineStrip, pos - labelOffset, labelSize, white);

    // line loop
    typeLoc = uint2(typePos / irc, typePos % irc);
    pos = float3(typeLoc.xy * sectionSize, 0.0) - gridOffset;
    typePos++;

    {
        // reserve space for a line loop with segments
        int lineSegments = 16;
        uint lv = GpuDbg_lines(GpuDbg_kLineLoop3D(), lineSegments, 5.0);

        float angle = 0.0;
        float angleStep = (kPi * 2.0) / (lineSegments-1);
        float rad = 100.0;

        for(int i = 0; i < lineSegments; ++i)
        {
            float4 col = i == lineSegments - 1 ? float4(1.0, 0.0, 0.5, 1.0) : float4(0.0, 1.0, 1.0, 1.0);

            float x = cos(angle);
            float y = -sin(angle);

            GpuDbg_setVertex(lv++, pos + float3(x, y, 0.0) * rad, col);

            angle += angleStep;
        }
    }

    uint labelLineLoop[] = { 'L', 'i', 'n', 'e', ' ', 'L', 'o', 'o', 'p' };
    GpuDbg_text(labelLineLoop, pos - labelOffset, labelSize, white);

    // text vars
    typeLoc = uint2(typePos / irc, typePos % irc);
    pos = float3(typeLoc.xy * sectionSize, 0.0) - gridOffset;
    typePos++;

    // currently only int or float are supported
    float4 textCol = float4(0.5, 1.0, 1.0, 1.0);

    uint small[] = { 's', 'm', 'a', 'l', 'l', ' ', '=', ' ', '%', 'f' };
    GpuDbg_textfEx(small, pos, labelSize * 1.25, textCol, float4(0.000001, 0.0, 0.0, 0.0), GpuDbg_kTextAlignLeft(), float4(1.0, 0.0, 0.0, 1.0));

    uint flt[] = { 'f', 'l', 'o', 'a', 't', ' ', '=', ' ', '%', 'f' };
    GpuDbg_textf(flt, pos - float3(0.0, 0.0, labelSize), labelSize * 1.25, textCol, float4(10.23, 0.0, 0.0, 0.0));

    uint integer[] = { 'i', 'n', 't', ' ', '=', ' ', '%', 'i' };
    GpuDbg_textf(integer, pos - float3(0.0, 0.0, labelSize * 2), labelSize * 1.25, textCol, float4(123.0, 0.0, 0.0, 0.0));

    uint labelTextVars[] = { 'T', 'e', 'x', 't', ' ', 'V', 'a', 'r', 's' };
    GpuDbg_text(labelTextVars, pos - labelOffset, labelSize, white);

    // print to the cpu console
    uint msg[] = { 'g', 'p', 'u', '_', 'd', 'b', 'g', ' ', 'c', 'o', 'm', 'm', 'a', 'n', 'd', 's', ' ', '%', 'i', ' ', 'v', 'e', 'r', 't', 'i', 'c', 'e', 's', ' ', '%', 'i' };
    GpuDbg_printf(msg, float4(gpu_dbg_counters[0].m_commandPos, gpu_dbg_counters[0].m_vertexPos, 0.0, 0.0));
}

//
// rendering
//

GpuDbg_VertexOutput GpuDbg_vertexOutputDefault()
{
    GpuDbg_VertexOutput output;
    output.m_position = float4(0.0, 0.0, 0.0, 0.0);
    output.m_uv = float2(0.0, 0.0);
    output.m_color = float4(0.0, 0.0, 0.0, 0.0);
    output.m_altColor = float4(0.0, 0.0, 0.0, 0.0);
    output.m_type = 0;
    return output;
}

// Description:
//      Scale from a size in pixels relative to a 2160p screen height, to a half extent in ndc space
float2 GpuDbg_pixelScale(float2 size)
{
    return size * (gpu_dbg_screen_size.y / 2160.0) / gpu_dbg_screen_size;
}

// unit quad corner for vertex 'i' (0-3)
float2 GpuDbg_quadCorner(uint i)
{
    return float2(i&1, (i>>1)&1);
}

// Description:
//      Returns the font atlas uv for char 'c', the atlas is an 8x8 grid of ascii 32-95. lower case uses the upper case
//      glyphs and unsupported chars map to '?'
float2 GpuDbg_charToUv(uint c, float2 unitUv)
{
    if(c >= 'a' && c <= 'z')
    {
        c -= 32;
    }

    uint i = (c >= 32 && c <= 95) ? c - 32 : '?' - 32;
    return (float2(i % 8, i / 8) + unitUv) / 8.0;
}

// Description:
//      Quad for char 'c' of a text command, projected from the command pos and aligned with the command flags
GpuDbg_VertexOutput GpuDbg_textVertex3D(uint i, uint c, GpuDbg_Command cmd)
{
    // project pos
    float4 ndc = mul(gpu_dbg_view_projection_matrix, float4(cmd.m_pos, 1.0));
    ndc /= ndc.w;

    // clip vertex behind the near plane
    if(ndc.z < 0.0)
    {
        return GpuDbg_vertexOutputDefault();
    }

    // start centred
    float2 scale = GpuDbg_pixelScale(cmd.m_size.xy);
    float2 spos = ndc.xy;
    float len = cmd.m_dataLength;

    // position char
    if(cmd.m_flags & GpuDbg_kTextAlignLeft())
    {
        spos.x += (c * scale.x);                            // char pos
    }
    else if(cmd.m_flags & GpuDbg_kTextAlignRight())
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
    float2 corner = GpuDbg_quadCorner(i);

    GpuDbg_VertexOutput output = GpuDbg_vertexOutputDefault();
    output.m_position = float4(spos + (corner * 2.0 - 1.0) * scale, 0.0, 1.0);

    // read char data
    output.m_uv = GpuDbg_charToUv(gpu_dbg_data[cmd.m_dataOffset + c], float2(corner.x, 1.0 - corner.y));
    output.m_color = cmd.m_color;
    output.m_altColor = cmd.m_altColor;
    return output;
}

GpuDbg_VertexOutput GpuDbg_quadVertex2D(uint i, float2 pos, float2 size)
{
    float2 spos = (pos / gpu_dbg_screen_size) * 2.0 - 1.0;
    float2 scale = GpuDbg_pixelScale(size);

    GpuDbg_VertexOutput output = GpuDbg_vertexOutputDefault();
    float2 qpos = spos + (GpuDbg_quadCorner(i) * 2.0 - 1.0) * (scale * 2.0);
    output.m_position = float4(qpos, 0.0, 1.0);
    return output;
}

GpuDbg_VertexOutput GpuDbg_pointVertex3D(uint i, float3 pos, float2 size)
{
    // project pos
    float4 ndc = mul(gpu_dbg_view_projection_matrix, float4(pos, 1.0));
    ndc /= ndc.w;

    // clip vertex behind the near plane
    if(ndc.z < 0.0)
    {
        return GpuDbg_vertexOutputDefault();
    }

    float2 scale = GpuDbg_pixelScale(size);

    GpuDbg_VertexOutput output = GpuDbg_vertexOutputDefault();
    float2 qpos = ndc.xy + (GpuDbg_quadCorner(i) * 2.0 - 1.0) * scale;
    output.m_position = float4(qpos, 0.0, 1.0);
    return output;
}

// Description:
//      Expand a line segment from 'v0' to 'v1' in ndc space into a quad 'thickness' pixels wide
GpuDbg_VertexOutput GpuDbg_lineQuadVertex(uint i, float2 ndc0, float2 ndc1, float4 col0, float4 col1, float thickness)
{
    // projected 2D line dir, normal and perp
    float2 ll = ndc1 - ndc0;
    float2 ln = normalize(ll);
    float2 lp = float2(ln.y, -ln.x);

    // * 2.0 because it's applied in ndc space, clamped so lines stay at least 1 pixel wide at low resolution
    float2 scale = max(GpuDbg_pixelScale(thickness * 2.0), 1.0 / gpu_dbg_screen_size);
    float2 uv = GpuDbg_quadCorner(i);

    GpuDbg_VertexOutput output = GpuDbg_vertexOutputDefault();
    float2 qpos = ndc0 + ((uv.x * 2.0 - 1.0) * lp * scale) + (ll * uv.y);
    output.m_position = float4(qpos, 0.0, 1.0);
    output.m_color = lerp(col0, col1, uv.y);
    return output;
}

GpuDbg_VertexOutput GpuDbg_lineVertex3D(uint i, GpuDbg_Command cmd, uint gid)
{
    uint stride = cmd.m_type == GpuDbg_kLineList3D() ? 2 : 1;
    uint offset = cmd.m_dataOffset + (gid * stride);
    uint next = cmd.m_dataOffset + (((gid * stride) + 1) % cmd.m_dataLength);

    GpuDbg_Vertex v0 = gpu_dbg_vertices[offset];
    GpuDbg_Vertex v1 = gpu_dbg_vertices[next];

    // project positions
    float4 clip0 = mul(gpu_dbg_view_projection_matrix, float4(v0.m_pos, 1.0));
    float4 clip1 = mul(gpu_dbg_view_projection_matrix, float4(v1.m_pos, 1.0));

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
            return GpuDbg_vertexOutputDefault();
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

    return GpuDbg_lineQuadVertex(i, clip0.xy / clip0.w, clip1.xy / clip1.w, v0.m_color, v1.m_color, cmd.m_size.x);
}

GpuDbg_VertexOutput GpuDbg_lineVertex2D(uint i, GpuDbg_Command cmd, uint gid)
{
    uint stride = cmd.m_type == GpuDbg_kLineListNDC() ? 2 : 1;
    uint offset = cmd.m_dataOffset + (gid * stride);
    uint next = cmd.m_dataOffset + (((gid * stride) + 1) % cmd.m_dataLength);

    GpuDbg_Vertex v0 = gpu_dbg_vertices[offset];
    GpuDbg_Vertex v1 = gpu_dbg_vertices[next];

    return GpuDbg_lineQuadVertex(i, v0.m_pos.xy, v1.m_pos.xy, v0.m_color, v1.m_color, cmd.m_size.x);
}

// 1 group per quad (line segment or point), the command index is set per indirect dispatch in gpu_dbg_draw_id
[numthreads(4, 1, 1)]
[outputtopology("triangle")]
void ms_gpu_dbg(
    uint gid : SV_GroupID,
    uint tid : SV_GroupThreadID,
    out indices uint3 tris[2],
    out vertices GpuDbg_VertexOutput verts[4]
)
{
    // fetch associated debug draw command
    GpuDbg_Command cmd = gpu_dbg_commands[gpu_dbg_draw_id];

    SetMeshOutputCounts(4, 2);

    if(tid < 2)
    {
        tris[tid] = tid == 0 ? uint3(0, 1, 2) : uint3(2, 1, 3);
    }

    GpuDbg_VertexOutput v;
    if(cmd.m_type == GpuDbg_kPoint())
    {
        v = GpuDbg_pointVertex3D(tid, cmd.m_pos, cmd.m_size.xy);
        v.m_color = cmd.m_color;
    }
    else if(cmd.m_type == GpuDbg_kQuad2D())
    {
        v = GpuDbg_quadVertex2D(tid, cmd.m_pos.xy, cmd.m_size.xy);
        v.m_color = cmd.m_color;
    }
    else if(cmd.m_type == GpuDbg_kText3D())
    {
        v = GpuDbg_textVertex3D(tid, gid, cmd);
    }
    else if(cmd.m_type >= GpuDbg_kLineStripNDC())
    {
        v = GpuDbg_lineVertex2D(tid, cmd, gid);
    }
    else
    {
        v = GpuDbg_lineVertex3D(tid, cmd, gid);
    }

    v.m_type = cmd.m_type;
    verts[tid] = v;
}

float4 ps_gpu_dbg(GpuDbg_VertexOutput input) : SV_Target
{
    if(input.m_type == GpuDbg_kText3D())
    {
        // sdf font with optional outline
        float edgeThreshold = 0.5;
        float outlineThreshold = 0.4;

        float text = gpu_dbg_font_atlas.Sample(gpu_dbg_linear_sampler, input.m_uv).r;

        float alpha = smoothstep(outlineThreshold - 0.01, outlineThreshold + 0.01, text);
        float innerAlpha = smoothstep(edgeThreshold - 0.01, edgeThreshold + 0.01, text);

        float4 color = lerp(input.m_altColor, input.m_color, innerAlpha);
        color = lerp(float4(input.m_altColor.rgb, 0.0), color, alpha);

        if(color.a == 0.0)
        {
            discard;
        }

        return color;
    }

    return input.m_color;
}

[numthreads(1, 1, 1)]
void cs_gpu_dbg_reset()
{
    gpu_dbg_counters[0].m_commandPos = 0;
    gpu_dbg_counters[0].m_vertexPos = 0;
    gpu_dbg_counters[0].m_dataPos = 0;
}

[numthreads(1, 1, 1)]
void cs_gpu_dbg_demo()
{
    GpuDbg_demo();
}
