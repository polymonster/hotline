///
/// glTF 2.0 model loading. `load_gltf` reads a `.glb` or `.gltf` into `ModelData`, plain structure of arrays with no
/// gpu dependency: a node hierarchy sorted so parents come before children, submeshes of `Vertex3D` and `u32`
/// indices, and pbr metallic roughness materials. Everything links by `u32` index, `NONE` means no link.
/// `Model::create` then uploads submeshes into `pmfx::Mesh` vertex / index buffers and images into textures, and
/// packs materials into `PbrMaterialData` with bindless srv indices ready for a structured buffer
///

use crate::gfx;
use crate::pmfx;
use crate::image;
use crate::primitives::{self, Vertex3D};
use crate::Error;
use gfx::Texture;

use maths_rs::prelude::*;
use std::path::{Path, PathBuf};
use std::collections::VecDeque;

/// Index value for no link (no parent, no material, no texture)
pub const NONE: u32 = u32::MAX;

/// Options for loading glTF files
#[derive(Clone, Copy, Debug)]
pub struct GltfLoadInfo {
    /// Converts glTF's right handed y-up space into left handed y-up by negating z. Positions, normals, tangents and
    /// node transforms are mirrored. Indices are kept as they are, the mirror itself turns glTF's counter clockwise
    /// front faces clockwise
    pub left_handed: bool,
}

impl Default for GltfLoadInfo {
    fn default() -> Self {
        Self {
            left_handed: true
        }
    }
}

/// How a material's alpha is used, `Mask` holds the alpha cutoff
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AlphaMode {
    Opaque,
    Mask(f32),
    Blend
}

/// Node hierarchy, sorted so a node's parent always has a lower index than the node itself, which means world
/// matrices can be updated in a single forward pass. Local transforms are kept as separate translation, rotation and
/// scale so they can be animated
#[derive(Clone, Default)]
pub struct ModelNodes {
    pub name: Vec<String>,
    /// Index of the parent node, `NONE` for roots
    pub parent: Vec<u32>,
    pub translation: Vec<Vec3f>,
    pub rotation: Vec<Quatf>,
    pub scale: Vec<Vec3f>,
    /// Local matrix from translation, rotation and scale, see `update_local_matrices`
    pub local_matrix: Vec<Mat34f>,
    /// Local matrix multiplied by parent world matrices, see `update_world_matrices`
    pub world_matrix: Vec<Mat34f>,
    /// First submesh drawn by this node
    pub submesh_first: Vec<u32>,
    /// Number of submeshes drawn by this node, 0 for nodes with no mesh
    pub submesh_count: Vec<u32>,
}

impl ModelNodes {
    pub fn len(&self) -> usize {
        self.parent.len()
    }

    pub fn is_empty(&self) -> bool {
        self.parent.is_empty()
    }

    /// Returns the index of the first node called `name`
    pub fn find(&self, name: &str) -> Option<usize> {
        self.name.iter().position(|n| n == name)
    }

    /// Rebuilds `local_matrix` from `translation`, `rotation` and `scale`
    pub fn update_local_matrices(&mut self) {
        for i in 0..self.len() {
            self.local_matrix[i] = Mat34f::from_translation(self.translation[i]) *
                Mat34f::from(self.rotation[i]) *
                Mat34f::from_scale(self.scale[i]);
        }
    }

    /// Rebuilds `world_matrix` by transforming each node's `local_matrix` by its parent's world matrix
    pub fn update_world_matrices(&mut self) {
        for i in 0..self.len() {
            let parent = self.parent[i];
            self.world_matrix[i] = if parent == NONE {
                self.local_matrix[i]
            }
            else {
                self.world_matrix[parent as usize] * self.local_matrix[i]
            };
        }
    }
}

