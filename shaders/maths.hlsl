// returns constant pi
float pi() {
    return 3.141592653589793238462643;
}

// returns constant inverse pi (1.0/pi)
float inv_pi() {
    return 0.318309886;
}

// returns constant phi (golden ratio)
float phi() {
    return 1.61803399;
}

// returns constant tau which is the ratio of the circumference to the radius of a circle
float tau() {
    return 6.283185307179586;
}

// returns a vector normalized / projected to a square
float3 chebyshev_normalize(float3 v) {
    return (v.xyz / max(max(abs(v.x), abs(v.y)), abs(v.z)));
}

// performs signed distance union of `d1` and `d2`
float op_union(float d1, float d2) {
    return min(d1,d2);
}

// performs subtract operation from 2 distance fields `d1` and `d2`
float op_subtract(float d1, float d2) {
    return max(-d1,d2);
}

// intersects 2 distance fields `d1` and `d2`
float op_intersect(float d1, float d2) {
    return max(d1,d2);
}

// signed distance from point `p` to a to sphere centred at 0 with radius `s`
float sd_sphere(float3 p, float s) {
    return length(p)-s;
}

// signed distance from point `p` to a box centred at 0 with half extents `b`
float sd_box(float3 p, float3 b) {
    float3 d = abs(p) - b;
    return min(max(d.x,max(d.y,d.z)),0.0) + length(max(d,0.0));
}

// signed distance from point `p` to an octahedron centred at 0 with half extents `b`
float sd_octahedron(float3 p, float s) {
    p = abs(p);
    return (p.x + p.y + p.z - s) * 0.57735027;
}

// unsigned distance from point `p` to a box centred at 0 with half extents `b`
float ud_box(float3 p, float3 b) {
    return length(max(abs(p) - b, 0.0));
}

// unsigned distance from point `p` to rounded box centred at 0 with extents `b` and roundness `r`
float ud_round_box(float3 p, float3 b, float r) {
    return length(max(abs(p) - b, 0.0)) - r;
}

// signed distance from point `p` to a cross with size `s`
float sd_cross(float3 p, float2 s) {
    float da = sd_box(p.xyz, float3(s.y, s.x, s.x));
    float db = sd_box(p.yzx, float3(s.x, s.y, s.x));
    float dc = sd_box(p.zxy, float3(s.x, s.x, s.y));
    return op_union(da, op_union(db, dc));
}

// signed distance from point `p` to a tourus with size `t`
float sd_torus(float3 p, float2 t) {
    float2 q = float2(length(p.xy) - t.x,p.z);
    return length(q)-t.y;
}

// signed distance from point `p` to a cylinder with size `c`
float sd_cylinder(float3 p, float3 c) {
    return length(p.xz - c.xy) - c.z;
}

// signed distance from point `p` to a cone with size `c` where `c` must be normalized
float sd_cone(float3 p, float2 c) {
    float q = length(p.xy);
    return dot(c, float2(q, p.z));
}

// signed distance from point `p` to a plane with equation `n` where `n` must be normalized
// and n.xyz = plane normal, n.w = plane constant
float sd_plane(float3 p, float4 n) {
  return dot(p, n.xyz) + n.w;
}

// hash3 is used for voronoise
float3 hash3(float2 p) {
    float3 q = float3(
        dot(p, float2(127.1,311.7)), 
        dot(p, float2(269.5,183.3)), 
        dot(p, float2(419.2,371.9))
    );
	return frac(sin(q)*43758.5453);
}

// supply a 2d coordniate to sample the noise (p)
// control the noise type with u and v:
// cell noise (blocky squares): u = 0, v = 0
// voronoi (voronoi like): u = 1, v = 0
// voronoi noise (voronoi like but soomth): u = 1, v = 1
// noise (perlin like): u = 0, v = 1
float voronoise(float2 p, float u, float v) {
	float k = 1.0 + 63.0 * pow(1.0-v, 6.0);
    float2 i = floor(p);
    float2 f = frac(p);
	float2 a = float2(0.0,0.0);
    for(int y = -2; y <= 2; y++) {
        for(int x = -2; x <= 2; x++) {
            float2 g = float2(x, y);
            float3 o = hash3(i + g) * float3(u, u, 1.0);
            float2 d = g - f + o.xy;
            float w = pow(1.0 - smoothstep( 0.0, 1.414, length(d)), k);
            a += float2(o.z * w, w);
        }
    }
    return a.x/a.y;
}

