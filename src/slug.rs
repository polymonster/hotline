///
/// Gpu text rendering directly from glyph outlines with the Slug algorithm (Eric Lengyel, JCGT 2017, patent dedicated
/// to the public domain in 2026). The shaders are the reference implementation in `shaders/slug.hlsl`, this module is
/// the cpu side they need: glyph outlines from a ttf converted to quadratic curves and packed into a curve texture,
/// with each glyph split into horizontal and vertical bands listing the curves that cross them in a band texture.
/// Text is drawn as one quad per glyph and the pixel shader computes coverage from the curves, so it stays sharp at
/// any scale with no glyph atlas
///

use crate::gfx;
use std::collections::HashMap;

/// Curve and band textures are this many texels wide, the shader wraps band data rows assuming it
pub const TEXTURE_WIDTH: usize = 4096;

/// Most bands to split a glyph into along each axis
const MAX_BANDS: usize = 16;

/// Bands overlap by this much (em) so curves on a band boundary are found from either side
const BAND_EPSILON: f32 = 1.0 / 1024.0;

/// Per vertex data, 5 float4s matching `vs_input` in `shaders/slug.hlsl`
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct SlugVertex {
    /// xy = object space position, zw = object space normal, scaled so the edges move out by the dilation distance
    pub pos: [f32; 4],
    /// xy = em space sample position, z = glyph data location in the band texture, w = band max and flags, the z and
    /// w are integers bit cast to floats
    pub tex: [f32; 4],
    /// Inverse Jacobian (00, 01, 10, 11) mapping object space offsets to em space
    pub jac: [f32; 4],
    /// Band scale xy and offset zw, mapping em space to band indices
    pub bnd: [f32; 4],
    /// Premultiplied colour
    pub col: [f32; 4]
}

/// Push constants for the `slug_text` pipeline, matching `cbuffer slug_params`
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct SlugParams {
    /// The four rows of the model view projection matrix
    pub matrix: [[f32; 4]; 4],
    /// Viewport width and height in pixels in xy
    pub viewport: [f32; 4]
}

/// A quadratic bezier in em space
#[derive(Clone, Copy, Debug, PartialEq)]
struct Curve {
    p: [[f32; 2]; 3]
}

impl Curve {
    fn min(&self, axis: usize) -> f32 {
        self.p[0][axis].min(self.p[1][axis]).min(self.p[2][axis])
    }

    fn max(&self, axis: usize) -> f32 {
        self.p[0][axis].max(self.p[1][axis]).max(self.p[2][axis])
    }

    /// Straight lines along an axis can't change the winding number for rays parallel to it, so they're left out of
    /// that axis's bands
    fn is_flat(&self, axis: usize) -> bool {
        self.p[0][axis] == self.p[1][axis] && self.p[1][axis] == self.p[2][axis]
    }
}

/// Collects glyph outlines from ttf-parser as em space quadratic curves, one list per contour
struct OutlineCollector {
    scale: f32,
    contours: Vec<Vec<Curve>>,
    start: [f32; 2],
    current: [f32; 2]
}

impl OutlineCollector {
    fn point(&self, x: f32, y: f32) -> [f32; 2] {
        [x * self.scale, y * self.scale]
    }

    fn push(&mut self, curve: Curve) {
        // zero length segments add nothing
        if curve.p[0] != curve.p[1] || curve.p[1] != curve.p[2] {
            self.contours.last_mut().unwrap().push(curve);
        }
        self.current = curve.p[2];
    }

    /// Lines are quadratics with the control point on the end point, as the reference implementation stores them
    fn line(&mut self, p: [f32; 2]) {
        self.push(Curve { p: [self.current, p, p] });
    }
}

