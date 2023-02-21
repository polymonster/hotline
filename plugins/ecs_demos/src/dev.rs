use hotline_rs::gfx;
use hotline_rs::pmfx;

use maths_rs::*;
use maths_rs::vec::*;
use maths_rs::Vec2f;
use maths_rs::Vec3f;
use maths_rs::num::*;

/// Generic structure for 3D lit geometry meshes
#[derive(Clone)]
#[repr(C)]
pub struct Vertex3D {
    pub position: Vec3f,
    pub texcoord: Vec2f,
    pub normal: Vec3f,
    pub tangent: Vec3f,
    pub bitangent: Vec3f,
}

/// Generic structure for simple 2D textured meshes
#[derive(Clone)]
#[repr(C)]
pub struct Vertex2D {
    pub position: Vec2f,
    pub texcoord: Vec2f,
}

/// Inverse golden ratio
const INV_PHI : f32 = 0.61803398875;
const PHI : f32 = 1.618033988749;

/// Returns an orthonormal basis given the axis returning (right, up, at)
fn basis_from_axis(axis: Vec3f) -> (Vec3f, Vec3f, Vec3f) {
    // right
    let mut right = cross(axis, Vec3f::unit_y());
    if mag(right) < 0.1 {
        right = cross(axis, Vec3f::unit_z());
    }
    if mag(right) < 0.1 {
        right = cross(axis, Vec3f::unit_x());
    }
    right = normalize(right);
    
    // up + re-adjust right
    let up = normalize(cross(axis, right));
    right = normalize(cross(axis, up));
    
    // at
    let at = cross(right, up);

    (right, up, at)
}

/// Subdivides a single triangle vertex into 4 evenly distributed smaller triangles, adjusting uv's and maintaining normals and tangents
pub fn subdivide_triangle(t0: &Vertex3D, t1: &Vertex3D, t2: &Vertex3D, order: u32, max_order: u32) -> Vec<Vertex3D> {
    if order == max_order {
        vec![t0.clone(), t1.clone(), t2.clone()]
    }
    else {
        //  /\      /\
        // /__\ -> /\/\
        // 
        //      t1
        //    s0  s2
        //  t0  s1  t2

        let lerp_half = |a: &Vertex3D, b: &Vertex3D| -> Vertex3D {
            Vertex3D {
                position: a.position + (b.position - a.position) * 0.5,
                texcoord: a.texcoord + (b.texcoord - a.texcoord) * 0.5,
                tangent: a.tangent,
                normal: a.normal,
                bitangent: a.bitangent
            }
        };

        let s0 = lerp_half(t0, t1);
        let s1 = lerp_half(t0, t2);
        let s2 = lerp_half(t2, t1);

        let mut sub = subdivide_triangle(t0, &s0, &s1, order + 1, max_order);
        sub.extend(subdivide_triangle(&s0,  t1, &s2, order + 1, max_order));
        sub.extend(subdivide_triangle(&s1, &s0, &s2, order + 1, max_order));
        sub.extend(subdivide_triangle(&s1, &s2, &t2, order + 1, max_order));
        sub
    }
}

/// Utility to create faceted meshes with varying index sizes depending on the index requirements
fn create_mesh_3d<D: gfx::Device>(dev: &mut D, vertices: Vec<Vertex3D>, indices: Vec<usize>) -> pmfx::Mesh<D> {
    let max_index = vertices.len();
    let index_buffer = if max_index > 65535 {
        let mut indices32 : Vec<u32> = Vec::new();
        for i in &indices {
            indices32.push(*i as u32);
        }

        dev.create_buffer(&gfx::BufferInfo {
            usage: gfx::BufferUsage::Index,
            cpu_access: gfx::CpuAccessFlags::NONE,
            num_elements: indices32.len(),
            format: gfx::Format::R32u,
            stride: 4,
            },
            Some(indices32.as_slice())
        ).unwrap()
    }
    else {
        let mut indices16 : Vec<u16> = Vec::new();
        for i in &indices {
            indices16.push(*i as u16);
        }

        dev.create_buffer(&gfx::BufferInfo {
            usage: gfx::BufferUsage::Index,
            cpu_access: gfx::CpuAccessFlags::NONE,
            num_elements: indices16.len(),
            format: gfx::Format::R16u,
            stride: 2,
            },
            Some(indices16.as_slice())
        ).unwrap()
    };

    pmfx::Mesh {
        vb: dev.create_buffer(&gfx::BufferInfo {
                usage: gfx::BufferUsage::Vertex,
                cpu_access: gfx::CpuAccessFlags::NONE,
                num_elements: vertices.len(),
                format: gfx::Format::Unknown,
                stride: std::mem::size_of::<Vertex3D>() 
            }, 
            Some(vertices.as_slice())
        ).unwrap(),
        ib: index_buffer,
        num_indices: indices.len() as u32
    }
}