// basic rng float for given 
float random(float2 uv) {
    return frac(sin(dot(uv.xy, float2(12.9898, 78.233))) * 43758.5453123);
}

// 2d noise for uv coordinate
// https://www.shadertoy.com/view/4dS3Wd
float noise(float2 uv) {
    float2 i = floor(uv);
    float2 f = frac(uv);

    // four corners in 2D of a tile
    float a = random(i);
    float b = random(i + float2(1.0, 0.0));
    float c = random(i + float2(0.0, 1.0));
    float d = random(i + float2(1.0, 1.0));

    float2 u = f * f * (3.0 - 2.0 * f);

    return lerp(a, b, u.x) + (c - a)* u.y * (1.0 - u.x) + (d - b) * u.x * u.y;
}

// fractal brownian motion for uv coordinate with n `octaves`
// https://www.shadertoy.com/view/4dS3Wd
float fbm(float2 uv, int octaves) {
    float value = 0.0;
    float amplitude = 0.3;
    float frequency = 0.0;
    for (int i = 0; i < octaves; i++) {
        value += amplitude * noise(uv);
        uv *= 3.0;
        amplitude *= 0.8;
    }
    return value;
}

// returns an rgb colour gradient for the value of x into 3 bands (like a heatmap)
float3 uv_gradient(float x) {
    float3 rgb_uv = float3(0.0, 0.0, 0.0);
    float grad = x % 1.0;
    if (grad < 0.333) {
        rgb_uv = lerp(float3(1.0, 0, 0.0), float3(0.0, 1.0, 0.0), grad * 3.333);
    }
    else if (grad < 0.666) {
        rgb_uv = lerp(float3(0.0, 1.0, 0.0), float3(0.0, 0.0, 1.0), (grad - 0.333) * 3.333);
    }
    else {
        rgb_uv = lerp(float3(0.0, 0.0, 1.0), float3(1.0, 0.0, 0.0), (grad - 0.666) * 3.333);
    }
    return rgb_uv;
}

// constructs ortho basis using hughes_moeller selecting from iput vector v, returning orthogonal vectors in bt and t
void construct_orthonormal_basis_hughes_moeller(float3 v, float3 out bt, float3 out t) {
    // choose a vector orthogonal to cv as the direction of b2.
    b = float3(0.0, -cv.z, cv.y);
    if(abs(n.x) > abs(n.z))
    {
        b = float3(-cv.y, cv.x, 0.0);
    }

    // normalise b2 and construct t
    b = b * rsqrt(dot(b, b));
    float3 t = cross(B, n);
}

// lambertian diffuse, where `l` is the direction from the light to the surface and `n` is the surface normal
float lambert(float3 l, float3 n) {
    return saturate(1.0 - dot(n, l));
}

// phong specular term, where `l` is the direction from the light to the surface, `n` is the surface normal
// `v` is the direction from the camera to the surface, `ks` is the specular coefficient useful in range 0-1
// `shininess controls the size of the highlight and useful ranges from 0-1000
float phong(
    float3 l,
    float3 n,
    float3 v,
    float ks,
    float shininess
) {
    return saturate(ks * pow(max(dot(reflect(-l, n), v), 0.0), shininess));
}

// blinn specular term, where `l` is the direction from surface to the light and `n` is the surface normal
// `v` is the direction from the camera to the surface, `ks` is the specular coefficient useful in range 0-1
// `shininess controls the size of the highlight and useful ranges from 0-1000
float blinn(
    float3 l,
    float3 n,
    float3 v,
    float ks,
    float shininess
) {
    float3 half_vector = -normalize(l + v);
    return saturate(ks * pow(saturate(dot(half_vector, n)), shininess));
}

// cook_torrance specular term with microfacet distribution
// where `l` is the direction from the light to the surface, `n` is the surface normal and `v` is the direction from the
// camera to the surface, `roughness` is the surface roughness in 0-1 range and `k` is the reflectivity coefficient
float cook_torrance(
    float3 l,
    float3 n,
    float3 v,
    float roughness,
    float k
) {
    l = -l;
    float n_dot_l = dot(n, l);
    if( n_dot_l > 0.0f ) {
        float roughness_sq = roughness * roughness;
        float3 hv = normalize(-v + l);
        
        float n_dot_v = dot(n, -v);
        float n_dot_h = dot(n, hv);
        float v_dot_h = dot(-v, hv);
        
        // geometric attenuation
        float n_dot_h_2 = 2.0f * n_dot_h;
        float g1 = (n_dot_h_2 * n_dot_v) / v_dot_h;
        float g2 = (n_dot_h_2 * n_dot_l) / v_dot_h;
        float geom_atten = min(1.0, min(g1, g2));
        
        // microfacet distribution function: beckmann
        float r1 = 1.0f / ( 4.0f * roughness_sq * pow(n_dot_h, 4.0f));
        float r2 = (n_dot_h * n_dot_h - 1.0) / (roughness_sq * n_dot_h * n_dot_h);
        float roughness_atten = r1 * exp(r2);
        
        // fresnel: schlick approximation
        float fresnel = pow(1.0 - v_dot_h, 5.0);
        fresnel *= roughness;
        fresnel += k;
                
        // specular
        float specular = (fresnel * geom_atten * roughness_atten) / (n_dot_v * n_dot_l * pi());
        return saturate(n_dot_l * (k + specular * (1.0 - k)));
    }   
    return 0.0;
}