impl ttf_parser::OutlineBuilder for OutlineCollector {
    fn move_to(&mut self, x: f32, y: f32) {
        self.start = self.point(x, y);
        self.current = self.start;
        self.contours.push(Vec::new());
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.point(x, y);
        self.line(p);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let c = self.point(x1, y1);
        let p = self.point(x, y);
        self.push(Curve { p: [self.current, c, p] });
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        // cubics (cff fonts) are split in half and each half approximated by a quadratic
        let p0 = self.current;
        let c1 = self.point(x1, y1);
        let c2 = self.point(x2, y2);
        let p3 = self.point(x, y);
        let mid = |a: [f32; 2], b: [f32; 2]| [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
        let ab = mid(p0, c1);
        let bc = mid(c1, c2);
        let cd = mid(c2, p3);
        let abc = mid(ab, bc);
        let bcd = mid(bc, cd);
        let m = mid(abc, bcd);
        let quad_control = |p0: [f32; 2], c1: [f32; 2], c2: [f32; 2], p3: [f32; 2]| [
            (3.0 * (c1[0] + c2[0]) - p0[0] - p3[0]) * 0.25,
            (3.0 * (c1[1] + c2[1]) - p0[1] - p3[1]) * 0.25
        ];
        self.push(Curve { p: [p0, quad_control(p0, ab, abc, m), m] });
        self.push(Curve { p: [m, quad_control(m, bcd, cd, p3), p3] });
    }

    fn close(&mut self) {
        if self.current != self.start {
            let start = self.start;
            self.line(start);
        }
    }
}

/// Where a glyph's data lives and how to find its bands, everything the vertices need
#[derive(Clone, Copy, Debug)]
pub struct SlugGlyphData {
    /// Em space bounds of the outline
    pub min: [f32; 2],
    pub max: [f32; 2],
    /// Texel of the glyph's band headers in the band texture
    pub location: [u16; 2],
    /// Number of vertical bands - 1 (x) and horizontal bands - 1 (y)
    pub band_max: [u32; 2],
    /// Scale (xy) and offset (zw) from em space to band index
    pub band_transform: [f32; 4]
}

#[derive(Clone, Copy, Debug)]
pub struct SlugGlyph {
    /// Horizontal advance in em
    pub advance: f32,
    /// `None` for glyphs with no outline, like space
    pub data: Option<SlugGlyphData>,
    id: ttf_parser::GlyphId
}

/// A font compiled for Slug rendering: per glyph metrics and the curve and band texture data
pub struct SlugFont {
    pub glyphs: HashMap<char, SlugGlyph>,
    /// Vertical metrics in em
    pub ascender: f32,
    pub descender: f32,
    pub line_gap: f32,
    /// Curve texture texels as 16 bit float bits (p1.x, p1.y, p2.x, p2.y), `TEXTURE_WIDTH` wide
    pub curve_texels: Vec<[u16; 4]>,
    /// Band texture texels (count, offset) for band headers and (x, y) curve locations for band lists
    pub band_texels: Vec<[u16; 2]>,
    /// Horizontal kerning between glyph pairs in em
    kerning: HashMap<(u16, u16), f32>
}

/// Converts to 16 bit float bits, rounding to nearest even
fn f32_to_f16(v: f32) -> u16 {
    let x = v.to_bits();
    let sign = ((x >> 16) & 0x8000) as u16;
    let exp = ((x >> 23) & 0xff) as i32;
    let mant = x & 0x7fffff;
    if exp == 0xff {
        return sign | 0x7c00 | if mant != 0 { 0x200 } else { 0 };
    }
    let e = exp - 127 + 15;
    if e >= 0x1f {
        return sign | 0x7c00;
    }
    let (half, rem, halfway) = if e <= 0 {
        // subnormal
        if e < -10 {
            return sign;
        }
        let m = mant | 0x800000;
        let shift = (14 - e) as u32;
        (m >> shift, m & ((1 << shift) - 1), 1 << (shift - 1))
    }
    else {
        (((e as u32) << 10) | (mant >> 13), mant & 0x1fff, 0x1000)
    };
    let rounded = if rem > halfway || (rem == halfway && (half & 1) == 1) { half + 1 } else { half };
    sign | rounded as u16
}

/// Picks the band count which minimises the most curves any one band has to test, fewer bands on a tie
fn choose_band_count(curves: &[Curve], axis: usize, min: f32, max: f32) -> usize {
    let size = max - min;
    if size <= 0.0 {
        return 1;
    }
    let mut best = (usize::MAX, 1);
    for n in 1..=MAX_BANDS {
        let band_size = size / n as f32;
        let mut counts = vec![0; n];
        for curve in curves.iter().filter(|c| !c.is_flat(axis)) {
            let first = ((curve.min(axis) - BAND_EPSILON - min) / band_size).floor().max(0.0) as usize;
            let last = ((curve.max(axis) + BAND_EPSILON - min) / band_size).floor().max(0.0) as usize;
            for count in &mut counts[first.min(n - 1)..=last.min(n - 1)] {
                *count += 1;
            }
        }
        let most = counts.into_iter().max().unwrap_or(0);
        if most < best.0 {
            best = (most, n);
        }
    }
    best.1
}

/// Indices of the curves crossing each of `n` bands along `axis`, which is the axis the bands are stacked along (y
/// for horizontal bands). Each list is sorted by descending max coordinate on the other axis, the direction the
/// shader's rays travel, so it can stop once the remaining curves are all behind the pixel
fn build_bands(curves: &[Curve], axis: usize, n: usize, min: f32, max: f32) -> Vec<Vec<usize>> {
    let band_size = (max - min) / n as f32;
    let ray_axis = 1 - axis;
    (0..n).map(|band| {
        let lo = min + band as f32 * band_size - BAND_EPSILON;
        let hi = min + (band + 1) as f32 * band_size + BAND_EPSILON;
        let mut list: Vec<usize> = (0..curves.len())
            .filter(|&i| !curves[i].is_flat(axis) && curves[i].max(axis) >= lo && curves[i].min(axis) <= hi)
            .collect();
        list.sort_by(|&a, &b| curves[b].max(ray_axis).total_cmp(&curves[a].max(ray_axis)));
        list
    }).collect()
}

/// Reserves `count` texels which don't straddle a row, returning the linear index of the first
fn alloc_in_row<T: Default + Clone>(texels: &mut Vec<T>, count: usize) -> usize {
    assert!(count <= TEXTURE_WIDTH, "slug: band data block of {count} texels is wider than the texture");
    if texels.len() % TEXTURE_WIDTH + count > TEXTURE_WIDTH {
        let pad = TEXTURE_WIDTH - texels.len() % TEXTURE_WIDTH;
        texels.resize(texels.len() + pad, T::default());
    }
    let start = texels.len();
    texels.resize(start + count, T::default());
    start
}

fn texel_location(index: usize) -> [u16; 2] {
    [(index % TEXTURE_WIDTH) as u16, (index / TEXTURE_WIDTH) as u16]
}

/// Writes contours into the curve texture, returning each curve's texel location. A curve takes 2 texels,
/// (p1, p2) then (p3, -), and consecutive curves in a contour share the texel at their common end point
fn pack_curves(contours: &[Vec<Curve>], texels: &mut Vec<[u16; 4]>) -> Vec<[u16; 2]> {
    let half = |p: [f32; 2]| [f32_to_f16(p[0]), f32_to_f16(p[1])];
    let mut locations = Vec::new();
    for contour in contours {
        let mut pending_end: Option<[f32; 2]> = None;
        for curve in contour {
            let shares = pending_end == Some(curve.p[0]) && texels.len() % TEXTURE_WIDTH != TEXTURE_WIDTH - 1;
            if !shares {
                // finish the previous curve and start a fresh run, both texels of a curve must be on one row
                if let Some(end) = pending_end {
                    let e = half(end);
                    texels.push([e[0], e[1], 0, 0]);
                }
                if texels.len() % TEXTURE_WIDTH == TEXTURE_WIDTH - 1 {
                    texels.push([0; 4]);
                }
            }
            let (p1, p2) = (half(curve.p[0]), half(curve.p[1]));
            locations.push(texel_location(texels.len()));
            texels.push([p1[0], p1[1], p2[0], p2[1]]);
            pending_end = Some(curve.p[2]);
        }
        if let Some(end) = pending_end {
            let e = half(end);
            texels.push([e[0], e[1], 0, 0]);
        }
    }
    locations
}

/// Writes a glyph's band headers, horizontal bands then vertical, followed by the band curve lists. Header offsets
/// are relative to the glyph's location, identical lists are only stored once
fn pack_bands(hbands: &[Vec<usize>], vbands: &[Vec<usize>], curve_locations: &[[u16; 2]], texels: &mut Vec<[u16; 2]>)
    -> [u16; 2] {
    let glyph = alloc_in_row(texels, hbands.len() + vbands.len());
    let mut stored: HashMap<&Vec<usize>, usize> = HashMap::new();
    for (i, band) in hbands.iter().chain(vbands.iter()).enumerate() {
        if band.is_empty() {
            continue;
        }
        let list = *stored.entry(band).or_insert_with(|| {
            let list = alloc_in_row(texels, band.len());
            for (j, &curve) in band.iter().enumerate() {
                texels[list + j] = curve_locations[curve];
            }
            list
        });
        let offset = list - glyph;
        assert!(offset <= u16::MAX as usize, "slug: band list too far from its glyph header");
        texels[glyph + i] = [band.len() as u16, offset as u16];
    }
    texel_location(glyph)
}

impl SlugFont {
    /// Compiles the glyphs for `chars` from ttf / otf font data
    pub fn from_ttf(font_data: &[u8], chars: impl IntoIterator<Item = char>) -> Result<Self, crate::Error> {
        let face = ttf_parser::Face::parse(font_data, 0)
            .map_err(|e| crate::Error { msg: format!("slug: failed to parse font: {e}") })?;
        let scale = 1.0 / face.units_per_em() as f32;

        let mut font = SlugFont {
            glyphs: HashMap::new(),
            ascender: face.ascender() as f32 * scale,
            descender: face.descender() as f32 * scale,
            line_gap: face.line_gap() as f32 * scale,
            curve_texels: Vec::new(),
            band_texels: Vec::new(),
            kerning: HashMap::new()
        };

        for c in chars {
            let Some(id) = face.glyph_index(c) else { continue };
            let advance = face.glyph_hor_advance(id).unwrap_or(0) as f32 * scale;
            let mut outline = OutlineCollector { scale, contours: Vec::new(), start: [0.0; 2], current: [0.0; 2] };
            face.outline_glyph(id, &mut outline);
            let curves: Vec<Curve> = outline.contours.iter().flatten().copied().collect();
            let data = (!curves.is_empty()).then(|| font.add_glyph(&outline.contours, &curves));
            font.glyphs.insert(c, SlugGlyph { advance, data, id });
        }

        // pair kerning from the legacy kern table, fonts with only gpos kerning get none
        if let Some(kern) = face.tables().kern {
            let ids: Vec<ttf_parser::GlyphId> = font.glyphs.values().map(|g| g.id).collect();
            for subtable in kern.subtables.into_iter().filter(|s| s.horizontal && !s.variable) {
                for &left in &ids {
                    for &right in &ids {
                        if let Some(k) = subtable.glyphs_kerning(left, right) {
                            font.kerning.entry((left.0, right.0)).or_insert(k as f32 * scale);
                        }
                    }
                }
            }
        }

        Ok(font)
    }