/// Submeshes, one per glTF mesh primitive. Nodes reference a range of these and nodes sharing a glTF mesh share the
/// same range
#[derive(Clone, Default)]
pub struct ModelSubmeshes {
    pub vertices: Vec<Vec<Vertex3D>>,
    /// Triangle list indices in glTF order, front faces are clockwise once mirrored with `left_handed`
    pub indices: Vec<Vec<u32>>,
    /// Index into the materials, `NONE` to use a default material
    pub material: Vec<u32>,
    pub aabb_min: Vec<Vec3f>,
    pub aabb_max: Vec<Vec3f>,
}

impl ModelSubmeshes {
    pub fn len(&self) -> usize {
        self.vertices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }
}

/// Pbr metallic roughness materials. Texture slots index into `ModelData::images`, `NONE` when absent so only the
/// factor applies
#[derive(Clone, Default)]
pub struct ModelMaterials {
    pub name: Vec<String>,
    pub base_colour_factor: Vec<Vec4f>,
    pub emissive_factor: Vec<Vec3f>,
    pub metallic_factor: Vec<f32>,
    pub roughness_factor: Vec<f32>,
    pub alpha_mode: Vec<AlphaMode>,
    pub double_sided: Vec<bool>,
    pub base_colour_image: Vec<u32>,
    pub normal_image: Vec<u32>,
    pub metallic_roughness_image: Vec<u32>,
    pub occlusion_image: Vec<u32>,
    pub emissive_image: Vec<u32>,
}

impl ModelMaterials {
    pub fn len(&self) -> usize {
        self.name.len()
    }

    pub fn is_empty(&self) -> bool {
        self.name.is_empty()
    }

    /// Returns true if `image` is sampled as colour (base colour or emissive) by any material and wants an srgb format
    pub fn is_colour_image(&self, image: u32) -> bool {
        self.base_colour_image.contains(&image) || self.emissive_image.contains(&image)
    }
}

/// Where an image's encoded file data lives
#[derive(Clone)]
pub enum ImageSource {
    /// External file, resolved relative to the glTF file
    File(PathBuf),
    /// Encoded file data embedded in a glTF buffer (png, jpg, etc)
    Embedded(Vec<u8>),
}

/// Cpu side model data loaded from glTF, see `load_gltf`
#[derive(Clone, Default)]
pub struct ModelData {
    pub nodes: ModelNodes,
    pub submeshes: ModelSubmeshes,
    pub materials: ModelMaterials,
    pub images: Vec<ImageSource>,
}

/// Loads a `.glb` or a `.gltf` with external `.bin` and image files. Base64 data uris are not supported
pub fn load_gltf(path: &str, info: &GltfLoadInfo) -> Result<ModelData, Error> {
    let bytes = std::fs::read(path).map_err(|err| Error {
        msg: format!("hotline_rs::model:: failed to read {}: {}", path, err)
    })?;
    let base_dir = Path::new(path).parent().unwrap_or(Path::new(""));
    load_gltf_from_memory(&bytes, base_dir, info).map_err(|err| Error {
        msg: format!("{}: {}", err.msg, path)
    })
}