// oren nayar diffuse to model reflection from rough surfaces
// where `l` is the direction from the light to the surface, `n` is the surface normal 
// `v` is the direction from the camera to the surface, `lum` represents the surface luminosity
// and `roughness` controls surface roughness in 0-1 range
float oren_nayar(
    float3 l,
    float3 n,
    float3 v,
    float lum,
    float roughness
) {
    l = -l;
    float l_dot_v = dot(l, v);
    float n_dot_l = dot(n, l);
    float n_dot_v = dot(n, v);

    float s = l_dot_v - n_dot_l * n_dot_v;
    float t = lerp(1.0, max(n_dot_l, n_dot_v), step(0.0, s));

    float sigma2 = roughness * roughness;
    float A = 1.0 + sigma2 * (lum / (sigma2 + 0.13) + 0.5 / (sigma2 + 0.33));
    float B = 0.45 * sigma2 / (sigma2 + 0.09);

    return max(n_dot_l, 0.0) * (A + B * s / t) / pi();
}

// calculates point light attenuation in respect to radius, where the light pos and world pos
// are both in world space, the atteniuation has infinite fall off
float point_light_attenuation(float3 light_pos, float radius, float3 world_pos) {
    float d = length(world_pos.xyz - light_pos.xyz);
    float r = radius;    
    float denom = d/r + 1.0;
    return 1.0 / (denom*denom);
}

// calculates a point light attentuation falloff such that the returned value reaches 0.0
// at the radius of the light and 1.0 when the distance to light is 0
float point_light_attenuation_cutoff(float3 light_pos, float radius, float3 world_pos) {
    float r = radius;
    float d = length(world_pos.xyz - light_pos.xyz);
    d = max(d - r, 0.0);
    float denom = d/r + 1.0;
    float attenuation = 1.0 / (denom*denom);
    float cutoff = 0.2;
    attenuation = (attenuation - cutoff) / (1.0 - cutoff);
    return max(attenuation, 0.0);
}

// returns a scalar attenuation coefficient for spot light, where `l` is the direction of the light to the surafce 
// cutoff is in radians (a wider or tighter cone) and the falloff ranges 0-1 (giving softer or harder edges)
float spot_light_attenuation(
    float3 l,
    float3 spot_dir,
    float  cutoff,
    float  falloff
) {
    float dp = (1.0 - dot(l, spot_dir));
    return smoothstep(cutoff, cutoff - falloff, dp);
}

// creates a crt scaline effect, returning src modulated, `tc` defines 0-1 uv space and tscale defines
// the scale of the crt texel size, use 1.0/image_size for 1:1 mapping, but you can tweak that for different effects
float3 crt_c(float3 src, float2 tc, float2 tscale) {
    float2 ca = float2(tscale.x * 2.0, 0.0);
    src.rgb *= saturate(abs(sin(tc.y / tscale.y/2.0)) + 0.5);
    return src;
}

// bends texcoords to create a crt monitor like curvature for the given `uv` in 0-1 range
float2 bend_tc(float2 uv){
    float2 tc = uv;
    float2 cc = tc - 0.5;
    float dist = dot(cc, cc) * 0.07;
    tc = tc * (tc + cc * (1.0 + dist) * dist) / tc;
    return tc;
}

// returns true if the sphere at `pos` with `radius` is inside or intersects the frustum
// defined by 6 planes (.xyz=normal, .w=constant (distance from origin))
bool sphere_vs_frustum(float3 pos, float radius, float4 planes[6]) {
    for (uint p = 0; p < 6; ++p) {
        float d = dot(pos, planes[p].xyz) + planes[p].w;
        if (d > radius) {
            return false;
        }
    }
    return true;
}