    fn add_glyph(&mut self, contours: &[Vec<Curve>], curves: &[Curve]) -> SlugGlyphData {
        let min = [curves.iter().map(|c| c.min(0)).fold(f32::MAX, f32::min),
            curves.iter().map(|c| c.min(1)).fold(f32::MAX, f32::min)];
        let max = [curves.iter().map(|c| c.max(0)).fold(f32::MIN, f32::max),
            curves.iter().map(|c| c.max(1)).fold(f32::MIN, f32::max)];

        // horizontal bands are stacked along y and vertical bands along x
        let nh = choose_band_count(curves, 1, min[1], max[1]);
        let nv = choose_band_count(curves, 0, min[0], max[0]);
        let hbands = build_bands(curves, 1, nh, min[1], max[1]);
        let vbands = build_bands(curves, 0, nv, min[0], max[0]);

        let curve_locations = pack_curves(contours, &mut self.curve_texels);
        let location = pack_bands(&hbands, &vbands, &curve_locations, &mut self.band_texels);

        let band_scale = |n: usize, lo: f32, hi: f32| if hi > lo { n as f32 / (hi - lo) } else { 0.0 };
        let sx = band_scale(nv, min[0], max[0]);
        let sy = band_scale(nh, min[1], max[1]);
        SlugGlyphData {
            min,
            max,
            location,
            band_max: [nv as u32 - 1, nh as u32 - 1],
            band_transform: [sx, sy, -min[0] * sx, -min[1] * sy]
        }
    }

