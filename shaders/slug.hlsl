// ===================================================
// Reference vertex and pixel shaders for the Slug algorithm.
// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright 2017, by Eric Lengyel.
// https://github.com/EricLengyel/Slug, see third_party/slug for the licence.
//
// Ported to hotline: the vertex attributes are a named struct so pmfx can reflect the layout, the entry points are
// renamed and resources are bound with explicit registers. The algorithm is unchanged. The curve and band data are
// built on the cpu by `hotline_rs::slug`
// ===================================================

// The curve and band textures use a fixed width of 4096 texels.

#define kLogBandTextureWidth 12

// It's convenient to have a texel load function to aid in translation to other shader languages.

#define TexelLoad2D(x, y) x.Load(int3(y, 0))

// pos.xy = object-space vertex coordinates.
// pos.zw = object-space normal vector.
// tex.xy = em-space sample coordinates.
// tex.z = location of glyph data in band texture (interpreted as integer): x in bits 0-15, y in bits 16-31
// tex.w = max band indexes and flags (interpreted as integer): band max x in bits 0-7, band max y in bits 16-23,
//         even-odd flag E in bit 28
// jac = inverse Jacobian matrix entries (00, 01, 10, 11).
// bnd = (band scale x, band scale y, band offset x, band offset y).
// col = vertex color (red, green, blue, alpha).

struct vs_input {
    float4 pos : POSITION;
    float4 tex : TEXCOORD0;
    float4 jac : TEXCOORD1;
    float4 bnd : TEXCOORD2;
    float4 col : TEXCOORD3;
};

struct ps_input {
    float4 position : SV_Position;                  // Clip-space vertex position.
    float4 color : COLOR;                           // Vertex color.
    float2 texcoord : TEXCOORD0;                    // Em-space sample coordinates.
    nointerpolation float4 banding : TEXCOORD1;     // Band scale and offset, constant over glyph.
    nointerpolation int4 glyph : TEXCOORD2;         // (glyph data x, glyph data y, band max x, band max y and flags)
};

cbuffer slug_params : register(b0) {
    float4 slug_matrix[4];                          // The four rows of the MVP matrix.
    float4 slug_viewport;                           // The viewport dimensions, in pixels.
};

Texture2D curve_tex : register(t0);                 // Control point texture.
Texture2D<uint4> band_tex : register(t1);           // Band data texture.

//
// vertex shader
//

void SlugUnpack(float4 tex, float4 bnd, out float4 vbnd, out int4 vgly)
{
    uint2 g = asuint(tex.zw);
    vgly = int4(g.x & 0xFFFFU, g.x >> 16U, g.y & 0xFFFFU, g.y >> 16U);
    vbnd = bnd;
}

float2 SlugDilate(float4 pos, float4 tex, float4 jac, float4 m0, float4 m1, float4 m3, float2 dim, out float2 vpos)
{
    float2 n = normalize(pos.zw);
    float s = dot(m3.xy, pos.xy) + m3.w;
    float t = dot(m3.xy, n);

    float u = (s * dot(m0.xy, n) - t * (dot(m0.xy, pos.xy) + m0.w)) * dim.x;
    float v = (s * dot(m1.xy, n) - t * (dot(m1.xy, pos.xy) + m1.w)) * dim.y;

    float s2 = s * s;
    float st = s * t;
    float uv = u * u + v * v;
    float2 d = pos.zw * (s2 * (st + sqrt(uv)) / (uv - st * st));

    vpos = pos.xy + d;
    return (float2(tex.x + dot(d, jac.xy), tex.y + dot(d, jac.zw)));
}

ps_input vs_slug(vs_input input)
{
    float2 p;
    ps_input vresult;

    // Apply dynamic dilation to vertex position. Returns new em-space sample position.

    vresult.texcoord = SlugDilate(input.pos, input.tex, input.jac, slug_matrix[0], slug_matrix[1], slug_matrix[3], slug_viewport.xy, p);

    // Apply MVP matrix to dilated vertex position.

    vresult.position.x = p.x * slug_matrix[0].x + p.y * slug_matrix[0].y + slug_matrix[0].w;
    vresult.position.y = p.x * slug_matrix[1].x + p.y * slug_matrix[1].y + slug_matrix[1].w;
    vresult.position.z = p.x * slug_matrix[2].x + p.y * slug_matrix[2].y + slug_matrix[2].w;
    vresult.position.w = p.x * slug_matrix[3].x + p.y * slug_matrix[3].y + slug_matrix[3].w;

    // Unpack or pass through remaining vertex data.

    SlugUnpack(input.tex, input.bnd, vresult.banding, vresult.glyph);
    vresult.color = input.col;
    return (vresult);
}

//
// pixel shader
//