/// Loads a `.glb` or `.gltf` from memory, external files are resolved relative to `base_dir`
pub fn load_gltf_from_memory(bytes: &[u8], base_dir: &Path, info: &GltfLoadInfo) -> Result<ModelData, Error> {
    let gltf = gltf::Gltf::from_slice(bytes)?;
    let document = &gltf.document;
    let mut blob = gltf.blob.clone();

    // buffer data, the glb binary chunk or external files
    let mut buffers = Vec::new();
    for buffer in document.buffers() {
        buffers.push(match buffer.source() {
            gltf::buffer::Source::Bin => blob.take().ok_or_else(|| Error {
                msg: "hotline_rs::model:: glb binary chunk is missing".to_string()
            })?,
            gltf::buffer::Source::Uri(uri) => read_uri(base_dir, uri)?
        });
    }

    let mut model = ModelData::default();

    // images
    for image in document.images() {
        model.images.push(match image.source() {
            gltf::image::Source::View { view, .. } => {
                let start = view.offset();
                ImageSource::Embedded(buffers[view.buffer().index()][start..start + view.length()].to_vec())
            }
            gltf::image::Source::Uri { uri, .. } => {
                check_uri(uri)?;
                ImageSource::File(base_dir.join(uri))
            }
        });
    }

    // materials
    let image_index = |info: Option<gltf::texture::Texture>| info.map_or(NONE, |t| t.source().index() as u32);
    for material in document.materials() {
        let pbr = material.pbr_metallic_roughness();
        let mats = &mut model.materials;
        mats.name.push(material.name().unwrap_or("").to_string());
        mats.base_colour_factor.push(v4(pbr.base_color_factor()));
        mats.emissive_factor.push(v3(material.emissive_factor()));
        mats.metallic_factor.push(pbr.metallic_factor());
        mats.roughness_factor.push(pbr.roughness_factor());
        mats.alpha_mode.push(match material.alpha_mode() {
            gltf::material::AlphaMode::Opaque => AlphaMode::Opaque,
            gltf::material::AlphaMode::Mask => AlphaMode::Mask(material.alpha_cutoff().unwrap_or(0.5)),
            gltf::material::AlphaMode::Blend => AlphaMode::Blend,
        });
        mats.double_sided.push(material.double_sided());
        mats.base_colour_image.push(image_index(pbr.base_color_texture().map(|t| t.texture())));
        mats.metallic_roughness_image.push(image_index(pbr.metallic_roughness_texture().map(|t| t.texture())));
        mats.normal_image.push(image_index(material.normal_texture().map(|t| t.texture())));
        mats.occlusion_image.push(image_index(material.occlusion_texture().map(|t| t.texture())));
        mats.emissive_image.push(image_index(material.emissive_texture().map(|t| t.texture())));
    }

    // submeshes, each glTF mesh becomes a contiguous range
    let mut mesh_ranges = Vec::new();
    for mesh in document.meshes() {
        let first = model.submeshes.len() as u32;
        for primitive in mesh.primitives() {
            if primitive.mode() != gltf::mesh::Mode::Triangles {
                println!("hotline_rs::model:: skipping non triangle primitive in mesh {}", mesh.name().unwrap_or(""));
                continue;
            }
            let reader = primitive.reader(|buffer| buffers.get(buffer.index()).map(|b| b.as_slice()));
            let positions: Vec<Vec3f> = match reader.read_positions() {
                Some(positions) => positions.map(v3).collect(),
                None => continue
            };
            let indices: Vec<u32> = match reader.read_indices() {
                Some(indices) => indices.into_u32().collect(),
                None => (0..positions.len() as u32).collect()
            };
            let normals: Vec<Vec3f> = match reader.read_normals() {
                Some(normals) => normals.map(v3).collect(),
                None => generate_normals(&positions, &indices)
            };
            let texcoords: Vec<Vec2f> = match reader.read_tex_coords(0) {
                Some(texcoords) => texcoords.into_f32().map(v2).collect(),
                None => vec![Vec2f::zero(); positions.len()]
            };
            let tangents: Option<Vec<[f32; 4]>> = reader.read_tangents().map(|t| t.collect());

            let mut vertices: Vec<Vertex3D> = (0..positions.len()).map(|i| {
                let normal = normals[i];
                let (tangent, bitangent) = match &tangents {
                    // bitangent is built in glTF space so it mirrors with the rest of the frame
                    Some(t) => {
                        let tangent = vec3f(t[i][0], t[i][1], t[i][2]);
                        (tangent, cross(normal, tangent) * t[i][3])
                    }
                    None => (Vec3f::zero(), Vec3f::zero())
                };
                Vertex3D {
                    position: positions[i],
                    texcoord: texcoords[i],
                    normal,
                    tangent,
                    bitangent
                }
            }).collect();

            if info.left_handed {
                for v in &mut vertices {
                    v.position.z = -v.position.z;
                    v.normal.z = -v.normal.z;
                    v.tangent.z = -v.tangent.z;
                    v.bitangent.z = -v.bitangent.z;
                }
            }

            if tangents.is_none() {
                generate_tangents(&mut vertices, &indices);
            }

            let (aabb_min, aabb_max) = vertices.iter()
                .fold((Vec3f::max_value(), -Vec3f::max_value()), |acc, v| min_max(v.position, acc));

            model.submeshes.vertices.push(vertices);
            model.submeshes.indices.push(indices);
            model.submeshes.material.push(primitive.material().index().map_or(NONE, |i| i as u32));
            model.submeshes.aabb_min.push(aabb_min);
            model.submeshes.aabb_max.push(aabb_max);
        }
        mesh_ranges.push((first, model.submeshes.len() as u32 - first));
    }

    // nodes, breadth first from the scene roots so parents are always visited before their children
    let scene = document.default_scene().or_else(|| document.scenes().next()).ok_or_else(|| Error {
        msg: "hotline_rs::model:: gltf has no scenes".to_string()
    })?;
    let mut queue: VecDeque<(gltf::Node, u32)> = scene.nodes().map(|node| (node, NONE)).collect();
    while let Some((node, parent)) = queue.pop_front() {
        let index = model.nodes.len() as u32;
        let (t, r, s) = node.transform().decomposed();
        let (translation, rotation) = if info.left_handed {
            // mirror in z: S * M * S with S = diag(1, 1, -1)
            (vec3f(t[0], t[1], -t[2]), quatf(-r[0], -r[1], r[2], r[3]))
        }
        else {
            (v3(t), quatf(r[0], r[1], r[2], r[3]))
        };
        let (submesh_first, submesh_count) = node.mesh().map_or((0, 0), |mesh| mesh_ranges[mesh.index()]);

        let nodes = &mut model.nodes;
        nodes.name.push(node.name().unwrap_or("").to_string());
        nodes.parent.push(parent);
        nodes.translation.push(translation);
        nodes.rotation.push(rotation);
        nodes.scale.push(v3(s));
        nodes.local_matrix.push(Mat34f::identity());
        nodes.world_matrix.push(Mat34f::identity());
        nodes.submesh_first.push(submesh_first);
        nodes.submesh_count.push(submesh_count);

        queue.extend(node.children().map(|child| (child, index)));
    }
    model.nodes.update_local_matrices();
    model.nodes.update_world_matrices();

    Ok(model)
}

