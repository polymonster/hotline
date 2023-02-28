#![allow(dead_code, unused_variables, unused_mut)]

use hotline_rs::prelude::*;
use maths_rs::prelude::*;

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

/// Subdivides a single quad into 4 evenly distributed smaller quads, adjusting uv's and maintaining normals and tangents
pub fn subdivide_quad(q0: &Vertex3D, q1: &Vertex3D, q2: &Vertex3D, q3: &Vertex3D, order: u32, max_order: u32) -> Vec<Vertex3D> {
    if order == max_order {
        vec![q0.clone(), q1.clone(), q2.clone(), q3.clone()]
    }
    else {
        //  __      ___
        // |  |    |_|_|
        // |__| -> |_|_|
        // 
        //  q1  s1  q2w
        //  s0  s4  s2
        //  q0  s3  q3

        let lerp_half = |a: &Vertex3D, b: &Vertex3D| -> Vertex3D {
            Vertex3D {
                position: a.position + (b.position - a.position) * 0.5,
                texcoord: a.texcoord + (b.texcoord - a.texcoord) * 0.5,
                tangent: a.tangent,
                normal: a.normal,
                bitangent: a.bitangent
            }
        };

        let s0 = lerp_half(q0, q1);
        let s1 = lerp_half(q1, q2);
        let s2 = lerp_half(q2, q3);
        let s3 = lerp_half(q3, q0);
        let s4 = lerp_half(&s3, &s1);

        let mut sub = subdivide_quad(q0, &s0, &s4, &s3, order + 1, max_order);
        sub.extend(subdivide_quad(&s0, &q1, &s1, &s4, order + 1, max_order));
        sub.extend(subdivide_quad(&s4, &s1, q2, &s2, order + 1, max_order));
        sub.extend(subdivide_quad(&s3, &s4, &s2, q3, order + 1, max_order));
        sub
    }
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

fn create_sphere_vertices(segments: usize, hemi_start: usize, hemi_end: usize, cap: bool) -> (Vec<Vertex3D>, Vec<usize>) {
    let vertex_segments = segments + 2;

    let angle_step = f32::two_pi() / segments as f32;
    let height_step = 2.0 / (segments - 1) as f32;

    let mid = segments / 2;

    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut angle = 0.0;
    let mut y = -1.0;

    let uoffet = 1.0 / segments as f32;

    for r in 0..vertex_segments {
        angle = 0.0;
        for i in 0..vertex_segments {
            let x = cos(angle);
            let z = -sin(angle);

            angle += angle_step;

            let radius = 1.0 - abs(y);
            let xz = vec3f(x, 0.0, z) * radius;
            let p = vec3f(xz.x, y, xz.z);

            // tangent
            let x = cos(angle);
            let z = -sin(angle);
            let xz = vec3f(x, 0.0, z) * radius;

            let p_next = vec3f(xz.x, y, xz.z);
            let p_next = normalize(p_next);

            let p = normalize(p);
            let t = p_next - p;
            let bt = cross(p, t);

            let mut u = 0.5 + atan2(z, x) / f32::two_pi();
            let v = 0.5 + asin(p.y) / f32::pi();

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
                texcoord: vec2f(u + uoffet, v) * 3.0
            });
        }

        y += height_step;
    }

    //
    // Indices
    //

    let hemi_start = if hemi_start > 0 {
        hemi_start - 1
    }
    else {
        hemi_start
    };

    let hemi_start = min(hemi_start, segments);
    let hemi_end = max(min(hemi_end, segments-1), 1);

    for r in hemi_start..hemi_end {
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

    if cap {
        // basis
        let n = Vec3f::unit_y();
        let t = Vec3f::unit_x();
        let bt = Vec3f::unit_z();
        let y = -1.0 + (hemi_end as f32 * height_step); // - height_step;

        let centre_cap = vertices.len();
        vertices.push(Vertex3D{
            position: Vec3f::unit_y() * y,
            normal: n,
            tangent: t,
            bitangent: bt,
            texcoord: Vec2f::point_five()
        });

        let loop_start = centre_cap + 1;

        for i in 0..vertex_segments {
            let x = cos(angle);
            let z = -sin(angle);
            let radius = 1.0 - abs(y);
            let xz = vec3f(x, 0.0, z) * radius;
            let p = vec3f(xz.x, y, xz.z);

            let p_next = vec3f(xz.x, y, xz.z);
            let p_next = normalize(p_next);
            let p = normalize(p);

            vertices.push(Vertex3D{
                position: p,
                normal: n,
                tangent: t,
                bitangent: bt,
                texcoord: vec2f(p.x, p.z) * 0.5 + 0.5
            });

            angle += angle_step;
        }

        // triangle per cap segmnent
        for i in 0..segments {
            indices.extend(vec![
                loop_start + i,
                centre_cap,
                loop_start + i + 1
            ]);
        }
    }
    (vertices, indices)
}