// returns true if an aabb defined by aabb_pos (centre) and aabb_extent (half extent) is inside or intersecting the frustum
// defined by 6 planes (.xyz=plane normal, .w=constant (distance from origin))
// implemented via info detailed in this insightful blog post: https://fgiesen.wordpress.com/2010/10/17/view-frustum-culling
bool aabb_vs_frustum(float3 aabb_pos, float3 aabb_extent, float4 planes[6]) {
    bool inside = true;
    for (int p = 0; p < 6; ++p) {
        float3 sign_flip = sign(planes[p].xyz) * -1.0f;
        float pd = planes[p].w;
        float d2 = dot(aabb_pos + aabb_extent * sign_flip, planes[p].xyz);
        if (d2 > -pd) {
            return false;
        }
    }
    return true;
}

// returns the fresnel factor for specular reflectance where `cos_theta` is h.v and `f0` is the reflectance at normal incidence
float3 fresnel_schlick(float cos_theta, float3 f0)
{
    return f0 + (1.0 - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

// returns the fresnel factor for specular reflectance accounting for material roughness where
// `cos_theta` is h.v and `f0` is the reflectance at normal incidence
// `roughness` is the material roughness in 0-1 range   
float3 fresnel_schlick_roughness(float cos_theta, float3 f0, float roughness)
{
    float invr = 1.0 - roughness;
    return f0 + (max(float3(invr, invr, invr), f0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

// normal distribution function (ndf) which returns the probability density of sufrace normals where
// `n` is the surface normal, `n_dot_h` is the dot product of surface normal and the half angle h
// and `roughness` is the material roughness in 0-1 range 
float distribution_beckmann(float3 n, float n_dot_h, float roughness)
{
    // microfacet distribution function: beckmann
    /*
    float r2 = roughness * roughness;
    float nh2 = n_dot_h * n_dot_h;
    float r1 = 1.0f / ( 4.0f * r2 * pow(n_dot_h, 4.0f));
    float r2 = (nh2 * nh2 - 1.0) / (r2 * nh2);
    return r1 * exp(r2);
    */

    float nh2 = n_dot_h * n_dot_h;
    float tanh = (1.0 - nh2) / nh2;
    float tanh2 = tanh*tanh;
    float r2 = roughness * roughness;
    float denom = (pi() * r2) * pow(n_dot_h, 4.0);
    return exp(-tanh2 / r2) / denom;
}

// normal distribution function (ndf) which returns the probability density of sufrace normals where
// `n` is the surface normal, `h` is the halfway vector between the surface normal and view direction
// and `roughness` is the material roughness in 0-1 range 
float distribution_ggx(float3 n, float3 h, float roughness)
{
    float a = roughness * roughness;
    float a2 = a * a;
    float nh = max(dot(h, n), 0.0);
    float nh2 = nh * nh;
    float nom = a2;
    float denom = (nh2 * (a2 - 1.0) + 1.0);
    denom = pi() * denom * denom;

    return nom / denom;
}

// geometry term which returns the probability that light will reflect light toward the viewer or be occluded
// where `n_dot_v` is the dot product of n (the surface normal) and v (the view direction)
// and `roughness` is the material roughness in 0-1 range 
float geometry_schlick_ggx(float n_dot_v, float roughness)
{
    float r = (roughness + 1.0);
    float k = (r * r) / 8.0;
    float nom = n_dot_v;
    float denom = n_dot_v * (1.0 - k) + k;

    return nom / denom;
}

// geometry term which returns the probability that light will reflect light toward the viewer or be occluded
// where `n` is the surface normal, `v` is the view direction, `l` is the light direction
// and `roughness` is the material roughness in 0-1 range 
float geometry_smith(float3 n, float3 v, float3 l, float roughness)
{
    float n_dot_v = max(dot(n, v), 0.0);
    float n_dot_l = max(dot(n, l), 0.0);
    float ggx2 = geometry_schlick_ggx(n_dot_v, roughness);
    float ggx1 = geometry_schlick_ggx(n_dot_l, roughness);

    return ggx1 * ggx2;
}

// probability density function which returns the density of scattered light for the light direction vector `l`
// viewing direction `v` and `g` is the assymetry factor (anisotropy) that describes the degree of forward or back scattering 
float phase_henyey_greenstein(float3 l, float3 v, float g)
{
    float cost = dot(l, -v);
    return (1.0 - g * g) / (4.0 * pi() * pow(1.0 + g * g - 2.0 * g * cost, 3.0 / 2.0));
}