fn v2(a: [f32; 2]) -> Vec2f {
    vec2f(a[0], a[1])
}

fn v3(a: [f32; 3]) -> Vec3f {
    vec3f(a[0], a[1], a[2])
}

fn v4(a: [f32; 4]) -> Vec4f {
    vec4f(a[0], a[1], a[2], a[3])
}

/// Quaternion from components, maths-rs only exposes euler, axis angle and matrix constructors
fn quatf(x: f32, y: f32, z: f32, w: f32) -> Quatf {
    let mut q = Quatf::identity();
    q.as_mut_slice().copy_from_slice(&[x, y, z, w]);
    q
}

fn check_uri(uri: &str) -> Result<(), Error> {
    if uri.starts_with("data:") {
        Err(Error {
            msg: "hotline_rs::model:: base64 data uris are not supported, export as .glb".to_string()
        })
    }
    else {
        Ok(())
    }
}

fn read_uri(base_dir: &Path, uri: &str) -> Result<Vec<u8>, Error> {
    check_uri(uri)?;
    let path = base_dir.join(uri);
    std::fs::read(&path).map_err(|err| Error {
        msg: format!("hotline_rs::model:: failed to read {}: {}", path.display(), err)
    })
}

/// Smooth normals from area weighted face normals, for primitives exported without normals. Expects glTF counter
/// clockwise winding
fn generate_normals(positions: &[Vec3f], indices: &[u32]) -> Vec<Vec3f> {
    let mut normals = vec![Vec3f::zero(); positions.len()];
    for tri in indices.chunks_exact(3) {
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
        let n = cross(positions[i1] - positions[i0], positions[i2] - positions[i0]);
        normals[i0] += n;
        normals[i1] += n;
        normals[i2] += n;
    }
    normals.into_iter().map(|n| if mag2(n) > 0.0 { normalize(n) } else { Vec3f::unit_y() }).collect()
}