    /// Height of a line of text in em
    pub fn line_height(&self) -> f32 {
        self.ascender - self.descender + self.line_gap
    }

    /// Rows in the curve and band textures
    pub fn texture_rows(&self) -> (usize, usize) {
        (self.curve_texels.len().div_ceil(TEXTURE_WIDTH).max(1), self.band_texels.len().div_ceil(TEXTURE_WIDTH).max(1))
    }

    /// Creates the curve (RGBA16f) and band (RG16u) textures
    pub fn create_textures<D: gfx::Device>(&self, device: &mut D) -> Result<(D::Texture, D::Texture), crate::Error> {
        let (curve_rows, band_rows) = self.texture_rows();
        let info = |format, rows: usize| gfx::TextureInfo {
            tex_type: gfx::TextureType::Texture2D,
            format,
            width: TEXTURE_WIDTH as u64,
            height: rows as u64,
            depth: 1,
            array_layers: 1,
            mip_levels: 1,
            samples: 1,
            usage: gfx::TextureUsage::SHADER_RESOURCE,
            initial_state: gfx::ResourceState::ShaderResource
        };
        let mut curves = self.curve_texels.clone();
        curves.resize(curve_rows * TEXTURE_WIDTH, [0; 4]);
        let mut bands = self.band_texels.clone();
        bands.resize(band_rows * TEXTURE_WIDTH, [0; 2]);
        let curve_texture = device.create_texture(&info(gfx::Format::RGBA16f, curve_rows), Some(curves.as_slice()))?;
        let band_texture = device.create_texture(&info(gfx::Format::RG16u, band_rows), Some(bands.as_slice()))?;
        Ok((curve_texture, band_texture))
    }