/// Create an `segments` sided prism, if `smooth` the prism is a cylinder with smooth normals
pub fn create_prism_vertices(segments: usize, smooth: bool, cap: bool) -> (Vec<Vertex3D>, Vec<usize>) {
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
    let angle_step = f32::two_pi() / segments as f32;
    for i in 0..vertex_segments {
        // current
        let mut x = cos(angle);
        let mut y = -sin(angle);
        let v1 = right * x + up * y;

        // next
        angle += angle_step;
        x = cos(angle);
        y = -sin(angle);
        let v2 = right * x + up * y;

        points.push(v1);
        tangents.push(v2 - v1);
        bottom_points.push(points[i] - Vec3f::unit_y());
        top_points.push(points[i] + Vec3f::unit_y());
    }

    //
    // Vertices
    //

    // bottom ring
    for i in 0..vertex_segments {
        let u = 0.5 + atan2(bottom_points[i].z, bottom_points[i].x) / f32::two_pi();
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
        let u = 0.5 + atan2(top_points[i].z, top_points[i].x) / f32::two_pi();
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

    if smooth {

        //
        // Smooth Indices
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

        if cap {
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
        }

        (vertices, indices)
    }
    else {
        // 2 tris per segment
        let mut triangle_vertices = Vec::new();
        for i in 0..segments {
            let bottom = i;
            let top = i + vertex_segments;
            let next = i + 1;
            let top_next = i + 1 + vertex_segments;

            let face_index = triangle_vertices.len();
            triangle_vertices.extend(vec![
                vertices[bottom].clone(),
                vertices[top].clone(),
                vertices[next].clone(),
                vertices[top].clone(),
                vertices[top_next].clone(),
                vertices[next].clone()
            ]);

            let v = face_index;
            let n = get_triangle_normal(
                triangle_vertices[v].position, 
                triangle_vertices[v+2].position, 
                triangle_vertices[v+1].position
            );

            // set hard face normals
            for v in face_index..triangle_vertices.len() {
                triangle_vertices[v].normal = n;
            }
        }

        if cap {
            // bottom face - tri fan
            for i in 0..segments {
                let face_offset = vertex_segments * 2;
                let face_current = face_offset + i;
                let face_next = face_offset + (i + 1);
                triangle_vertices.extend(vec![
                    vertices[centre_bottom].clone(),
                    vertices[face_current].clone(),
                    vertices[face_next].clone(),
                ]);
            }
    
            // top face - tri fan
            for i in 0..segments {
                let face_offset = vertex_segments * 3;
                let face_current = face_offset + i;
                let face_next = face_offset + (i + 1);
                triangle_vertices.extend(vec![
                    vertices[centre_top].clone(),
                    vertices[face_next].clone(),
                    vertices[face_current].clone(),
                ]);
            }
        }

        (vertices, Vec::new())
    }
}

/// Create a hemi-icosahedron in axis with subdivisions
pub fn hemi_icosohedron(axis: Vec3f, pos: Vec3f, start_angle: f32, subdivisions: u32) -> Vec<Vertex3D> {
    let (right, up, at) = basis_from_axis(axis);

    let tip = pos - at * f32::inv_phi();
    let dip = pos + at * 0.5 * 2.0;

    let angle_step = f32::pi() / 2.5;

    let mut a = start_angle;
    let mut vertices = Vec::new();

    for _ in 0..5 {
        let x = sin(a);
        let y = cos(a);
        let p = pos + right * x + up * y;

        a += angle_step;
        let x2 = sin(a);
        let y2 = cos(a);
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

/// Create a custom sphere mesh with segments subdivision, hemi_segments can clip the sphere
/// in different heights, supply `hemi_segments=segments/2` to create a perfect hemi-sphere
/// use cap to cap the cliped sphere or not
pub fn create_sphere_mesh_ex<D: gfx::Device>(dev: &mut D, segments: usize, hemi_segments: usize, cap: bool) -> pmfx::Mesh<D> {
    let (vertices, indices) = create_sphere_vertices(segments, 0, hemi_segments, cap);
    create_mesh_3d(dev, vertices, indices)
}

/// Create an indexed smooth sphere with subdivided icosophere vertices and smooth normals
pub fn create_sphere_mesh<D: gfx::Device>(dev: &mut D, segments: usize) -> pmfx::Mesh<D> {
    create_sphere_mesh_ex(dev, segments, segments, false)
}

/// Create an `segments` sided prism, if `smooth` the prism is a cylinder with smooth normals
pub fn create_prism_mesh<D: gfx::Device>(dev: &mut D, segments: usize, smooth: bool, cap: bool) -> pmfx::Mesh<D> {
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
    let angle_step = f32::two_pi() / segments as f32;
    for i in 0..vertex_segments {
        // current
        let mut x = cos(angle);
        let mut y = -sin(angle);
        let v1 = right * x + up * y;

        // next
        angle += angle_step;
        x = cos(angle);
        y = -sin(angle);
        let v2 = right * x + up * y;

        points.push(v1);
        tangents.push(v2 - v1);
        bottom_points.push(points[i] - Vec3f::unit_y());
        top_points.push(points[i] + Vec3f::unit_y());
    }

    //
    // Vertices
    //

    // bottom ring
    for i in 0..vertex_segments {
        let u = 0.5 + atan2(bottom_points[i].z, bottom_points[i].x) / f32::two_pi();
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
        let u = 0.5 + atan2(top_points[i].z, top_points[i].x) / f32::two_pi();
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

    if smooth {

        //
        // Smooth Indices
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

        if cap {
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
        }

        create_mesh_3d(dev, vertices, indices)
    }
    else {
        // 2 tris per segment
        let mut triangle_vertices = Vec::new();
        for i in 0..segments {
            let bottom = i;
            let top = i + vertex_segments;
            let next = i + 1;
            let top_next = i + 1 + vertex_segments;

            let face_index = triangle_vertices.len();
            triangle_vertices.extend(vec![
                vertices[bottom].clone(),
                vertices[top].clone(),
                vertices[next].clone(),
                vertices[top].clone(),
                vertices[top_next].clone(),
                vertices[next].clone()
            ]);

            let v = face_index;
            let n = get_triangle_normal(
                triangle_vertices[v].position, 
                triangle_vertices[v+2].position, 
                triangle_vertices[v+1].position
            );

            // set hard face normals
            for v in face_index..triangle_vertices.len() {
                triangle_vertices[v].normal = n;
            }
        }

        if cap {
            // bottom face - tri fan
            for i in 0..segments {
                let face_offset = vertex_segments * 2;
                let face_current = face_offset + i;
                let face_next = face_offset + (i + 1);
                triangle_vertices.extend(vec![
                    vertices[centre_bottom].clone(),
                    vertices[face_current].clone(),
                    vertices[face_next].clone(),
                ]);
            }
    
            // top face - tri fan
            for i in 0..segments {
                let face_offset = vertex_segments * 3;
                let face_current = face_offset + i;
                let face_next = face_offset + (i + 1);
                triangle_vertices.extend(vec![
                    vertices[centre_top].clone(),
                    vertices[face_next].clone(),
                    vertices[face_current].clone(),
                ]);
            }
        }

        create_faceted_mesh_3d(dev, triangle_vertices)
    }
}

/// Create a smooth unit-cylinder mesh with extents -1 to 1 and radius 1
pub fn create_cylinder_mesh<D: gfx::Device>(dev: &mut D, segments: usize) -> pmfx::Mesh<D> {
    create_prism_mesh(dev, segments, true, true)
}

/// Create an indexed unit cube subdivision mesh instance where the faces are subdivided into 4 smaller quads for `subdivisions` 
pub fn create_cube_subdivision_mesh<D: gfx::Device>(dev: &mut D, subdivisions: u32) -> pmfx::Mesh<D> {
    // cube veritces
    let vertices: Vec<Vertex3D> = vec![
        // front face
        Vertex3D {
            position: vec3f(-1.0, -1.0, 1.0),
            texcoord: vec2f(0.0, 0.0),
            normal: vec3f(0.0, 0.0, 1.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(1.0, -1.0,  1.0),
            texcoord: vec2f(1.0, 0.0),
            normal: vec3f(0.0, 0.0, 1.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(1.0, 1.0, 1.0),
            texcoord: vec2f(1.0, 1.0),
            normal: vec3f(0.0, 0.0, 1.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(-1.0, 1.0, 1.0),
            texcoord: vec2f(0.0, 1.0),
            normal: vec3f(0.0, 0.0, 1.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        // back face
        Vertex3D {
            position: vec3f(-1.0, -1.0, -1.0),
            texcoord: vec2f(0.0, 0.0),
            normal: vec3f(0.0, 0.0, -1.0),
            tangent: vec3f(-1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(-1.0, 1.0, -1.0),
            texcoord: vec2f(0.0, 1.0),
            normal: vec3f(0.0, 0.0, -1.0),
            tangent: vec3f(-1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(1.0, 1.0, -1.0),
            texcoord: vec2f(1.0, 1.0),
            normal: vec3f(0.0, 0.0, -1.0),
            tangent: vec3f(-1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(1.0, -1.0, -1.0),
            texcoord: vec2f(1.0, 0.0),
            normal: vec3f(0.0, 0.0, -1.0),
            tangent: vec3f(-1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        // right face
        Vertex3D {
            position: vec3f(1.0, -1.0, -1.0),
            texcoord: vec2f(0.0, 0.0),
            normal: vec3f(1.0, 0.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(1.0, 1.0, -1.0),
            texcoord: vec2f(1.0, 0.0),
            normal: vec3f(1.0, 0.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(1.0, 1.0, 1.0),
            texcoord: vec2f(1.0, 1.0),
            normal: vec3f(1.0, 0.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(1.0, -1.0, 1.0),
            texcoord: vec2f(0.0, 1.0),
            normal: vec3f(1.0, 0.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        // left face
        Vertex3D {
            position: vec3f(-1.0, -1.0, -1.0),
            texcoord: vec2f(0.0, 0.0),
            normal: vec3f(-1.0, 0.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(-1.0, -1.0, 1.0),
            texcoord: vec2f(0.0, 1.0),
            normal: vec3f(-1.0, 0.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(-1.0, 1.0, 1.0),
            texcoord: vec2f(1.0, 1.0),
            normal: vec3f(-1.0, 0.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(-1.0, 1.0, -1.0),
            texcoord: vec2f(1.0, 0.0),
            normal: vec3f(-1.0, 0.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        // top face
        Vertex3D {
            position: vec3f(-1.0, 1.0, -1.0),
            texcoord: vec2f(0.0, 0.0),
            normal: vec3f(0.0, 1.0, 0.0),
            tangent: vec3f(-1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(-1.0, 1.0, 1.0),
            texcoord: vec2f(0.0, 1.0),
            normal: vec3f(0.0, 1.0, 0.0),
            tangent: vec3f(-1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(1.0, 1.0, 1.0),
            texcoord: vec2f(1.0, 1.0),
            normal: vec3f(0.0, 1.0, 0.0),
            tangent: vec3f(-1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(1.0, 1.0, -1.0),
            texcoord: vec2f(1.0, 0.0),
            normal: vec3f(0.0, 1.0, 0.0),
            tangent: vec3f(-1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        // bottom face
        Vertex3D {
            position: vec3f(-1.0, -1.0, -1.0),
            texcoord: vec2f(0.0, 0.0),
            normal: vec3f(0.0, -1.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(1.0, -1.0, -1.0),
            texcoord: vec2f(1.0, 0.0),
            normal: vec3f(0.0, -1.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(1.0, -1.0, 1.0),
            texcoord: vec2f(1.0, 1.0),
            normal: vec3f(0.0, -1.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(-1.0, -1.0, 1.0),
            texcoord: vec2f(0.0, 1.0),
            normal: vec3f(0.0, -1.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
    ];

    let mut subdiv_vertices = Vec::new();
    for i in (0..vertices.len()).step_by(4) {
        let sub = subdivide_quad(&vertices[i], &vertices[i+1], &vertices[i+2], &vertices[i+3], 0, subdivisions);
        subdiv_vertices.extend(sub);
    }

    // explode to a sphere
    for v in &mut subdiv_vertices {
        v.position = normalize(v.position);
        v.normal = v.position;
    }

    // create indices... flip the triangles to create better 

    //   ___
    //  |/|\|
    //  |\|/|
    // 

    let mut indices = Vec::new();
    for i in (0..subdiv_vertices.len()).step_by(4) {
        let quad = (i / 4) % 4;
        if subdivisions > 0 {
            if quad == 0 {
                indices.extend(vec![
                    i,  i + 3,  i + 1,
                    i + 1,  i + 3,  i + 2
                ]);
            }
            else if quad == 3 {
                indices.extend(vec![
                    i,  i + 3,  i + 2,
                    i,  i + 2,  i + 1
                ]);
            }
            else if quad == 2 {
                indices.extend(vec![
                    i,  i + 3,  i + 1,
                    i + 1,  i + 3,  i + 2
                ]);
            }
            else {
                indices.extend(vec![
                    i,  i + 2,  i + 1,
                    i,  i + 3,  i + 2
                ]);
            }
        }
        else {
            indices.extend(vec![
                i,  i + 2,  i + 1,  i + 2,  i,  i + 3
            ]);
        }
    }

    create_mesh_3d(dev, subdiv_vertices, indices)
}

/// creates a pyramid mesh, if smooth this is essentially a low poly cone with smooth normals
pub fn create_pyramid_mesh<D: gfx::Device>(dev: &mut D, segments: usize, smooth: bool, cap: bool) -> pmfx::Mesh<D> {
    let axis = Vec3f::unit_y();
    let right = Vec3f::unit_x();
    let up = cross(axis, right);
    let right = cross(axis, up);
    let tip = Vec3f::unit_y();
    let base = -Vec3f::unit_y();

    let mut segment_vertices = Vec::new();
    let mut vertices = Vec::new();

    // add an extra segment at the end to make uv's wrap nicely
    let vertex_segments = segments + 1;

    // rotate around up axis and extract some data we can lookup to build vb and ib
    let mut angle = 0.0;
    let angle_step = f32::two_pi() / segments as f32;
    for i in 0..vertex_segments {
        // current
        let mut x = cos(angle);
        let mut y = -sin(angle);
        let v1 = right * x + up * y;

        // next
        angle += angle_step;
        x = cos(angle);
        y = -sin(angle);
        let v2 = right * x + up * y;

        // uv
        let u = 0.5 + atan2(v1.z, v2.x) / f32::two_pi();
        let u = if i == segments { 0.0 } else { u };

        // tbn
        let n = cross(normalize(v2 - v1), normalize(tip - v1));
        let t = v2 - v1;
        let bt = cross(n, t);

        segment_vertices.push(
            Vertex3D {
                position: v1 - Vec3f::unit_y(),
                texcoord: vec2f(u * 3.0, 0.0),
                normal: n,
                tangent: t,
                bitangent: bt
            });
    }

    //
    // Vertices (traingle faces)
    //

    // tri per segment connected to the tip
    if smooth {
        for i in 0..segments {
            let mid = segment_vertices[i].texcoord.x + (segment_vertices[i + 1].texcoord.x - segment_vertices[i].texcoord.x) * 0.5;
            vertices.extend(vec![
                segment_vertices[i].clone(),
                Vertex3D {
                    position: tip,
                    texcoord: vec2f(mid, 1.0),
                    normal: segment_vertices[i].normal,
                    tangent: segment_vertices[i].tangent,
                    bitangent: segment_vertices[i].bitangent,
                },
                segment_vertices[i + 1].clone(),
            ])
        }
    }
    else {
        for i in 0..segments {

            let t0 = segment_vertices[i].position;
            let t1 = tip;
            let t2 = segment_vertices[i + 1].position;
            let n = get_triangle_normal(t0, t2, t1);
    
            vertices.extend(vec![
                Vertex3D {
                    position: t0,
                    texcoord: Vec2f::zero(),
                    normal: n,
                    tangent: segment_vertices[i].tangent,
                    bitangent: segment_vertices[i].bitangent,
                },
                Vertex3D {
                    position: t1,
                    texcoord: vec2f(0.5, 1.0),
                    normal: n,
                    tangent: segment_vertices[i].tangent,
                    bitangent: segment_vertices[i].bitangent,
                },
                Vertex3D {
                    position: t2,
                    texcoord: vec2f(1.0, 0.0),
                    normal: n,
                    tangent: segment_vertices[i].tangent,
                    bitangent: segment_vertices[i].bitangent,
                }
            ])
        }  
    }

    // base cap
    if cap {
        for i in 0..segments {
            vertices.extend(vec![
                Vertex3D {
                    position: segment_vertices[i].position,
                    texcoord: segment_vertices[i].position.xz() * 0.5 + 0.5,
                    normal: -Vec3f::unit_y(),
                    tangent: Vec3f::unit_x(),
                    bitangent: Vec3f::unit_z(),
                },
                Vertex3D {
                    position: segment_vertices[i + 1].position,
                    texcoord: segment_vertices[i + 1].position.xz() * 0.5 + 0.5,
                    normal: -Vec3f::unit_y(),
                    tangent: Vec3f::unit_x(),
                    bitangent: Vec3f::unit_z(),
                },
                Vertex3D {
                    position: base,
                    texcoord: Vec2f::point_five(),
                    normal: -Vec3f::unit_y(),
                    tangent: Vec3f::unit_x(),
                    bitangent: Vec3f::unit_z(),
                }
            ])
        }
    }

    create_faceted_mesh_3d(dev, vertices)
}

// create a cone mesh with smooth normals made up of `segments` number of sides
pub fn create_cone_mesh<D: gfx::Device>(dev: &mut D, segments: usize) -> pmfx::Mesh<D> {
    create_pyramid_mesh(dev, segments, true, true)
}

// create a capsule mesh with smooth normals made up of `segments` number of sides
pub fn create_capsule_mesh<D: gfx::Device>(dev: &mut D, segments: usize) -> pmfx::Mesh<D> {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let offset = 0.5 + (1.0 / segments as f32);

    // bottom sphere
    let (mut v0, i0) = create_sphere_vertices(segments, 0, segments/2, false);
    for v in &mut v0 {
        v.position += vec3f(0.0, -offset, 0.0);
    }

    // top sphere
    let (mut v1, mut i1) = create_sphere_vertices(segments, segments/2, segments, false);    
    for v in &mut v1 {
        v.position += vec3f(0.0, offset, 0.0);
    }

    // offset the indices
    let base = v0.len();
    for i in &mut i1 {
        *i += base;
    }

    // cylinder
    let (mut v2, mut i2) = create_prism_vertices(segments, true, false);   
    for v in &mut v2 {
        v.position *= vec3f(1.0, 0.5, 1.0);
    } 

    // offset the indices
    let base = base + v1.len();
    for i in &mut i2 {
        *i += base;
    }

    vertices.extend(v0);
    indices.extend(i0);
    vertices.extend(v1);
    indices.extend(i1);
    vertices.extend(v2);
    indices.extend(i2);

    create_mesh_3d(dev, vertices, indices)
}

/// Create a unit smooth tourus mesh 
pub fn create_tourus_mesh<D: gfx::Device>(dev: &mut D, segments: usize) -> pmfx::Mesh<D> {
    let mut segment_vertices = Vec::new();
    let mut vertices = Vec::new();

    // add an extra segment at the end to make uv's wrap nicely
    let vertex_segments = segments + 1;
    let radius = 0.5;

    let mid = segments / 2;

    // rotate around up axis and extract some data we can lookup to build vb and ib
    let mut hangle = 0.0;
    let angle_step = f32::two_pi() / segments as f32;
    for i in 0..vertex_segments + 1 {
        let x = cos(hangle);
        let y = -sin(hangle);
        
        hangle += angle_step;
        let x2 = cos(hangle);
        let y2 = -sin(hangle);
        
        let x3 = -sin(hangle + angle_step);
        let y3 = cos(hangle + angle_step);
        
        let p = vec3f(x, 0.0, y);
        let np = vec3f(x2, 0.0, y2);
        let nnp = vec3f(x3, 0.0, y3);
        
        let at = normalize(np - p);
        let up = Vec3f::unit_y();
        let right = cross(up, at);
        
        let nat = normalize(nnp - np);
        let nright = cross(up, nat);

        let mut vangle = 0.0;
        for j in 0..vertex_segments {
            let vx = cos(vangle) * radius;
            let vy = -sin(vangle) * radius;
            let vv = p + vx * up + vy * right;
              
            let n = normalize(vx * up + vy * right);
            let t = right;
            let bt = up;

            let mut u = 0.5 + atan2(y, x) / f32::two_pi();
            let mut v = 0.5 + atan2(vy, vx) / f32::two_pi();

            let u = if i == mid+1 { 1.0 } else { u };
            if i == mid {
                hangle -= angle_step;
            }

            let v = if j == mid+1 { 0.0 } else { v };
            if j == mid {
                //vangle -= angle_step;
            }

            segment_vertices.extend(vec![
                Vertex3D {
                    position: vv,
                    normal: n,
                    tangent: t,
                    bitangent: bt,
                    texcoord: vec2f(u, v) * 3.0
                }
            ]);

            vangle += angle_step;
        }
    }

    for i in 0..segments+1 {
        for j in 0..segments {
            let base = (i * vertex_segments) + j;
            let next_loop = base + 1;
            let next_base = ((i + 1) * vertex_segments) + j;
            let next_next_loop = next_base + 1;
            vertices.extend(vec![
                segment_vertices[base].clone(),
                segment_vertices[next_base].clone(),
                segment_vertices[next_loop].clone(),
                segment_vertices[next_base].clone(),
                segment_vertices[next_next_loop].clone(),
                segment_vertices[next_loop].clone(),
            ]);
        }
    }

    create_faceted_mesh_3d(dev, vertices)
}