/// Tangents and bitangents accumulated from uv derivatives per triangle, then orthonormalised against the normal.
/// The result doesn't depend on winding, so this works on either handedness
fn generate_tangents(vertices: &mut [Vertex3D], indices: &[u32]) {
    let mut tangents = vec![Vec3f::zero(); vertices.len()];
    let mut bitangents = vec![Vec3f::zero(); vertices.len()];
    for tri in indices.chunks_exact(3) {
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
        let e1 = vertices[i1].position - vertices[i0].position;
        let e2 = vertices[i2].position - vertices[i0].position;
        let d1 = vertices[i1].texcoord - vertices[i0].texcoord;
        let d2 = vertices[i2].texcoord - vertices[i0].texcoord;
        let det = d1.x * d2.y - d2.x * d1.y;
        if det.abs() < 1e-12 {
            continue;
        }
        let r = 1.0 / det;
        let t = (e1 * d2.y - e2 * d1.y) * r;
        let b = (e2 * d1.x - e1 * d2.x) * r;
        for i in [i0, i1, i2] {
            tangents[i] += t;
            bitangents[i] += b;
        }
    }

    for (i, v) in vertices.iter_mut().enumerate() {
        let n = v.normal;
        // gram-schmidt, falling back to any perpendicular axis when the uvs are degenerate
        let mut t = tangents[i] - n * dot(n, tangents[i]);
        if mag2(t) < 1e-12 {
            t = cross(n, if n.y.abs() < 0.99 { Vec3f::unit_y() } else { Vec3f::unit_x() });
        }
        let t = normalize(t);
        let b = cross(n, t);
        let b = if dot(b, bitangents[i]) < 0.0 { -b } else { b };
        v.tangent = t;
        v.bitangent = b;
    }
}

/// Material flags packed into `PbrMaterialData::flags`
pub mod material_flags {
    pub const ALPHA_MASK: u32 = 1 << 0;
    pub const ALPHA_BLEND: u32 = 1 << 1;
    pub const DOUBLE_SIDED: u32 = 1 << 2;
}

/// Gpu friendly pbr material for a structured buffer, texture ids are bindless srv indices or `NONE` for no texture
#[repr(C)]
#[derive(Clone, Debug)]
pub struct PbrMaterialData {
    pub base_colour_factor: Vec4f,
    pub emissive_factor: Vec3f,
    pub metallic_factor: f32,
    pub roughness_factor: f32,
    pub alpha_cutoff: f32,
    /// See `material_flags`
    pub flags: u32,
    pub padding0: u32,
    pub base_colour_id: u32,
    pub normal_id: u32,
    pub metallic_roughness_id: u32,
    pub occlusion_id: u32,
    pub emissive_id: u32,
    pub padding1: [u32; 3],
}

impl Default for PbrMaterialData {
    /// The glTF default material
    fn default() -> Self {
        Self {
            base_colour_factor: Vec4f::one(),
            emissive_factor: Vec3f::zero(),
            metallic_factor: 1.0,
            roughness_factor: 1.0,
            alpha_cutoff: 0.5,
            flags: 0,
            padding0: 0,
            base_colour_id: NONE,
            normal_id: NONE,
            metallic_roughness_id: NONE,
            occlusion_id: NONE,
            emissive_id: NONE,
            padding1: [0; 3],
        }
    }
}

/// A model ready to render: node hierarchy, a vertex / index buffer per submesh, textures and gpu materials
pub struct Model<D: gfx::Device> {
    pub nodes: ModelNodes,
    /// One mesh per submesh, in the same order as `ModelSubmeshes`
    pub meshes: Vec<pmfx::Mesh<D>>,
    /// Index into `materials` per submesh, always valid. Submeshes with no material use the default at the end
    pub submesh_material: Vec<u32>,
    /// One texture per image in `ModelData::images`
    pub textures: Vec<D::Texture>,
    /// One per material in `ModelData::materials`, followed by the glTF default material
    pub materials: Vec<PbrMaterialData>,
}