    /// Appends a quad per glyph for `text` to `vertices` and `indices`. `origin` is the left of the first line's
    /// baseline in object space with y up, `size` is the em size in object space units and `colour` is
    /// premultiplied. Returns the pen position after the text
    pub fn layout(&self, text: &str, origin: [f32; 2], size: f32, colour: [f32; 4], vertices: &mut Vec<SlugVertex>,
        indices: &mut Vec<u32>) -> [f32; 2] {
        let mut pen = origin;
        let mut prev: Option<ttf_parser::GlyphId> = None;
        for c in text.chars() {
            if c == '\n' {
                pen = [origin[0], pen[1] - self.line_height() * size];
                prev = None;
                continue;
            }
            let Some(glyph) = self.glyphs.get(&c) else { continue };
            if let Some(prev) = prev {
                pen[0] += self.kerning.get(&(prev.0, glyph.id.0)).copied().unwrap_or(0.0) * size;
            }
            if let Some(data) = &glyph.data {
                self.push_quad(data, pen, size, colour, vertices, indices);
            }
            pen[0] += glyph.advance * size;
            prev = Some(glyph.id);
        }
        pen
    }

    fn push_quad(&self, data: &SlugGlyphData, pen: [f32; 2], size: f32, colour: [f32; 4],
        vertices: &mut Vec<SlugVertex>, indices: &mut Vec<u32>) {
        let glyph_loc = f32::from_bits(data.location[0] as u32 | (data.location[1] as u32) << 16);
        let band_max = f32::from_bits(data.band_max[0] | data.band_max[1] << 16);
        let base = vertices.len() as u32;
        // corners with normals (+/-1, +/-1) so the vertex shader's dilation moves each edge out by the same distance
        for (corner, normal) in [([0, 0], [-1.0, -1.0]), ([1, 0], [1.0, -1.0]), ([1, 1], [1.0, 1.0]), ([0, 1], [-1.0, 1.0])] {
            let em = [
                if corner[0] == 0 { data.min[0] } else { data.max[0] },
                if corner[1] == 0 { data.min[1] } else { data.max[1] }
            ];
            vertices.push(SlugVertex {
                pos: [pen[0] + em[0] * size, pen[1] + em[1] * size, normal[0], normal[1]],
                tex: [em[0], em[1], glyph_loc, band_max],
                jac: [1.0 / size, 0.0, 0.0, 1.0 / size],
                bnd: data.band_transform,
                col: colour
            });
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(a: [f32; 2], b: [f32; 2]) -> Curve {
        Curve { p: [a, b, b] }
    }

    /// An outer square with a curved bump on top, and a square hole
    fn test_contours() -> Vec<Vec<Curve>> {
        vec![
            vec![
                line([0.0, 0.0], [1.0, 0.0]),
                line([1.0, 0.0], [1.0, 1.0]),
                Curve { p: [[1.0, 1.0], [0.5, 1.5], [0.0, 1.0]] },
                line([0.0, 1.0], [0.0, 0.0])
            ],
            vec![
                line([0.25, 0.25], [0.25, 0.75]),
                line([0.25, 0.75], [0.75, 0.75]),
                line([0.75, 0.75], [0.75, 0.25]),
                line([0.75, 0.25], [0.25, 0.25])
            ]
        ]
    }

    #[test]
    fn half_floats() {
        assert_eq!(f32_to_f16(0.0), 0x0000);
        assert_eq!(f32_to_f16(1.0), 0x3c00);
        assert_eq!(f32_to_f16(0.5), 0x3800);
        assert_eq!(f32_to_f16(-2.0), 0xc000);
        assert_eq!(f32_to_f16(65504.0), 0x7bff);
        assert_eq!(f32_to_f16(1.0e6), 0x7c00);
        assert_eq!(f32_to_f16(5.960464e-8), 0x0001);
        assert_eq!(f32_to_f16(0.333333), 0x3555);
    }

    #[test]
    fn bands_hold_every_crossing_curve_sorted() {
        let curves: Vec<Curve> = test_contours().into_iter().flatten().collect();
        let (min, max) = ([0.0, 0.0], [1.0, 1.25]);
        for axis in 0..2 {
            let n = choose_band_count(&curves, axis, min[axis], max[axis]);
            let bands = build_bands(&curves, axis, n, min[axis], max[axis]);
            let band_size = (max[axis] - min[axis]) / n as f32;
            for (b, list) in bands.iter().enumerate() {
                let lo = min[axis] + b as f32 * band_size;
                let hi = lo + band_size;
                for (i, curve) in curves.iter().enumerate() {
                    let crosses = curve.max(axis) >= lo && curve.min(axis) <= hi;
                    if crosses && !curve.is_flat(axis) {
                        assert!(list.contains(&i), "axis {axis} band {b} is missing curve {i}");
                    }
                    if curve.is_flat(axis) {
                        assert!(!list.contains(&i), "axis {axis} band {b} has flat curve {i}");
                    }
                }
                let ray_axis = 1 - axis;
                assert!(list.windows(2).all(|w| curves[w[0]].max(ray_axis) >= curves[w[1]].max(ray_axis)));
            }
        }
    }

    #[test]
    fn curves_share_end_points_and_stay_on_one_row() {
        let contours = test_contours();
        let mut texels = vec![[0u16; 4]; TEXTURE_WIDTH - 3];
        let locations = pack_curves(&contours, &mut texels);
        let curves: Vec<Curve> = contours.iter().flatten().copied().collect();
        for (curve, loc) in curves.iter().zip(&locations) {
            assert!((loc[0] as usize) < TEXTURE_WIDTH - 1, "curve straddles a row");
            let index = loc[1] as usize * TEXTURE_WIDTH + loc[0] as usize;
            assert_eq!(texels[index][0..2], [f32_to_f16(curve.p[0][0]), f32_to_f16(curve.p[0][1])]);
            assert_eq!(texels[index][2..4], [f32_to_f16(curve.p[1][0]), f32_to_f16(curve.p[1][1])]);
            assert_eq!(texels[index + 1][0..2], [f32_to_f16(curve.p[2][0]), f32_to_f16(curve.p[2][1])]);
        }
    }

    #[test]
    fn band_headers_point_at_lists() {
        let contours = test_contours();
        let curves: Vec<Curve> = contours.iter().flatten().copied().collect();
        let mut curve_texels = Vec::new();
        let locations = pack_curves(&contours, &mut curve_texels);
        let hbands = build_bands(&curves, 1, 3, 0.0, 1.25);
        let vbands = build_bands(&curves, 0, 2, 0.0, 1.0);
        let mut texels = vec![[0u16; 2]; TEXTURE_WIDTH - 2];
        let glyph = pack_bands(&hbands, &vbands, &locations, &mut texels);
        let glyph_index = glyph[1] as usize * TEXTURE_WIDTH + glyph[0] as usize;
        assert_eq!(glyph[0] as usize + hbands.len() + vbands.len() <= TEXTURE_WIDTH, true);
        for (i, band) in hbands.iter().chain(vbands.iter()).enumerate() {
            let [count, offset] = texels[glyph_index + i];
            assert_eq!(count as usize, band.len());
            for (j, &curve) in band.iter().enumerate() {
                assert_eq!(texels[glyph_index + offset as usize + j], locations[curve]);
            }
        }
    }

    #[test]
    fn compiles_a_font() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/hotline-data/src/fonts/roboto_medium.ttf");
        let Ok(data) = std::fs::read(path) else { return };
        let font = SlugFont::from_ttf(&data, (' '..='~').chain(['é'])).unwrap();
        assert_eq!(font.glyphs.len(), 96);
        assert!(font.glyphs[&' '].data.is_none());
        let o = font.glyphs[&'o'].data.unwrap();
        assert!(o.max[1] > o.min[1] && o.max[0] > o.min[0]);
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        font.layout("Hello\nworld", [0.0, 0.0], 32.0, [1.0; 4], &mut vertices, &mut indices);
        assert_eq!(vertices.len(), 40);
        assert_eq!(indices.len(), 60);
    }
}