/// Utility to create a facent mesh which will have hard edged normals and automatically generate and index buffer from vertices
fn create_faceted_mesh_3d<D: gfx::Device>(dev: &mut D, vertices: Vec<Vertex3D>) -> pmfx::Mesh<D> {
    let mut indices = Vec::new();
    for i in 0..vertices.len() {
        indices.push(i);
    }
    create_mesh_3d(dev, vertices, indices)
}

/// Create a hemi-icosahedron in axis with subdivisions
pub fn hemi_icosohedron(axis: Vec3f, pos: Vec3f, start_angle: f32, subdivisions: u32) -> Vec<Vertex3D> {
    let (right, up, at) = basis_from_axis(axis);

    let tip = pos - at * INV_PHI;
    let dip = pos + at * 0.5 * 2.0;

    let angle_step = f32::pi() / 2.5;

    let mut a = start_angle;
    let mut vertices = Vec::new();

    for _ in 0..5 {
        let x = f32::sin(a);
        let y = f32::cos(a);
        let p = pos + right * x + up * y;

        a += angle_step;
        let x2 = f32::sin(a);
        let y2 = f32::cos(a);
        let np = pos + right * x2 + up * y2;

        let n = get_triangle_normal(p, np, tip);
        let b = normalize(p - tip);
        let t = cross(n, b);

        let tri = vec![
            Vertex3D {
                position: p,
                texcoord: Vec2f::new(-1.0, -1.0),
                normal: n,
                tangent: t,
                bitangent: b,
            },
            Vertex3D {
                position: tip,
                texcoord: Vec2f::new(0.0, 1.0),
                normal: n,
                tangent: t,
                bitangent: b,
            },
            Vertex3D {
                position: np,
                texcoord: Vec2f::new(1.0, -1.0),
                normal: n,
                tangent: t,
                bitangent: b,
            }
        ];

        vertices.extend(subdivide_triangle(&tri[0], &tri[1], &tri[2], 0, subdivisions));
        
        let side_dip = dip + cross(normalize(p-np), at);
        
        let n = get_triangle_normal(p, side_dip, np);
        let b = normalize(p - np);
        let t = cross(n, b);
        
        let tri = vec![
            Vertex3D {
                position: p,
                texcoord: Vec2f::new(-1.0, 1.0),
                normal: n,
                tangent: t,
                bitangent: b,
            },
            Vertex3D {
                position: np,
                texcoord: Vec2f::new(1.0, 1.0),
                normal: n,
                tangent: t,
                bitangent: b,
            },
            Vertex3D {
                position: side_dip,
                texcoord: Vec2f::new(0.0, -1.0),
                normal: n,
                tangent: t,
                bitangent: b,
            }
            
        ];
        vertices.extend(subdivide_triangle(&tri[0], &tri[1], &tri[2], 0, subdivisions));
    }
    vertices
}

/// Create an indexed smooth sphere with subdivided icosophere vertices and smooth normals
pub fn create_sphere_mesh<D: gfx::Device>(dev: &mut D, segments: usize) -> pmfx::Mesh<D> {
    let vertex_segments = segments + 2;

    let two_pi = f32::pi() * 2.0;
    let angle_step = two_pi / segments as f32;
    let height_step = 2.0 / (segments - 1) as f32;

    let mid = segments / 2;

    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut angle = 0.0;
    let mut y = -1.0;

    for r in 0..vertex_segments {
        angle = 0.0;
        for i in 0..vertex_segments {
            let x = f32::cos(angle);
            let z = -f32::sin(angle);

            angle += angle_step;

            let radius = 1.0 - abs(y);
            let xz = vec3f(x, 0.0, z) * radius;
            let p = vec3f(xz.x, y, xz.z);

            // tangent
            let x = f32::cos(angle);
            let z = -f32::sin(angle);
            let xz = vec3f(x, 0.0, z) * radius;

            let p_next = vec3f(xz.x, y, xz.z);
            let p_next = normalize(p_next);

            let p = normalize(p);
            let t = p_next - p;
            let bt = cross(p, t);

            let mut u = 0.5 + f32::atan2(z, x) / two_pi;
            let v = 0.5 + f32::asin(p.y) / f32::pi();

            // clamps the UV's at the end to avoid interpolation artifacts
            let u = if i == mid-1 { 0.0 } else { u };
            let u = if i == mid { 1.0 } else { u };

            if i == mid-1 {
                angle -= angle_step;
            }

            vertices.push(Vertex3D{
                position: p,
                normal: p,
                tangent: t,
                bitangent: bt,
                texcoord: vec2f(u, v) * 3.0
            });
        }

        y += height_step;
    }

    //
    // Indices
    //

    for r in 0..segments-1 {
        for i in 0..segments+1 {
            let i_next = i + 1;
            let v_index = r * vertex_segments;
            let v_next_index = (r + 1) * vertex_segments + i;
            let v_next_next_index = (r + 1) * vertex_segments + i_next;

            indices.extend(vec![
                v_index + i,
                v_next_index,
                v_index + i_next,
                v_next_index,
                v_next_next_index,
                v_index + i_next,
            ]);
        }
    }

    create_mesh_3d(dev, vertices, indices)
}