impl<D: gfx::Device> Model<D> {
    /// Creates gpu resources for `data`. Textures get shader resource views in `heap`, or the device heap if `None`,
    /// and their srv indices are written into `materials`
    pub fn create(device: &mut D, data: &ModelData, mut heap: Option<&mut D::Heap>) -> Result<Self, Error> {
        let meshes = (0..data.submeshes.len())
            .map(|i| primitives::try_create_mesh_3d(device, &data.submeshes.vertices[i], &data.submeshes.indices[i]))
            .collect::<Result<Vec<_>, _>>()?;

        let mut textures = Vec::new();
        for (i, source) in data.images.iter().enumerate() {
            let mut image = match source {
                ImageSource::File(path) => image::load_from_file(&path.to_string_lossy())?,
                ImageSource::Embedded(bytes) => image::load_from_memory(bytes)?
            };
            if image.info.format == gfx::Format::RGBA8n && data.materials.is_colour_image(i as u32) {
                image.info.format = gfx::Format::RGBA8nSRGB;
            }
            textures.push(device.create_texture_with_heaps(
                &image.info,
                gfx::TextureHeapInfo {
                    shader: heap.as_deref_mut(),
                    ..Default::default()
                },
                crate::data![image.data.as_slice()]
            )?);
        }

        let srv = |image: u32| -> u32 {
            if image == NONE {
                NONE
            }
            else {
                textures[image as usize].get_srv_index().map_or(NONE, |i| i as u32)
            }
        };

        let mats = &data.materials;
        let mut materials = Vec::new();
        for i in 0..mats.len() {
            let (alpha_cutoff, mut flags) = match mats.alpha_mode[i] {
                AlphaMode::Opaque => (0.5, 0),
                AlphaMode::Mask(cutoff) => (cutoff, material_flags::ALPHA_MASK),
                AlphaMode::Blend => (0.5, material_flags::ALPHA_BLEND),
            };
            if mats.double_sided[i] {
                flags |= material_flags::DOUBLE_SIDED;
            }
            materials.push(PbrMaterialData {
                base_colour_factor: mats.base_colour_factor[i],
                emissive_factor: mats.emissive_factor[i],
                metallic_factor: mats.metallic_factor[i],
                roughness_factor: mats.roughness_factor[i],
                alpha_cutoff,
                flags,
                base_colour_id: srv(mats.base_colour_image[i]),
                normal_id: srv(mats.normal_image[i]),
                metallic_roughness_id: srv(mats.metallic_roughness_image[i]),
                occlusion_id: srv(mats.occlusion_image[i]),
                emissive_id: srv(mats.emissive_image[i]),
                ..Default::default()
            });
        }
        let default_material = materials.len() as u32;
        materials.push(PbrMaterialData::default());

        let submesh_material = data.submeshes.material.iter()
            .map(|m| if *m == NONE { default_material } else { *m })
            .collect();

        Ok(Model {
            nodes: data.nodes.clone(),
            meshes,
            submesh_material,
            textures,
            materials,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Packs json and binary chunks into a glb
    fn make_glb(json: &str, bin: &[u8]) -> Vec<u8> {
        let mut json = json.as_bytes().to_vec();
        while json.len() % 4 != 0 {
            json.push(b' ');
        }
        let mut bin = bin.to_vec();
        while bin.len() % 4 != 0 {
            bin.push(0);
        }
        let total = 12 + 8 + json.len() + 8 + bin.len();
        let mut glb = Vec::new();
        glb.extend_from_slice(b"glTF");
        glb.extend_from_slice(&2u32.to_le_bytes());
        glb.extend_from_slice(&(total as u32).to_le_bytes());
        glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
        glb.extend_from_slice(b"JSON");
        glb.extend_from_slice(&json);
        glb.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        glb.extend_from_slice(b"BIN\0");
        glb.extend_from_slice(&bin);
        glb
    }

    /// One triangle under a 3 level hierarchy declared child first: c is a child of b, b a child of a. a is rotated 90
    /// degrees about y, b and c are translated along z
    fn test_glb() -> Vec<u8> {
        let json = r#"{
            "asset": {"version": "2.0"},
            "scene": 0,
            "scenes": [{"nodes": [2]}],
            "nodes": [
                {"name": "c", "translation": [0, 0, 1]},
                {"name": "b", "children": [0], "translation": [0, 0, 2], "mesh": 0},
                {"name": "a", "children": [1], "rotation": [0, 0.70710677, 0, 0.70710677]}
            ],
            "meshes": [{"primitives": [{"attributes": {"POSITION": 0}, "indices": 1}]}],
            "buffers": [{"byteLength": 44}],
            "bufferViews": [
                {"buffer": 0, "byteOffset": 0, "byteLength": 36},
                {"buffer": 0, "byteOffset": 36, "byteLength": 6}
            ],
            "accessors": [
                {"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0, 0, 0], "max": [1, 1, 1]},
                {"bufferView": 1, "componentType": 5123, "count": 3, "type": "SCALAR"}
            ]
        }"#;
        let mut bin = Vec::new();
        for f in [0.0f32, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
            bin.extend_from_slice(&f.to_le_bytes());
        }
        for i in [0u16, 1, 2] {
            bin.extend_from_slice(&i.to_le_bytes());
        }
        make_glb(json, &bin)
    }

    fn approx(a: Vec3f, b: Vec3f) -> bool {
        mag(a - b) < 1e-5
    }

    #[test]
    fn hierarchy_is_parent_first() {
        let model = load_gltf_from_memory(&test_glb(), Path::new(""), &GltfLoadInfo::default()).unwrap();
        assert_eq!(model.nodes.name, vec!["a", "b", "c"]);
        assert_eq!(model.nodes.parent, vec![NONE, 0, 1]);
        for (i, parent) in model.nodes.parent.iter().enumerate() {
            assert!(*parent == NONE || (*parent as usize) < i);
        }
        assert_eq!(model.nodes.submesh_count, vec![0, 1, 0]);
    }

    #[test]
    fn world_matrices_match_gltf() {
        // in glTF space a's rotation takes +z to +x and +x to -z, so b sits at x = 2 and c at x = 3. z is negated
        // going left handed, which leaves those positions alone but turns +x into +z
        let model = load_gltf_from_memory(&test_glb(), Path::new(""), &GltfLoadInfo::default()).unwrap();
        let world = &model.nodes.world_matrix;
        assert!(approx(world[1] * Vec3f::zero(), vec3f(2.0, 0.0, 0.0)));
        assert!(approx(world[2] * Vec3f::zero(), vec3f(3.0, 0.0, 0.0)));
        assert!(approx(world[0] * Vec3f::unit_x(), vec3f(0.0, 0.0, 1.0)));

        let rh = load_gltf_from_memory(&test_glb(), Path::new(""), &GltfLoadInfo { left_handed: false }).unwrap();
        assert!(approx(rh.nodes.world_matrix[0] * Vec3f::unit_x(), vec3f(0.0, 0.0, -1.0)));
    }

    #[test]
    fn left_handed_mirrors_geometry() {
        let model = load_gltf_from_memory(&test_glb(), Path::new(""), &GltfLoadInfo::default()).unwrap();
        let vertices = &model.submeshes.vertices[0];
        assert!(approx(vertices[0].position, vec3f(0.0, 0.0, -1.0)));
        assert_eq!(model.submeshes.indices[0], vec![0, 1, 2]);
        // generated in glTF space from (1, 1, 1) then mirrored
        assert!(approx(vertices[0].normal, normalize(vec3f(1.0, 1.0, -1.0))));
        assert_eq!(model.submeshes.material[0], NONE);
        assert!(approx(model.submeshes.aabb_min[0], vec3f(0.0, 0.0, -1.0)));
        assert!(approx(model.submeshes.aabb_max[0], vec3f(1.0, 1.0, 0.0)));
    }
}