uint CalcRootCode(float y1, float y2, float y3)
{
    // Calculate the root eligibility code for a sample-relative quadratic Bezier curve.
    // Extract the signs of the y coordinates of the three control points.

    uint i1 = asuint(y1) >> 31U;
    uint i2 = asuint(y2) >> 30U;
    uint i3 = asuint(y3) >> 29U;

    uint shift = (i2 & 2U) | (i1 & ~2U);
    shift = (i3 & 4U) | (shift & ~4U);

    // Eligibility is returned in bits 0 and 8.

    return ((0x2E74U >> shift) & 0x0101U);
}

float2 SolveHorizPoly(float4 p12, float2 p3)
{
    // Solve for the values of t where the curve crosses y = 0.
    // The quadratic polynomial in t is given by
    //
    //     a t^2 - 2b t + c,
    //
    // where a = p1.y - 2 p2.y + p3.y, b = p1.y - p2.y, and c = p1.y.
    // The discriminant b^2 - ac is clamped to zero, and imaginary
    // roots are treated as a double root at the global minimum
    // where t = b / a.

    float2 a = p12.xy - p12.zw * 2.0 + p3;
    float2 b = p12.xy - p12.zw;
    float ra = 1.0 / a.y;
    float rb = 0.5 / b.y;

    float d = sqrt(max(b.y * b.y - a.y * p12.y, 0.0));
    float t1 = (b.y - d) * ra;
    float t2 = (b.y + d) * ra;

    // If the polynomial is nearly linear, then solve -2b t + c = 0.

    if (abs(a.y) < 1.0 / 65536.0) t1 = t2 = p12.y * rb;

    // Return the x coordinates where C(t) = 0.

    return (float2((a.x * t1 - b.x * 2.0) * t1 + p12.x, (a.x * t2 - b.x * 2.0) * t2 + p12.x));
}

float2 SolveVertPoly(float4 p12, float2 p3)
{
    // Solve for the values of t where the curve crosses x = 0.

    float2 a = p12.xy - p12.zw * 2.0 + p3;
    float2 b = p12.xy - p12.zw;
    float ra = 1.0 / a.x;
    float rb = 0.5 / b.x;

    float d = sqrt(max(b.x * b.x - a.x * p12.x, 0.0));
    float t1 = (b.x - d) * ra;
    float t2 = (b.x + d) * ra;

    // If the polynomial is nearly linear, then solve -2b t + c = 0.

    if (abs(a.x) < 1.0 / 65536.0) t1 = t2 = p12.x * rb;

    // Return the y coordinates where C(t) = 0.

    return (float2((a.y * t1 - b.y * 2.0) * t1 + p12.y, (a.y * t2 - b.y * 2.0) * t2 + p12.y));
}

int2 CalcBandLoc(int2 glyphLoc, uint offset)
{
    // If the offset causes the x coordinate to exceed the texture width, then wrap to the next line.

    int2 bandLoc = int2(glyphLoc.x + int(offset), glyphLoc.y);
    bandLoc.y += bandLoc.x >> kLogBandTextureWidth;
    bandLoc.x &= (1 << kLogBandTextureWidth) - 1;
    return (bandLoc);
}

float CalcCoverage(float xcov, float ycov, float xwgt, float ywgt, int flags)
{
    // Combine coverages from the horizontal and vertical rays using their weights.
    // Absolute values ensure that either winding direction convention works.

    float coverage = max(abs(xcov * xwgt + ycov * ywgt) / max(xwgt + ywgt, 1.0 / 65536.0), min(abs(xcov), abs(ycov)));

    // If SLUG_EVENODD is defined during compilation, then check E flag in tex.w. (See vertex shader.)

    #if defined(SLUG_EVENODD)

        if ((flags & 0x1000) == 0)
        {

    #endif

            // Using nonzero fill rule here.

            coverage = saturate(coverage);

    #if defined(SLUG_EVENODD)

        }
        else
        {
            // Using even-odd fill rule here.

            coverage = 1.0 - abs(1.0 - frac(coverage * 0.5) * 2.0);
        }

    #endif

    // If SLUG_WEIGHT is defined during compilation, then take a square root to boost optical weight.

    #if defined(SLUG_WEIGHT)

        coverage = sqrt(coverage);

    #endif

    return (coverage);
}