pub fn create_cylinder_mesh<D: gfx::Device>(dev: &mut D, segments: usize) -> pmfx::Mesh<D> {
    let two_pi = f32::pi() * 2.0;
    let axis = Vec3f::unit_y();
    let right = Vec3f::unit_x();
    let up = cross(axis, right);
    let right = cross(axis, up);

    let mut vertices = Vec::new();
    let mut points = Vec::new();
    let mut bottom_points = Vec::new();
    let mut top_points = Vec::new();
    let mut tangents = Vec::new();
    let mut indices = Vec::new();

    // add an extra segment at the end to make uv's wrap nicely
    let vertex_segments = segments + 1;

    // rotate around up axis and extract some data we can lookup to build vb and ib
    let mut angle = 0.0;
    let angle_step = two_pi / segments as f32;
    for i in 0..vertex_segments {
        // current
        let mut x = f32::cos(angle);
        let mut y = -f32::sin(angle);
        let v1 = right * x + up * y;

        // next
        angle += angle_step;
        x = f32::cos(angle);
        y = -f32::sin(angle);
        let v2 = right * x + up * y;

        points.push(v1);
        tangents.push(v2 - v1);
        bottom_points.push(points[i] - Vec3f::unit_y());
        top_points.push(points[i] + Vec3f::unit_y());
    }

    //
    // Vertices
    //

    let two_pi = 2.0 * f32::pi();

    // bottom ring
    for i in 0..vertex_segments {
        let u = 0.5 + f32::atan2(bottom_points[i].z, bottom_points[i].x) / two_pi;
        let u = if i == segments { 0.0 } else { u };
        let bt = cross(tangents[i], points[i]);
        vertices.push(Vertex3D{
            position: bottom_points[i],
            normal: points[i],
            tangent: tangents[i],
            bitangent: bt,
            texcoord: Vec2f::new(u * 3.0, 0.0)
        });
    }

    // top ring
    for i in 0..vertex_segments {
        let u = 0.5 + f32::atan2(top_points[i].z, top_points[i].x) / two_pi;
        let u = if i == segments { 0.0 } else { u };
        let bt = cross(tangents[i], points[i]);
        vertices.push(Vertex3D{
            position: top_points[i],
            normal: points[i],
            tangent: tangents[i],
            bitangent: bt,
            texcoord: Vec2f::new(u * 3.0, 1.0)
        });
    }
        
    // bottom face
    for i in 0..vertex_segments {
        vertices.push(Vertex3D{
            position: bottom_points[i],
            normal: -Vec3f::unit_y(),
            tangent: Vec3f::unit_x(),
            bitangent: Vec3f::unit_z(),
            texcoord: Vec2f::new(bottom_points[i].x, bottom_points[i].z) * 0.5 + 0.5
        });
    }

    // bottom face
    for i in 0..vertex_segments {
        vertices.push(Vertex3D{
            position: top_points[i],
            normal: Vec3f::unit_y(),
            tangent: Vec3f::unit_x(),
            bitangent: Vec3f::unit_z(),
            texcoord: Vec2f::new(bottom_points[i].x, bottom_points[i].z) * 0.5 + 0.5
        });
    }

    // centre points
    vertices.push(Vertex3D{
        position: -Vec3f::unit_y(),
        normal: -Vec3f::unit_y(),
        tangent: Vec3f::unit_x(),
        bitangent: Vec3f::unit_z(),
        texcoord: Vec2f::point_five()
    });
    let centre_bottom = vertices.len()-1;

    vertices.push(Vertex3D{
        position: Vec3f::unit_y(),
        normal: Vec3f::unit_y(),
        tangent: Vec3f::unit_x(),
        bitangent: Vec3f::unit_z(),
        texcoord: Vec2f::point_five()
    });
    let centre_top = vertices.len()-1;

    //
    // Indices
    //

    // sides
    for i in 0..segments {
        let bottom = i;
        let top = i + vertex_segments;
        let next = i + 1;
        let top_next = i + 1 + vertex_segments;
        indices.extend(vec![
            bottom,
            top,
            next,
            top,
            top_next,
            next
        ]);
    }

    // bottom face - tri fan
    for i in 0..segments {
        let face_offset = vertex_segments * 2;
        let face_current = face_offset + i;
        let face_next = face_offset + (i + 1);
        indices.extend(vec![
            centre_bottom,
            face_current,
            face_next
        ]);
    }

    // top face - tri fan
    for i in 0..segments {
        let face_offset = vertex_segments * 3;
        let face_current = face_offset + i;
        let face_next = face_offset + (i + 1);
        indices.extend(vec![
            centre_top,
            face_next,
            face_current
        ]);
    }

    create_mesh_3d(dev, vertices, indices)
}