float SlugRender(Texture2D curveData, Texture2D<uint4> bandData, float2 renderCoord, float4 bandTransform, int4 glyphData)
{
    int curveIndex;

    // The effective pixel dimensions of the em square are computed
    // independently for x and y directions with texcoord derivatives.

    float2 emsPerPixel = fwidth(renderCoord);
    float2 pixelsPerEm = 1.0 / emsPerPixel;

    int2 bandMax = glyphData.zw;
    bandMax.y &= 0x00FF;

    // Determine what bands the current pixel lies in by applying a scale and offset
    // to the render coordinates. The scales are given by bandTransform.xy, and the
    // offsets are given by bandTransform.zw. Band indexes are clamped to [0, bandMax.xy].

    int2 bandIndex = clamp(int2(renderCoord * bandTransform.xy + bandTransform.zw), int2(0, 0), bandMax);
    int2 glyphLoc = glyphData.xy;

    float xcov = 0.0;
    float xwgt = 0.0;

    // Fetch data for the horizontal band from the index texture. The number
    // of curves intersecting the band is in the x component, and the offset
    // to the list of locations for those curves is in the y component.

    uint2 hbandData = TexelLoad2D(bandData, int2(glyphLoc.x + bandIndex.y, glyphLoc.y)).xy;
    int2 hbandLoc = CalcBandLoc(glyphLoc, hbandData.y);

    // Loop over all curves in the horizontal band.

    for (curveIndex = 0; curveIndex < int(hbandData.x); curveIndex++)
    {
        // Fetch the location of the current curve from the index texture.

        int2 curveLoc = int2(TexelLoad2D(bandData, int2(hbandLoc.x + curveIndex, hbandLoc.y)).xy);

        // Fetch the three 2D control points for the current curve from the curve texture.
        // The first texel contains both p1 and p2 in the (x,y) and (z,w) components, respectively,
        // and the the second texel contains p3 in the (x,y) components. Subtracting the render
        // coordinates makes the curve relative to the sample position. The quadratic Bezier curve
        // C(t) is given by
        //
        //     C(t) = (1 - t)^2 p1 + 2t(1 - t) p2 + t^2 p3

        float4 p12 = TexelLoad2D(curveData, curveLoc) - float4(renderCoord, renderCoord);
        float2 p3 = TexelLoad2D(curveData, int2(curveLoc.x + 1, curveLoc.y)).xy - renderCoord;

        // If the largest x coordinate among all three control points falls
        // left of the current pixel, then there are no more curves in the
        // horizontal band that can influence the result, so exit the loop.
        // (The curves are sorted in descending order by max x coordinate.)

        if (max(max(p12.x, p12.z), p3.x) * pixelsPerEm.x < -0.5) break;

        uint code = CalcRootCode(p12.y, p12.w, p3.y);
        if (code != 0U)
        {
            // At least one root makes a contribution. Calculate them and scale so
            // that the current pixel corresponds to the range [0,1].

            float2 r = SolveHorizPoly(p12, p3) * pixelsPerEm.x;

            // Bits in code tell which roots make a contribution.

            if ((code & 1U) != 0U)
            {
                xcov += saturate(r.x + 0.5);
                xwgt = max(xwgt, saturate(1.0 - abs(r.x) * 2.0));
            }

            if (code > 1U)
            {
                xcov -= saturate(r.y + 0.5);
                xwgt = max(xwgt, saturate(1.0 - abs(r.y) * 2.0));
            }
        }
    }

    float ycov = 0.0;
    float ywgt = 0.0;

    // Fetch data for the vertical band from the index texture. This follows
    // the data for all horizontal bands, so we have to add bandMax.y + 1.

    uint2 vbandData = TexelLoad2D(bandData, int2(glyphLoc.x + bandMax.y + 1 + bandIndex.x, glyphLoc.y)).xy;
    int2 vbandLoc = CalcBandLoc(glyphLoc, vbandData.y);

    // Loop over all curves in the vertical band.

    for (curveIndex = 0; curveIndex < int(vbandData.x); curveIndex++)
    {
        int2 curveLoc = int2(TexelLoad2D(bandData, int2(vbandLoc.x + curveIndex, vbandLoc.y)).xy);
        float4 p12 = TexelLoad2D(curveData, curveLoc) - float4(renderCoord, renderCoord);
        float2 p3 = TexelLoad2D(curveData, int2(curveLoc.x + 1, curveLoc.y)).xy - renderCoord;

        // If the largest y coordinate among all three control points falls
        // below the current pixel, then there are no more curves in the
        // vertical band that can influence the result, so exit the loop.
        // (The curves are sorted in descending order by max y coordinate.)

        if (max(max(p12.y, p12.w), p3.y) * pixelsPerEm.y < -0.5) break;

        uint code = CalcRootCode(p12.x, p12.z, p3.x);
        if (code != 0U)
        {
            float2 r = SolveVertPoly(p12, p3) * pixelsPerEm.y;

            if ((code & 1U) != 0U)
            {
                ycov -= saturate(r.x + 0.5);
                ywgt = max(ywgt, saturate(1.0 - abs(r.x) * 2.0));
            }

            if (code > 1U)
            {
                ycov += saturate(r.y + 0.5);
                ywgt = max(ywgt, saturate(1.0 - abs(r.y) * 2.0));
            }
        }
    }

    return (CalcCoverage(xcov, ycov, xwgt, ywgt, glyphData.w));
}

float4 ps_slug(ps_input vresult) : SV_Target
{
    float coverage = SlugRender(curve_tex, band_tex, vresult.texcoord, vresult.banding, vresult.glyph);
    return (vresult.color * coverage);
}
