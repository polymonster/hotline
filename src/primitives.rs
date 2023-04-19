use crate::prelude::*;
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
const INV_PHI : f32 = 0.618034;

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
        // |  | -> |_|_|
        // |__|    |_|_|
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
        sub.extend(subdivide_quad(&s0, q1, &s1, &s4, order + 1, max_order));
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
        sub.extend(subdivide_triangle(&s1, &s2, t2, order + 1, max_order));
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
            usage: gfx::BufferUsage::INDEX,
            cpu_access: gfx::CpuAccessFlags::NONE,
            num_elements: indices32.len(),
            format: gfx::Format::R32u,
            stride: 4,
            initial_state: gfx::ResourceState::IndexBuffer
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
            usage: gfx::BufferUsage::INDEX,
            cpu_access: gfx::CpuAccessFlags::NONE,
            num_elements: indices16.len(),
            format: gfx::Format::R16u,
            stride: 2,
            initial_state: gfx::ResourceState::IndexBuffer
            },
            Some(indices16.as_slice())
        ).unwrap()
    };

    let aabb_min = vertices.iter().fold( Vec3f::max_value(), |acc, v| min(acc, v.position));
    let aabb_max = vertices.iter().fold(-Vec3f::max_value(), |acc, v| max(acc, v.position));

    pmfx::Mesh {
        vb: dev.create_buffer(&gfx::BufferInfo {
                usage: gfx::BufferUsage::VERTEX,
                cpu_access: gfx::CpuAccessFlags::NONE,
                num_elements: vertices.len(),
                format: gfx::Format::Unknown,
                stride: std::mem::size_of::<Vertex3D>(),
                initial_state: gfx::ResourceState::VertexConstantBuffer
            }, 
            Some(vertices.as_slice())
        ).unwrap(),
        ib: index_buffer,
        num_indices: indices.len() as u32,
        aabb_min,
        aabb_max
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
    let vertex_segments = segments + 1;

    let angle_step = f32::two_pi() / segments as f32;
    let height_step = 2.0 / (segments - 1) as f32;

    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut angle = 0.0;
    let mut y = -1.0;

    for _ in 0..vertex_segments {
        angle = -f32::pi();
        for i in 0..vertex_segments {
            let x = cos(angle);
            let z = -sin(angle);

            let u = 0.5 + atan2(z, x) / f32::two_pi();

            let radius = 1.0 - abs(y);
            let xz = vec3f(x, 0.0, z) * radius;
            let p = vec3f(xz.x, y, xz.z);

            // tangent
            angle += angle_step;

            let x = cos(angle);
            let z = -sin(angle);
            let xz = vec3f(x, 0.0, z) * radius;

            let p_next = vec3f(xz.x, y, xz.z);
            let p_next = normalize(p_next);

            let p = normalize(p);

            let mut t = p_next - p;

            // handle case of small p_next - p
            if mag2(t) < 0.001 {
                t = Vec3f::unit_x();
            }

            let bt = cross(p, t);

            let v = 0.5 + asin(p.y) / f32::pi();

            // clamps the UV's in the last segment to prevent interpolation artifacts            
            let u = if i == segments { 0.0 } else { u };
            let u = if i == 0 { 1.0 } else { u };

            vertices.push(Vertex3D{
                position: p,
                normal: p,
                tangent: t,
                bitangent: bt,
                texcoord: vec2f(1.0 - u, 1.0 - v) * 3.0
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
        for i in 0..segments {
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

        for _ in 0..vertex_segments {
            let x = cos(angle);
            let z = -sin(angle);
            let radius = 1.0 - abs(y);
            let xz = vec3f(x, 0.0, z) * radius;
            let p = vec3f(xz.x, y, xz.z);
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
    for point in bottom_points.iter().take(segments) {
        vertices.push(Vertex3D{
            position: *point,
            normal: -Vec3f::unit_y(),
            tangent: Vec3f::unit_x(),
            bitangent: Vec3f::unit_z(),
            texcoord: Vec2f::new(point.x, point.z) * 0.5 + 0.5
        });
    }

    // top face
    for point in top_points.iter().take(segments) {
        vertices.push(Vertex3D{
            position: *point,
            normal: Vec3f::unit_y(),
            tangent: Vec3f::unit_x(),
            bitangent: Vec3f::unit_z(),
            texcoord: Vec2f::new(point.x, point.z) * 0.5 + 0.5
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
            for vertex in triangle_vertices.iter_mut().skip(face_index) {
                vertex.normal = n;
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

/// Create an indexed unit quad mesh instance
pub fn create_unit_quad_mesh<D: gfx::Device>(dev: &mut D) -> pmfx::Mesh<D> {
    // front face
    let vertices: Vec<Vertex2D> = vec![
        Vertex2D {
            position: vec2f(-1.0, -1.0),
            texcoord: vec2f(0.0, 1.0),
        },
        Vertex2D {
            position: vec2f(1.0, -1.0),
            texcoord: vec2f(1.0, 1.0),
        },
        Vertex2D {
            position: vec2f(1.0, 1.0),
            texcoord: vec2f(1.0, 0.0),
        },
        Vertex2D {
            position: vec2f(-1.0, 1.0),
            texcoord: vec2f(0.0, 0.0),
        }
    ];

    let indices: Vec<u16> = vec![
        0,  2,  1,  2,  0,  3
    ];

    pmfx::Mesh {
        vb: dev.create_buffer(&gfx::BufferInfo {
                usage: gfx::BufferUsage::VERTEX,
                cpu_access: gfx::CpuAccessFlags::NONE,
                num_elements: 4,
                format: gfx::Format::Unknown,
                stride: std::mem::size_of::<Vertex2D>(),
                initial_state: gfx::ResourceState::VertexConstantBuffer
            }, 
            Some(vertices.as_slice())
        ).unwrap(),
        ib: dev.create_buffer(&gfx::BufferInfo {
            usage: gfx::BufferUsage::INDEX,
            cpu_access: gfx::CpuAccessFlags::NONE,
            num_elements: 6,
            format: gfx::Format::R16u,
            stride: std::mem::size_of::<u16>(),
            initial_state: gfx::ResourceState::IndexBuffer
            },
            Some(indices.as_slice())
        ).unwrap(),
        num_indices: 6,
        aabb_min: vec3f(-1.0, -1.0, 0.0),
        aabb_max: vec3f( 1.0,  1.0, 0.0)
    } 
}

/// Create an indexed unit billboard quad mesh instance with the front face pointing +z 
pub fn create_billboard_mesh<D: gfx::Device>(dev: &mut D) -> pmfx::Mesh<D> {
    // quad veritces
    let vertices: Vec<Vertex3D> = vec![
        // front face
        Vertex3D {
            position: vec3f(-1.0, -1.0, 0.0),
            texcoord: vec2f(0.0, 1.0),
            normal: vec3f(0.0, 0.0, 1.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(1.0, -1.0, 0.0),
            texcoord: vec2f(1.0, 1.0),
            normal: vec3f(0.0, 0.0, 1.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(1.0, 1.0, 0.0),
            texcoord: vec2f(1.0, 0.0),
            normal: vec3f(0.0, 0.0, 1.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(-1.0, 1.0, 0.0),
            texcoord: vec2f(0.0, 0.0),
            normal: vec3f(0.0, 0.0, 1.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        }
    ];

    let indices: Vec<u16> = vec![
        0,  2,  1,  2,  0,  3,   // front face
    ];

    pmfx::Mesh {
        vb: dev.create_buffer(&gfx::BufferInfo {
                usage: gfx::BufferUsage::VERTEX,
                cpu_access: gfx::CpuAccessFlags::NONE,
                num_elements: 4,
                format: gfx::Format::Unknown,
                stride: std::mem::size_of::<Vertex3D>(),
                initial_state: gfx::ResourceState::VertexConstantBuffer
            }, 
            Some(vertices.as_slice())
        ).unwrap(),
        ib: dev.create_buffer(&gfx::BufferInfo {
                usage: gfx::BufferUsage::INDEX,
                cpu_access: gfx::CpuAccessFlags::NONE,
                num_elements: 6,
                format: gfx::Format::R16u,
                stride: std::mem::size_of::<u16>(),
                initial_state: gfx::ResourceState::IndexBuffer
            },
            Some(indices.as_slice())
        ).unwrap(),
        num_indices: 6,
        aabb_min: vec3f(-1.0, -1.0, 0.0),
        aabb_max: vec3f( 1.0,  1.0, 0.0)
    } 
}

/// Create an indexed unit triangle mesh instance with the front face pointing +z 
pub fn create_triangle_mesh<D: gfx::Device>(dev: &mut D) -> pmfx::Mesh<D> {
    // quad veritces
    let vertices: Vec<Vertex3D> = vec![
        // front face
        Vertex3D {
            position: vec3f(-1.0, -1.0, 0.0),
            texcoord: vec2f(0.0, 1.0),
            normal: vec3f(0.0, 0.0, 1.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(0.0, 1.0, 0.0),
            texcoord: vec2f(0.5, 0.0),
            normal: vec3f(0.0, 0.0, 1.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(1.0, -1.0, 0.0),
            texcoord: vec2f(1.0, 1.0),
            normal: vec3f(0.0, 0.0, 1.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
    ];

    let indices: Vec<u16> = vec![
        0,  2,  1
    ];

    let aabb_min = vertices.iter().fold( Vec3f::max_value(), |acc, v| min(acc, v.position));
    let aabb_max = vertices.iter().fold(-Vec3f::max_value(), |acc, v| max(acc, v.position));

    pmfx::Mesh {
        vb: dev.create_buffer(&gfx::BufferInfo {
                usage: gfx::BufferUsage::VERTEX,
                cpu_access: gfx::CpuAccessFlags::NONE,
                num_elements: 3,
                format: gfx::Format::Unknown,
                stride: std::mem::size_of::<Vertex3D>(),
                initial_state: gfx::ResourceState::VertexConstantBuffer
            }, 
            Some(vertices.as_slice())
        ).unwrap(),
        ib: dev.create_buffer(&gfx::BufferInfo {
                usage: gfx::BufferUsage::INDEX,
                cpu_access: gfx::CpuAccessFlags::NONE,
                num_elements: 3,
                format: gfx::Format::R16u,
                stride: std::mem::size_of::<u16>(),
                initial_state: gfx::ResourceState::IndexBuffer
            },
            Some(indices.as_slice())
        ).unwrap(),
        num_indices: 3,
        aabb_min,
        aabb_max
    } 
}

/// Create an indexed unit subdivided plane mesh facing +y direction with evenly subdivided quads `subdivisions`
pub fn create_plane_mesh<D: gfx::Device>(dev: &mut D, subdivisions: u32) -> pmfx::Mesh<D> {
    let start = vec3f(-1.0, 0.0, -1.0);
    let increment = 2.0 / subdivisions as f32;
    
    let mut pos = start;
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for _ in 0..subdivisions {
        pos.x = start.x;
        for _ in 0..subdivisions {
            // quad per suub division
            let quad_base_index = vertices.len();

            vertices.extend(vec![
                Vertex3D {
                    position: vec3f(pos.x, 0.0, pos.z),
                    texcoord: vec2f(0.0, 0.0),
                    normal: Vec3f::unit_y(),
                    tangent: vec3f(1.0, 0.0, 0.0),
                    bitangent: vec3f(0.0, 1.0, 0.0),
                },
                Vertex3D {
                    position: vec3f(pos.x + increment, 0.0, pos.z),
                    texcoord: vec2f(1.0, 0.0),
                    normal: Vec3f::unit_y(),
                    tangent: vec3f(1.0, 0.0, 0.0),
                    bitangent: vec3f(0.0, 1.0, 0.0),
                },
                Vertex3D {
                    position: vec3f(pos.x + increment, 0.0, pos.z + increment),
                    texcoord: vec2f(1.0, 1.0),
                    normal: Vec3f::unit_y(),
                    tangent: vec3f(1.0, 0.0, 0.0),
                    bitangent: vec3f(0.0, 1.0, 0.0),
                },
                Vertex3D {
                    position: vec3f(pos.x, 0.0, pos.z + increment),
                    texcoord: vec2f(0.0, 1.0),
                    normal: Vec3f::unit_y(),
                    tangent: vec3f(1.0, 0.0, 0.0),
                    bitangent: vec3f(0.0, 1.0, 0.0),
                }
            ]);
            
            indices.extend(vec![
                quad_base_index,  quad_base_index + 1,  quad_base_index + 2,  
                quad_base_index,  quad_base_index + 2,  quad_base_index + 3
            ]);

            pos.x += increment;
        }
        pos.z += increment;
    }

    create_mesh_3d(dev, vertices, indices)
}

/// Create a an indexed unit tetrahedron mesh instance
pub fn create_tetrahedron_mesh<D: gfx::Device>(dev: &mut D) -> pmfx::Mesh<D> {
    let pos = vec3f(0.0, -INV_PHI, 0.0);
    let right = Vec3f::unit_x();
    let up = Vec3f::unit_z();
    let at = Vec3f::unit_y();
    let angle_step = (f32::pi() * 2.0) / 3.0;
    let tip = pos + at * sqrt(2.0); // sqrt 2 is pythagoras constant

    // we gather the base vertices and faces as we iterate
    let mut base_positions = Vec::new();
    let mut vertices = Vec::new();

    // pos, next pos, top pos
    let get_face_vertices = |p: Vec3f, np: Vec3f, tp: Vec3f| -> Vec<Vertex3D> {
        let n = maths_rs::get_triangle_normal(p, np, tp);
        let b = normalize(p - np);
        let t = cross(n, b);
        vec![
            Vertex3D {
                position: normalize(tp),
                texcoord: vec2f(0.5, 0.0),
                normal: n,
                tangent: t,
                bitangent: b,
            },
            Vertex3D {
                position: normalize(np),
                texcoord: vec2f(1.0, 1.0),
                normal: n,
                tangent: t,
                bitangent: b,
            },
            Vertex3D {
                position: normalize(p),
                texcoord: vec2f(0.0, 1.0),
                normal: n,
                tangent: t,
                bitangent: b,
            }
        ]
    };    

    // make the sides with y-up
    let mut a = 0.0;
    for _ in 0..3 {
        // current pos
        let x = f32::sin(a);
        let y = f32::cos(a);
        let p = pos + right * x + up * y;
        
        // next pos and tip
        a += angle_step;
        let x2 = f32::sin(a);
        let y2 = f32::cos(a);
        let np = pos + right * x2 + up * y2;
        let tp = tip;
        
        base_positions.push(p);
        vertices.extend(get_face_vertices(p , np, tp));
    }

    // make the base face
    vertices.extend(get_face_vertices(base_positions[0], base_positions[2], base_positions[1]));

    // generate indices
    create_faceted_mesh_3d(dev, vertices)
}

pub fn cube_vertices() -> Vec<Vertex3D> {
    vec![
        // front face
        Vertex3D {
            position: vec3f(-1.0, -1.0, 1.0),
            texcoord: vec2f(0.0, 1.0),
            normal: vec3f(0.0, 0.0, 1.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(1.0, -1.0,  1.0),
            texcoord: vec2f(1.0, 1.0),
            normal: vec3f(0.0, 0.0, 1.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(1.0, 1.0, 1.0),
            texcoord: vec2f(1.0, 0.0),
            normal: vec3f(0.0, 0.0, 1.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(-1.0, 1.0, 1.0),
            texcoord: vec2f(0.0, 0.0),
            normal: vec3f(0.0, 0.0, 1.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        // back face
        Vertex3D {
            position: vec3f(-1.0, -1.0, -1.0),
            texcoord: vec2f(1.0, 1.0),
            normal: vec3f(0.0, 0.0, -1.0),
            tangent: vec3f(-1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(-1.0, 1.0, -1.0),
            texcoord: vec2f(1.0, 0.0),
            normal: vec3f(0.0, 0.0, -1.0),
            tangent: vec3f(-1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(1.0, 1.0, -1.0),
            texcoord: vec2f(0.0, 0.0),
            normal: vec3f(0.0, 0.0, -1.0),
            tangent: vec3f(-1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(1.0, -1.0, -1.0),
            texcoord: vec2f(0.0, 1.0),
            normal: vec3f(0.0, 0.0, -1.0),
            tangent: vec3f(-1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        // right face
        Vertex3D {
            position: vec3f(1.0, -1.0, -1.0),
            texcoord: vec2f(0.0, 1.0),
            normal: vec3f(1.0, 0.0, 0.0),
            tangent: vec3f(0.0, 0.0, 1.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(1.0, 1.0, -1.0),
            texcoord: vec2f(0.0, 0.0),
            normal: vec3f(1.0, 0.0, 0.0),
            tangent: vec3f(0.0, 0.0, 1.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(1.0, 1.0, 1.0),
            texcoord: vec2f(1.0, 0.0),
            normal: vec3f(1.0, 0.0, 0.0),
            tangent: vec3f(0.0, 0.0, 1.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(1.0, -1.0, 1.0),
            texcoord: vec2f(1.0, 1.0),
            normal: vec3f(1.0, 0.0, 0.0),
            tangent: vec3f(0.0, 0.0, 1.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        // left face
        Vertex3D {
            position: vec3f(-1.0, -1.0, -1.0),
            texcoord: vec2f(1.0, 1.0),
            normal: vec3f(-1.0, 0.0, 0.0),
            tangent: vec3f(0.0, 0.0, -1.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(-1.0, -1.0, 1.0),
            texcoord: vec2f(0.0, 1.0),
            normal: vec3f(-1.0, 0.0, 0.0),
            tangent: vec3f(0.0, 0.0, -1.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(-1.0, 1.0, 1.0),
            texcoord: vec2f(0.0, 0.0),
            normal: vec3f(-1.0, 0.0, 0.0),
            tangent: vec3f(0.0, 0.0, -1.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(-1.0, 1.0, -1.0),
            texcoord: vec2f(1.0, 0.0),
            normal: vec3f(-1.0, 0.0, 0.0),
            tangent: vec3f(0.0, 0.0, -1.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        // top face
        Vertex3D {
            position: vec3f(-1.0, 1.0, -1.0),
            texcoord: vec2f(0.0, 1.0),
            normal: vec3f(0.0, 1.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 0.0, 1.0),
        },
        Vertex3D {
            position: vec3f(-1.0, 1.0, 1.0),
            texcoord: vec2f(0.0, 0.0),
            normal: vec3f(0.0, 1.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 0.0, 1.0),
        },
        Vertex3D {
            position: vec3f(1.0, 1.0, 1.0),
            texcoord: vec2f(1.0, 0.0),
            normal: vec3f(0.0, 1.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 0.0, 1.0),
        },
        Vertex3D {
            position: vec3f(1.0, 1.0, -1.0),
            texcoord: vec2f(1.0, 1.0),
            normal: vec3f(0.0, 1.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 0.0, 1.0),
        },
        // bottom face
        Vertex3D {
            position: vec3f(-1.0, -1.0, -1.0),
            texcoord: vec2f(1.0, 0.0),
            normal: vec3f(0.0, -1.0, 0.0),
            tangent: vec3f(-1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 0.0, -1.0),
        },
        Vertex3D {
            position: vec3f(1.0, -1.0, -1.0),
            texcoord: vec2f(0.0, 0.0),
            normal: vec3f(0.0, -1.0, 0.0),
            tangent: vec3f(-1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 0.0, -1.0),
        },
        Vertex3D {
            position: vec3f(1.0, -1.0, 1.0),
            texcoord: vec2f(0.0, 1.0),
            normal: vec3f(0.0, -1.0, 0.0),
            tangent: vec3f(-1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 0.0, -1.0),
        },
        Vertex3D {
            position: vec3f(-1.0, -1.0, 1.0),
            texcoord: vec2f(1.0, 1.0),
            normal: vec3f(0.0, -1.0, 0.0),
            tangent: vec3f(-1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 0.0, -1.0),
        },
    ]
}

/// Create an indexed unit cube mesh instance.
pub fn create_cube_mesh<D: gfx::Device>(dev: &mut D) -> pmfx::Mesh<D> {
    let indices: Vec<usize> = vec![
        0,  2,  1,  2,  0,  3,   // front face
        4,  6,  5,  6,  4,  7,   // back face
        8,  10, 9,  10, 8,  11,  // right face
        12, 14, 13, 14, 12, 15,  // left face
        16, 18, 17, 18, 16, 19,  // top face
        20, 22, 21, 22, 20, 23   // bottom face
    ];

    create_mesh_3d(dev, cube_vertices(), indices)
}

/// Creates a unit octahedron mesh aligned y-up
pub fn create_octahedron_mesh<D: gfx::Device>(dev: &mut D) -> pmfx::Mesh<D> {
    let corner = [
        vec3f(-1.0, 0.0, -1.0),
        vec3f(-1.0, 0.0,  1.0),
        vec3f( 1.0, 0.0,  1.0),
        vec3f( 1.0, 0.0, -1.0)
    ];

    let pc = sqrt(2.0);
    let top = vec3f(0.0, pc, 0.0);
    let bottom = vec3f(0.0, -pc, 0.0);

    // we make it in 2 halfs one points up in y, the other down
    let yextent = [
        top,
        bottom
    ];

    let mut vertices = Vec::new();

    for i in 0..4 {
        let n = (i + 1) % 4;
                
        // 2 tris per-edge 1 up, one down
        for (j, yextent) in yextent.iter().enumerate() {
            
            // vertices
            let mut t0 = corner[i];
            let t1 = corner[n];
            let mut t2 = *yextent;

            // tex coords
            let mut tc0 = vec2f(1.0, 1.0);
            let tc1 = vec2f(0.0, 1.0);
            let mut tc2 = vec2f(0.5, 0.0); 

            // flip if we are the top
            if j == 0 {
                std::mem::swap(&mut t0, &mut t2);
                std::mem::swap(&mut tc0, &mut tc2);
            }

            // normals and tangents
            let n = get_triangle_normal(t0, t2, t1);
            let b = normalize(t0 - t1);
            let t = cross(n, b);

            let tri: Vec<Vertex3D> = vec![
                Vertex3D {
                    position: t0,
                    texcoord: tc0,
                    normal: n,
                    tangent: t,
                    bitangent: b,
                },
                Vertex3D {
                    position: t1,
                    texcoord: tc1,
                    normal: n,
                    tangent: t,
                    bitangent: b,
                },
                Vertex3D {
                    position: t2,
                    texcoord: tc2,
                    normal: n,
                    tangent: t,
                    bitangent: b,
                }
            ];
            vertices.extend(tri);
        }
    }

    create_faceted_mesh_3d(dev, vertices)
}

/// Intenral utility which can regursively build a hemi-dodecahedrin starting with a single pentagonal face with normal `axis`
fn dodecahedron_face_in_axis(axis: Vec3f, pos: Vec3f, start_angle: f32, recurse: bool) -> Vec<Vertex3D> {
    let (right, up, _) = basis_from_axis(axis);
    let angle_step = f32::pi() / 2.5;

    let mut a = start_angle;
    let mut vertices = Vec::new();

    // pos is centreed inthe middle of a pentagonal face
    let t2 = pos;

    // pentagon with tri per edge, makes a tri with the 2 edge vertices and 1 vertex and t2 in the centre
    for _ in 0..5 {
        let x = f32::sin(a) * INV_PHI;
        let y = f32::cos(a) * INV_PHI;
        let t0 = pos + right * x + up * y;
        let uv0 = Vec2f::new(f32::sin(a), f32::cos(a));
        
        a += angle_step;
        let x2 = f32::sin(a) * INV_PHI;
        let y2 = f32::cos(a) * INV_PHI;
        let t1 = pos + right * x2 + up * y2;
        let uv1 = Vec2f::new(f32::sin(a), f32::cos(a));

        let n = get_triangle_normal(t0, t2, t1);
        let b = normalize(t0 - t1);
        let t = cross(n, b);
                    
        let tri = vec![
            Vertex3D {
                position: t0,
                texcoord: uv0 * 0.5 + 0.5,
                normal: n,
                tangent: t,
                bitangent: b,
            },
            Vertex3D {
                position: t1,
                texcoord: uv1 * 0.5 + 0.5,
                normal: n,
                tangent: t,
                bitangent: b,
            },
            Vertex3D {
                position: t2,
                texcoord: Vec2f::point_five(),
                normal: n,
                tangent: t,
                bitangent: b,
            }
        ];
        vertices.extend(tri);

        if recurse {
            let half_gr = f32::phi() / 2.0;
            let internal_angle = 0.309017 * 1.5;

            let ev = normalize(t1 - t0);
            let cp = normalize(cross(ev, axis));
            let mid = t0 + (t1 - t0) * 0.5;
            
            let rx = f32::sin((f32::pi() * 2.0) + internal_angle) * INV_PHI;
            let ry = f32::cos((f32::pi() * 2.0) + internal_angle) * INV_PHI;
            let xp = mid + cp * rx + axis * ry;
            let xv = normalize(xp - mid);

            let next_axis = normalize(cross(xv, ev));
            let face_vertices = dodecahedron_face_in_axis(next_axis, mid + xv * half_gr * INV_PHI, f32::pi() + start_angle, false);
            vertices.extend(face_vertices);
        }
    }
    vertices
}

/// Create an indexed faceted dodecahedron mesh.
pub fn create_dodecahedron_mesh<D: gfx::Device>(dev: &mut D) -> pmfx::Mesh<D> {
    let h = f32::pi() * 0.8333333 * 0.5 * INV_PHI;
    let mut vertices = dodecahedron_face_in_axis(Vec3f::unit_y(), vec3f(0.0, -h, 0.0), 0.0, true);
    let bottom_vertices = dodecahedron_face_in_axis(-Vec3f::unit_y(), vec3f(0.0, h, 0.0), f32::pi(), true);
    vertices.extend(bottom_vertices);
    create_faceted_mesh_3d(dev, vertices)
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
                texcoord: Vec2f::new(0.0, 0.0),
                normal: n,
                tangent: t,
                bitangent: b,
            },
            Vertex3D {
                position: tip,
                texcoord: Vec2f::new(0.5, 1.0),
                normal: n,
                tangent: t,
                bitangent: b,
            },
            Vertex3D {
                position: np,
                texcoord: Vec2f::new(1.0, 0.0),
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
                texcoord: Vec2f::new(0.0, 1.0),
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
                texcoord: Vec2f::new(0.5, 0.0),
                normal: n,
                tangent: t,
                bitangent: b,
            }
        ];
        vertices.extend(subdivide_triangle(&tri[0], &tri[1], &tri[2], 0, subdivisions));
    }
    vertices
}

/// Create an indexed faceted icosohedron mesh.
pub fn create_icosahedron_mesh<D: gfx::Device>(dev: &mut D) -> pmfx::Mesh<D> {
    // construct from 2 hemi icosahedrons one in the +y and one in -y axis
    let mut vertices = hemi_icosohedron(Vec3f::unit_y(), Vec3f::unit_y() * 0.5, 0.0, 0);
    let bottom_vertices = hemi_icosohedron(-Vec3f::unit_y(), Vec3f::unit_y() * -0.5, f32::pi(), 0);
    vertices.extend(bottom_vertices);
    create_faceted_mesh_3d(dev, vertices)
}

/// Create an indexed faceted icosahedron mesh
pub fn create_icosasphere_mesh<D: gfx::Device>(dev: &mut D, subdivisions: u32) -> pmfx::Mesh<D> {
    // we start from an icosahedron with subdivided faces
    let mut vertices = hemi_icosohedron(Vec3f::unit_y(), Vec3f::unit_y() * 0.5, 0.0, subdivisions);
    let bottom_vertices = hemi_icosohedron(-Vec3f::unit_y(), Vec3f::unit_y() * -0.5, f32::pi(), subdivisions);
    vertices.extend(bottom_vertices);

    // project the points outwards to make a sphere
    for v in &mut vertices {
        v.position = normalize(v.position);
    }

    // keep the facet normals
    for i in (0..vertices.len()).step_by(3) {
        let n = get_triangle_normal(vertices[i].position, vertices[i + 2].position, vertices[i + 1].position);
        let b = normalize(vertices[i].position - vertices[i + 2].position);
        let t = cross(n, b);
        for v in vertices.iter_mut().skip(i).take(3) {
            v.normal = n;
            v.bitangent = b;
            v.tangent = t;
        }
    }

    create_faceted_mesh_3d(dev, vertices)
}

/// Create a custom sphere mesh with segments subdivision, hemi_segments can clip the sphere
/// in different heights, supply `hemi_segments=segments/2` to create a perfect hemi-sphere
/// use cap to cap the cliped sphere or not
pub fn create_sphere_mesh_truncated<D: gfx::Device>(dev: &mut D, segments: usize, hemi_segments: usize, cap: bool) -> pmfx::Mesh<D> {
    let (vertices, indices) = create_sphere_vertices(segments, 0, hemi_segments, cap);
    create_mesh_3d(dev, vertices, indices)
}

/// Create an indexed smooth sphere with subdivided icosophere vertices and smooth normals
pub fn create_sphere_mesh<D: gfx::Device>(dev: &mut D, segments: usize) -> pmfx::Mesh<D> {
    create_sphere_mesh_truncated(dev, segments, segments, false)
}

/// Create an `segments` sided prism, if `smooth` the prism is a cylinder with smooth normals
/// convert to a trapezoid using `taper` with a value between 0-1 to taper the top cap inward where 1 is no taper and 0 makes a pyramid
pub fn create_prism_mesh<D: gfx::Device>(dev: &mut D, segments: usize, smooth: bool, cap: bool, height: f32, taper: f32) -> pmfx::Mesh<D> {
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

        bottom_points.push(points[i] - Vec3f::unit_y() * height);
        top_points.push(points[i] * taper + Vec3f::unit_y() * height);
    }

    //
    // Vertices
    //

    // bottom ring
    for i in 0..vertex_segments {
        let u = 0.5 + atan2(bottom_points[i].z, bottom_points[i].x) / f32::two_pi();
        let u = if i == segments { 0.0 } else { u };
        let bt = cross(points[i], tangents[i]);
        vertices.push(Vertex3D{
            position: bottom_points[i],
            normal: points[i],
            tangent: tangents[i],
            bitangent: bt,
            texcoord: Vec2f::new((1.0 - u) * 3.0, 1.0)
        });
    }

    // top ring
    for i in 0..vertex_segments {
        let u = 0.5 + atan2(top_points[i].z, top_points[i].x) / f32::two_pi();
        let u = if i == segments { 0.0 } else { u };
        let bt = cross(points[i], tangents[i]);
        vertices.push(Vertex3D{
            position: top_points[i],
            normal: points[i],
            tangent: tangents[i],
            bitangent: bt,
            texcoord: Vec2f::new((1.0 - u) * 3.0, 0.0)
        });
    }
        
    // bottom face
    for point in bottom_points.iter().take(vertex_segments) {
        vertices.push(Vertex3D{
            position: *point,
            normal: -Vec3f::unit_y(),
            tangent: Vec3f::unit_x(),
            bitangent: Vec3f::unit_z(),
            texcoord: Vec2f::new(-point.x, -point.z) * 0.5 + 0.5
        });
    }

    // top face
    for point in top_points.iter().take(vertex_segments) {
        vertices.push(Vertex3D{
            position: *point,
            normal: Vec3f::unit_y(),
            tangent: Vec3f::unit_x(),
            bitangent: Vec3f::unit_z(),
            texcoord: Vec2f::new(point.x, -point.z) * 0.5 + 0.5
        });
    }

    // centre points
    vertices.push(Vertex3D{
        position: -Vec3f::unit_y() * height,
        normal: -Vec3f::unit_y(),
        tangent: Vec3f::unit_x(),
        bitangent: Vec3f::unit_z(),
        texcoord: Vec2f::point_five()
    });
    let centre_bottom = vertices.len()-1;

    vertices.push(Vertex3D{
        position: Vec3f::unit_y() * height,
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
        let mut triangle_vertices = Vec::new();
        if taper != 1.0 {
            // 4-tris per segment, trapezoid
            for i in 0..segments {
                let bottom = i;
                let top = i + vertex_segments;
                let next = i + 1;
                let top_next = i + 1 + vertex_segments;

                // add a mid vertex to distribute the UV's more nicely
                let mut mid = vertices[bottom].clone();
                mid.position = (
                    vertices[bottom].position +
                    vertices[top].position + 
                    vertices[next].position +
                    vertices[top_next].position
                 ) * 0.25;

                mid.texcoord = splat2f(0.5);

                let face_index = triangle_vertices.len();
                triangle_vertices.extend(vec![
                    vertices[bottom].clone(),
                    vertices[top].clone(),
                    mid.clone(),

                    vertices[top].clone(),
                    vertices[top_next].clone(),
                    mid.clone(),

                    vertices[bottom].clone(),
                    mid.clone(),
                    vertices[next].clone(),

                    vertices[next].clone(),
                    mid,
                    vertices[top_next].clone(),
                ]);

                let v = face_index;
                let n = get_triangle_normal(
                    triangle_vertices[v].position, 
                    triangle_vertices[v+2].position, 
                    triangle_vertices[v+1].position
                );

                // set hard face normals
                for vertex in triangle_vertices.iter_mut().skip(face_index) {
                    vertex.normal = n;
                }

                let top_u = 0.0;
                let top_next_u = 1.0;

                triangle_vertices[face_index].texcoord.x = 0.0;
                triangle_vertices[face_index + 1].texcoord.x = top_u;
                triangle_vertices[face_index + 3].texcoord.x = top_u;
                triangle_vertices[face_index + 4].texcoord.x = top_next_u;
                triangle_vertices[face_index + 6].texcoord.x = 0.0;
                triangle_vertices[face_index + 8].texcoord.x = 1.0; 
                triangle_vertices[face_index + 9].texcoord.x = 1.0;
                triangle_vertices[face_index + 11].texcoord.x = top_next_u;
            }
        }
        else {
            // 2 tris per segment (prism)
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
                for vertex in triangle_vertices.iter_mut().skip(face_index) {
                    vertex.normal = n;
                }
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
    create_prism_mesh(dev, segments, true, true, 1.0, 1.0)
}

/// Create an indexed unit cube subdivision mesh instance where the faces are subdivided into 4 smaller quads for `subdivisions` 
pub fn create_cube_subdivision_mesh<D: gfx::Device>(dev: &mut D, subdivisions: u32) -> pmfx::Mesh<D> {
    let vertices = cube_vertices();

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

/// creates a pyramid mesh, if smooth this is essentially a low poly cone with smooth normals, 
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
                texcoord: vec2f((1.0 - u) * 3.0, 1.0),
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
                    texcoord: vec2f(mid, 0.0),
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
                    texcoord: vec2f(0.0, 1.0),
                    normal: n,
                    tangent: segment_vertices[i].tangent,
                    bitangent: segment_vertices[i].bitangent,
                },
                Vertex3D {
                    position: t1,
                    texcoord: vec2f(0.5, 0.0),
                    normal: n,
                    tangent: segment_vertices[i].tangent,
                    bitangent: segment_vertices[i].bitangent,
                },
                Vertex3D {
                    position: t2,
                    texcoord: vec2f(1.0, 1.0),
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

    // stick the sphere cap verts into a buffer to weld the cylinder verts
    let mut weld_positions = Vec::new(); 
    let weld_size = (1.0 / segments as f32) * 0.1;

    // bottom sphere
    let (mut v0, i0) = create_sphere_vertices(segments, 0, segments/2, false);

    for v in &mut v0 {
        v.position += vec3f(0.0, -offset, 0.0);
        weld_positions.push(v.position);
    }

    // top sphere
    let (mut v1, mut i1) = create_sphere_vertices(segments, segments/2, segments, false);    
    for v in &mut v1 {
        v.position += vec3f(0.0, offset, 0.0);
        weld_positions.push(v.position);
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
        v.texcoord.y = 1.0 - v.texcoord.y;

        // look in weld positions
        for w in &weld_positions {
            if dist2(v.position, *w) < weld_size {
                v.position = *w;
                break;
            }
        }
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

    // rotate around up axis and extract some data we can lookup to build vb and ib
    let mut hangle = -f32::pi();
    let angle_step = f32::two_pi() / segments as f32;
    for i in 0..vertex_segments + 1 {
        let x = cos(hangle);
        let y = -sin(hangle);
        
        hangle += angle_step;
        let x2 = cos(hangle);
        let y2 = -sin(hangle);
                
        let p = vec3f(x, 0.0, y);
        let np = vec3f(x2, 0.0, y2);

        let at = normalize(np - p);
        let up = Vec3f::unit_y();
        let right = cross(up, at);
        
        let mut vangle = -f32::pi();
        for j in 0..vertex_segments {
            let vx = cos(vangle) * radius;
            let vy = -sin(vangle) * radius;
            let vv = p + vx * up + vy * right;
              
            let n = normalize(vx * up + vy * right);
            let t = right;
            let bt = up;

            let u = 0.5 + atan2(y, x) / f32::two_pi();
            let u = if i == 0 { 1.0 } else { u };

            let v = 0.5 + atan2(vy, vx) / f32::two_pi();
            let v = if j == 0 { 1.0 } else { v };

            segment_vertices.extend(vec![
                Vertex3D {
                    position: vv,
                    normal: n,
                    tangent: t,
                    bitangent: bt,
                    texcoord: vec2f(1.0 - u, 1.0 - v) * 3.0
                }
            ]);

            vangle += angle_step;
        }
    }

    for i in 0..segments {
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

/// Create a unit smooth helix mesh 
pub fn create_helix_mesh<D: gfx::Device>(dev: &mut D, segments: usize, coils: usize) -> pmfx::Mesh<D> {
    let mut segment_vertices = Vec::new();
    let mut vertices = Vec::new();

    // add an extra segment at the end to make uv's wrap nicely
    let vertex_segments = segments + 1;
    let radius = 0.5;

    // rotate around up axis and extract some data we can lookup to build vb and ib
    let mut hangle = -f32::pi();
    let angle_step = f32::two_pi() / segments as f32;
    let height_step = 1.5 / segments as f32;
    let scale = 2.0 / coils as f32;

    let mut h = -height_step * (segments * 2) as f32;
    for _ in 0..coils {
        let mut uv_hangle = -f32::pi();
        for i in 0..vertex_segments {
            let x = cos(hangle);
            let y = -sin(hangle);
            
            let uvx = cos(uv_hangle);
            let uvy = -sin(uv_hangle);
            uv_hangle += angle_step;
            
            hangle += angle_step;
            let x2 = cos(hangle);
            let y2 = -sin(hangle);
            
            let p = vec3f(x, h, y);
            let np = vec3f(x2, h + angle_step, y2);
            
            let at = normalize(np - p);
            let up = Vec3f::unit_y();
            let right = cross(up, at);
                
            let mut vangle = -f32::pi();
            for j in 0..vertex_segments {
                let vx = cos(vangle) * radius;
                let vy = -sin(vangle) * radius;
                let vv = p + vx * up + vy * right;
                  
                let n = normalize(vx * up + vy * right);
                let t = right;
                let bt = up;
    
                let u = 0.5 + atan2(uvy, uvx) / f32::two_pi();
                let u = if i == 0 { 1.0 } else { u };

                let v = 0.5 + atan2(vy, vx) / f32::two_pi();
                let v = if j == 0 { 1.0 } else { v };

                segment_vertices.extend(vec![
                    Vertex3D {
                        position: vv * scale,
                        normal: n,
                        tangent: t,
                        bitangent: bt,
                        texcoord: vec2f(u, 1.0 - v) * 3.0
                    }
                ]);
    
                vangle += angle_step;
            }

            h += height_step;

            // this adds in an extra (degenerate) loop
            if i == segments {
                hangle -= angle_step;
                h -= height_step;
            } 
        }
    }
    
    for k in 0..coils {
        for i in 0..segments {
            for j in 0..segments {
                let coil_base = vertex_segments * segments * k;
                let base = coil_base + (i * vertex_segments) + j;
                let next_loop = base + 1;
                let next_base = coil_base + ((i + 1) * vertex_segments) + j;
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
    }

    // start cap
    let mut mid_pos = Vec3f::zero();
    for vertex in segment_vertices.iter().take(segments) {
        mid_pos += vertex.position;
    }
    mid_pos /= segments as f32;

    for j in 0..segments {
        let p0 = segment_vertices[j + 1].position;
        let p1 = segment_vertices[j].position;
        vertices.extend(vec![
            Vertex3D {
                position: p0,
                normal: -Vec3f::unit_z(),
                texcoord: normalize(mid_pos.xy() - p0.xy()) * 0.5 + 0.5,
                tangent: Vec3f::unit_x(),
                bitangent: Vec3f::unit_y()
            },
            Vertex3D {
                position: mid_pos,
                normal: -Vec3f::unit_z(),
                texcoord: vec2f(0.5, 0.5),
                tangent: Vec3f::unit_x(),
                bitangent: Vec3f::unit_y()
            },
            Vertex3D {
                position: p1,
                normal: -Vec3f::unit_z(),
                texcoord: normalize(mid_pos.xy() - p1.xy()) * 0.5 + 0.5,
                tangent: Vec3f::unit_x(),
                bitangent: Vec3f::unit_y()
            }
        ]);
    }

    // end cap
    let offset = vertex_segments * segments * coils;
    let mut mid_pos = Vec3f::zero();
    for j in 0..segments {
        mid_pos += segment_vertices[offset + j].position;
    }
    mid_pos /= segments as f32;

    for j in 0..segments {
        let p0 = segment_vertices[offset + j].position;
        let p1 = segment_vertices[offset + j + 1].position;
        vertices.extend(vec![
            Vertex3D {
                position: p0,
                normal: Vec3f::unit_z(),
                texcoord: normalize(mid_pos.xy() - p0.xy()) * 0.5 + 0.5,
                tangent: Vec3f::unit_x(),
                bitangent: Vec3f::unit_y()
            },
            Vertex3D {
                position: mid_pos,
                normal: Vec3f::unit_z(),
                texcoord: vec2f(0.5, 0.5),
                tangent: Vec3f::unit_x(),
                bitangent: Vec3f::unit_y()
            },
            Vertex3D {
                position: p1,
                normal: Vec3f::unit_z(),
                texcoord: normalize(mid_pos.xy() - p1.xy()) * 0.5 + 0.5,
                tangent: Vec3f::unit_x(),
                bitangent: Vec3f::unit_y()
            }
        ]);
    }

    create_faceted_mesh_3d(dev, vertices)
}

/// Creates a chamfer cube mesh with curved edges with `radius` size and `segments` subdivisions
pub fn create_chamfer_cube_mesh<D: gfx::Device>(dev: &mut D, radius: f32, segments: usize) -> pmfx::Mesh<D> {
    let inset = 1.0 - radius;
    let edge_uv_scale = radius;

    // cube verts with inset
    let mut vertices = cube_vertices();

    let insets = [
        vec3f(inset, inset, 1.0),
        vec3f(1.0, inset, inset),
        vec3f(inset, 1.0, inset),
    ];

    for i in 0..vertices.len() {
        let face = i / 8;
        vertices[i].position *= insets[face];
    }

    let mut indices: Vec<usize> = vec![
        0,  2,  1,  2,  0,  3,   // front face
        4,  6,  5,  6,  4,  7,   // back face
        8,  10, 9,  10, 8,  11,  // right face
        12, 14, 13, 14, 12, 15,  // left face
        16, 18, 17, 18, 16, 19,  // top face
        20, 22, 21, 22, 20, 23,  // bottom face
    ];

    // join edges
    let join_edge = |edge_indices: [usize; 4], clamp_axis: usize, vertices: &mut Vec<Vertex3D>, indices: &mut Vec<usize>| {
        let bottom_start = vertices[edge_indices[0]].position;
        let top_start = vertices[edge_indices[1]].position; 
        let top_end = vertices[edge_indices[2]].position;
        let bottom_end = vertices[edge_indices[3]].position;   
        let fsegments = segments as f32;
        let base_index = vertices.len();
        
        let pivot = bottom_start - vertices[edge_indices[0]].normal * radius;
        let top_pivot = top_start - vertices[edge_indices[0]].normal * radius;

        for i in 0..segments+1 {
            let cur = (1.0 / fsegments) * i as f32;
            let v = cur * edge_uv_scale;
    
            // linear lerp
            let lv0 = lerp(bottom_start, bottom_end, cur);
            let lv1 = lerp(top_start, top_end, cur);
    
            // project corner to unit cube (chebyshev_normalize)
            let cur = if cur > 0.5 {
                1.0 - cur
            }
            else {
                cur
            };

            // lerp between square corner and cut corner, forming circle
            let mut p0 = lerp(chebyshev_normalize(lv0), lv0, cur);
            let mut p1 = lerp(chebyshev_normalize(lv1), lv1, cur);
    
            // ..
            p0[clamp_axis] = bottom_start[clamp_axis];
            p1[clamp_axis] = top_start[clamp_axis];

            let n = normalize(p0 - pivot);
            let t = normalize(top_pivot - pivot);
            let bt = cross(n, t);
    
            vertices.extend(
                vec![
                    Vertex3D {
                        position: pivot + normalize(p0 - pivot) * radius,
                        normal: n,
                        tangent: t,
                        bitangent: bt,
                        texcoord: vec2f(0.0, v)
                    },
                    Vertex3D {
                        position: top_pivot + normalize(p1 - top_pivot) * radius,
                        normal: n,
                        tangent: t,
                        bitangent: bt,
                        texcoord: vec2f(1.0, v)
                    }
                ]
            );
        }

        for i in 0..segments {
            let strip_base = base_index + i * 2;
            indices.extend(vec![
                strip_base, strip_base + 1, strip_base + 3, 
                strip_base, strip_base + 3, strip_base + 2
            ]);
        }
    };

    // join sides
    join_edge([1, 2, 10, 11], 1, &mut vertices, &mut indices); // front-right
    join_edge([8, 9, 6, 7], 1, &mut vertices, &mut indices); // right-back
    join_edge([4, 5, 12, 15], 1, &mut vertices, &mut indices); // back-left
    join_edge([13, 14, 0, 3], 1, &mut vertices, &mut indices); // left-front

    // join top
    let ft_loop_start = vertices.len();
    join_edge([2, 3, 17, 18], 0, &mut vertices, &mut indices); // front-top

    let rt_loop_start = vertices.len();
    join_edge([9, 10, 19, 18], 2, &mut vertices, &mut indices); // right-top

    let bt_loop_start = vertices.len();
    join_edge([5, 6, 16, 19], 0, &mut vertices, &mut indices); // back-top

    let lt_loop_start = vertices.len();
    join_edge([14, 15, 16, 17], 2, &mut vertices, &mut indices); // left-top

    // join bottom
    let fb_loop_start = vertices.len();
    join_edge([0, 1, 22, 23], 0, &mut vertices, &mut indices); // front-bottom

    let rb_loop_start = vertices.len();
    join_edge([11, 8, 22, 21], 2, &mut vertices, &mut indices); // right-bottom

    let bb_loop_start = vertices.len();
    join_edge([7, 4, 21, 20], 0, &mut vertices, &mut indices); // back-bottom

    let lb_loop_start = vertices.len();
    join_edge([12, 13, 20, 23], 2, &mut vertices, &mut indices); // left-bottom

    let join_corner = |start_loop_start: usize, end_loop_start: usize, vertices: &mut Vec<Vertex3D>, indices: &mut Vec<usize>| {
        let base_index = vertices.len();        
        let fsegments = segments as f32;
        let centre = vertices[start_loop_start].position - vertices[start_loop_start].normal * radius;

        for j in 0..segments+1 {
            let joffset = j * 2;
            let start = vertices[start_loop_start + joffset].position;
            let end = vertices[end_loop_start + 1 + joffset].position;
            let v = (1.0 / fsegments) * j as f32;
    
            for i in 0..segments+1 {
                let u = (1.0 / fsegments) * i as f32;
                let next = (1.0 / fsegments) * (i+1) as f32;
    
                let lv0 = lerp(start, end, u);
                let lv1 = lerp(start, end, next);

                let p = centre + normalize(lv0 - centre) * radius;
                let nextp = centre + normalize(lv1 - centre) * radius;
    
                let n = normalize(lv0 - centre);
                let mut t = normalize(p - nextp);
                
                if j > segments - 1 {
                    t = Vec3f::unit_x();
                }
                let bt = cross(n, t);

                vertices.extend(vec![
                    Vertex3D {
                        position: p,
                        normal: n,
                        tangent: t,
                        bitangent: bt,
                        texcoord: vec2f(u, v) * edge_uv_scale
                    },
                ]);
            }
        }
    
        for j in 0..segments {
            let ycur = base_index + j * (segments+1);
            let ynext = base_index + (j+1) * (segments+1);
            for i in 0..segments {
                let xcur = ycur + i;
                let xnext = ynext + i;
                indices.extend(vec![
                    xcur, xnext, xcur + 1,
                    xnext, xnext+1, xcur + 1
                ]);
            }
        }
    };

    join_corner(ft_loop_start, rt_loop_start, &mut vertices, &mut indices);
    join_corner(rt_loop_start, bt_loop_start, &mut vertices, &mut indices);
    join_corner(bt_loop_start, lt_loop_start, &mut vertices, &mut indices);
    join_corner(lt_loop_start, ft_loop_start, &mut vertices, &mut indices);
    join_corner(rb_loop_start, fb_loop_start, &mut vertices, &mut indices);
    join_corner(bb_loop_start, rb_loop_start, &mut vertices, &mut indices);
    join_corner(lb_loop_start, bb_loop_start, &mut vertices, &mut indices);
    join_corner(fb_loop_start, lb_loop_start, &mut vertices, &mut indices);

    create_mesh_3d(dev, vertices, indices)
}

/// Create an `segments` sided prism tude, if `smooth` the prism is a cylinder with smooth normals
/// convert to a trapezoid using `taper` with a value between 0-1 to taper the top cap inward where 1 is no taper and 0 makes a pyramid
/// use thickness to control the size of the inner hole 
pub fn create_tube_prism_mesh<D: gfx::Device>(
    dev: &mut D, segments: usize, trunc_start: usize, trunc_end: usize, smooth: bool, cap: bool, height: f32, thickness: f32, taper: f32) -> pmfx::Mesh<D> {
    let axis = Vec3f::unit_y();
    let right = Vec3f::unit_x();
    let up = cross(axis, right);
    let right = cross(axis, up);

    // add an extra segment at the end to make uv's wrap nicely
    let vertex_segments = segments + 1;

    let prism_vertices = |radius: f32, flip: f32| -> (Vec<Vertex3D>, Vec<Vertex3D>) {
        let mut vertices = Vec::new();
        let mut cap_vertices = Vec::new();
        let mut points = Vec::new();
        let mut bottom_points = Vec::new();
        let mut top_points = Vec::new();
        let mut tangents = Vec::new();
    
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
    
            points.push(v1 * radius);
            tangents.push(v2 - v1);
    
            bottom_points.push(points[i] - Vec3f::unit_y() * height);
            top_points.push(points[i] * taper + Vec3f::unit_y() * height);
        }
    
        // bottom ring
        for i in 0..vertex_segments {
            let u = 0.5 + atan2(bottom_points[i].z, bottom_points[i].x) / f32::two_pi();
            let u = if i == segments { 0.0 } else { u };
            let bt = cross(tangents[i], points[i]);
            vertices.push(Vertex3D{
                position: bottom_points[i],
                normal: points[i] * flip,
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
                normal: points[i] * flip,
                tangent: tangents[i],
                bitangent: bt,
                texcoord: Vec2f::new(u * 3.0, 1.0)
            });
        }

        // bottom cap
        for i in 0..vertex_segments {
            cap_vertices.push(Vertex3D{
                position: bottom_points[i],
                normal: -Vec3f::unit_y(),
                tangent: Vec3f::unit_x(),
                bitangent: Vec3f::unit_z(),
                texcoord: bottom_points[i].xz() * 0.5 + 0.5
            });
        }

        // top cap
        for i in 0..vertex_segments {
            cap_vertices.push(Vertex3D{
                position: top_points[i],
                normal: Vec3f::unit_y(),
                tangent: Vec3f::unit_x(),
                bitangent: Vec3f::unit_z(),
                texcoord: top_points[i].xz() * 0.5 + 0.5
            });
        }

        (vertices, cap_vertices)
    };

    let end_cap = |loop_start: usize, v: &Vec<Vertex3D>, inner_offset: usize, flip: f32| -> Vec<Vertex3D> {
        // start
        let bottom = loop_start;
        let top = loop_start + vertex_segments;
        let inner_bottom = bottom + inner_offset;
        let inner_top = top + inner_offset;

        let mut verts = vec![
            v[bottom].clone(),
            v[top].clone(),
            v[inner_bottom].clone(),
            v[inner_top].clone(),
        ];

        let b = normalize(verts[1].position - verts[0].position);
        let t = normalize(verts[2].position - verts[0].position);
        let n = cross(b, t) * flip;

        for v in &mut verts {
            v.normal = n;
            v.tangent = t;
            v.bitangent = b;
        }

        verts[0].texcoord = Vec2f::zero();
        verts[1].texcoord = vec2f(0.0, 1.0);
        verts[2].texcoord = vec2f(1.0 - thickness, 0.0);
        verts[3].texcoord = vec2f(1.0 - thickness, 1.0);

        verts
    };

    let (outer_vertices, outer_cap_vertices) = prism_vertices(1.0, 1.0);
    let (inner_vertices, inner_cap_vertices) = prism_vertices(thickness, -1.0);

    if smooth {
        let mut indices = Vec::new();
        let mut vertices = Vec::new();
    
        vertices.extend(outer_vertices);
    
        let outer_cap_offset = vertices.len();
        vertices.extend(outer_cap_vertices);
    
        let inner_offset = vertices.len();
        vertices.extend(inner_vertices);
    
        let inner_cap_offset = vertices.len();
        vertices.extend(inner_cap_vertices);

        // sides
        for i in trunc_start..trunc_end {
            let bottom = i;
            let top = i + vertex_segments;
            let next = i + 1;
            let top_next = i + 1 + vertex_segments;

            // outer
            indices.extend(vec![
                bottom,
                top,
                next,
                top,
                top_next,
                next
            ]);

            // inner
            indices.extend(vec![
                bottom + inner_offset,
                next + inner_offset,
                top + inner_offset,
                top + inner_offset,
                next + inner_offset,
                top_next + inner_offset,
            ]);

            // caps
            if cap {
                let inner_bottom = bottom + inner_cap_offset;
                let inner_top = top + inner_cap_offset;
                let inner_top_next = top_next + inner_cap_offset;
                let inner_bottom_next = next + inner_cap_offset;

                let outer_bottom = bottom + outer_cap_offset;
                let outer_top = top + outer_cap_offset;
                let outer_top_next = top_next + outer_cap_offset;
                let outer_bottom_next = next + outer_cap_offset;

                indices.extend(vec![
                    outer_top,
                    inner_top,
                    inner_top_next,
                    outer_top,
                    inner_top_next,
                    outer_top_next
                ]);

                indices.extend(vec![
                    inner_bottom,
                    outer_bottom,
                    outer_bottom_next,
                    inner_bottom,
                    outer_bottom_next,
                    inner_bottom_next
                ]);
            }
        }

        // end caps
        if trunc_start != 0 || trunc_end != segments {
            let start_verts = end_cap(trunc_start, &vertices, inner_offset, 1.0);
            let start_offset = vertices.len();
            vertices.extend(start_verts);

            indices.extend(vec![
                start_offset,
                start_offset + 3,
                start_offset + 1,
                start_offset,
                start_offset + 2,
                start_offset + 3,
            ]);

            let end_verts = end_cap(trunc_end, &vertices, inner_offset, -1.0);
            let end_offset = vertices.len();
            vertices.extend(end_verts);

            // end
            indices.extend(vec![
                end_offset,
                end_offset + 1,
                end_offset + 3,
                end_offset,
                end_offset + 3,
                end_offset + 2,
            ]);
        }

        create_mesh_3d(dev, vertices, indices)
    }
    else {
        // 2 tris per segment
        let mut triangle_vertices = Vec::new();
        for i in trunc_start..trunc_end {
            let bottom = i;
            let top = i + vertex_segments;
            let next = i + 1;
            let top_next = i + 1 + vertex_segments;

            let face_index = triangle_vertices.len();
            triangle_vertices.extend(vec![
                outer_vertices[bottom].clone(),
                outer_vertices[top].clone(),
                outer_vertices[next].clone(),
                outer_vertices[top].clone(),
                outer_vertices[top_next].clone(),
                outer_vertices[next].clone()
            ]);

            let v = face_index;
            let n = get_triangle_normal(
                triangle_vertices[v].position, 
                triangle_vertices[v+2].position, 
                triangle_vertices[v+1].position
            );

            // set hard face normals
            for vertex in triangle_vertices.iter_mut().skip(face_index) {
                vertex.normal = n;
            }
        }

        for i in trunc_start..trunc_end {
            let bottom = i;
            let top = i + vertex_segments;
            let next = i + 1;
            let top_next = i + 1 + vertex_segments;

            let face_index = triangle_vertices.len();
            triangle_vertices.extend(vec![
                inner_vertices[bottom].clone(),
                inner_vertices[next].clone(),
                inner_vertices[top].clone(),
                inner_vertices[top].clone(),
                inner_vertices[next].clone(),
                inner_vertices[top_next].clone()
            ]);

            let v = face_index;
            let n = get_triangle_normal(
                triangle_vertices[v].position, 
                triangle_vertices[v+2].position,
                triangle_vertices[v+1].position,
            );

            // set hard face normals
            for vertex in triangle_vertices.iter_mut().skip(face_index) {
                vertex.normal = n;
            }
        }

        // top / bottom cap
        if cap {
            for i in trunc_start..trunc_end {
                let bottom = i;
                let top = i + vertex_segments;
                let next = i + 1;
                let top_next = i + 1 + vertex_segments;

                triangle_vertices.extend(vec![
                    outer_cap_vertices[top].clone(),
                    inner_cap_vertices[top].clone(),
                    inner_cap_vertices[top_next].clone(),
                    outer_cap_vertices[top].clone(),
                    inner_cap_vertices[top_next].clone(),
                    outer_cap_vertices[top_next].clone()
                ]);

                triangle_vertices.extend(vec![
                    inner_cap_vertices[bottom].clone(),
                    outer_cap_vertices[bottom].clone(),
                    outer_cap_vertices[next].clone(),
                    inner_cap_vertices[bottom].clone(),
                    outer_cap_vertices[next].clone(),
                    inner_cap_vertices[next].clone()
                ]);
            }    
        }

        // end cap
        if trunc_start != 0 || trunc_end != segments {
            let mut vertices = outer_vertices.to_vec();
            let inner_offset = vertices.len();
            vertices.extend(inner_vertices);

            let start_verts = end_cap(trunc_start, &vertices, inner_offset, 1.0);
            triangle_vertices.extend(vec![
                start_verts[0].clone(),
                start_verts[3].clone(),
                start_verts[1].clone(),
                start_verts[0].clone(),
                start_verts[2].clone(),
                start_verts[3].clone(),
            ]);

            let end_verts = end_cap(trunc_end, &vertices, inner_offset, -1.0);
            triangle_vertices.extend(vec![
                end_verts[0].clone(),
                end_verts[1].clone(),
                end_verts[3].clone(),
                end_verts[0].clone(),
                end_verts[3].clone(),
                end_verts[2].clone(),
            ]);
        }

        create_faceted_mesh_3d(dev, triangle_vertices)
    }
}

fn cubic_interpolate(p1: Vec3f, p2: Vec3f, p3: Vec3f, p4: Vec3f, t: f32) -> Vec3f {
    p1 * (1.0 - t) * (1.0 - t) * (1.0 - t) +
    p2 * 3.0 * t * (1.0 - t) * (1.0 - t) +
    p3 * 3.0 * t * t * (1.0 - t) +
    p4 * t * t * t
}

fn cubic_tangent(p1: Vec3f, p2: Vec3f, p3: Vec3f, p4: Vec3f, t: f32) -> Vec3f {
    p1 * (-1.0 + 2.0 * t - t * t) +
    p2 * (1.0 - 4.0 * t + 3.0 * t * t) +
    p3 * (2.0 * t - 3.0 * t * t) +
    p4 * (t * t)
}

/// Create a unit utah teapot mesh
pub fn create_teapot_mesh<D: gfx::Device>(dev: &mut D, tessellation: usize) -> pmfx::Mesh<D> {
    // this code was ported from DirectXTK https://github.com/microsoft/DirectXTK
    struct TeapotPatch {
        mirror_z: bool,
        indices: [u32; 16]
    }

    let patches = vec![
        // rim
        TeapotPatch {
            mirror_z: true,
            indices: [
                102, 103, 104, 105, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15
            ]
        },

        // body
        TeapotPatch {
            mirror_z: true,
            indices: [
                12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27
            ]
        },
        TeapotPatch {
            mirror_z: true,
            indices: [
                24, 25, 26, 27, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40
            ]
        },

        // lid
        TeapotPatch {
            mirror_z: true,
            indices: [
                96, 96, 96, 96, 97, 98, 99, 100, 101, 101, 101, 101, 0, 1, 2, 3
            ]
        },
        TeapotPatch {
            mirror_z: true,
            indices: [
                0, 1, 2, 3, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117
            ]
        },

        // handle 
        TeapotPatch {
            mirror_z: false,
            indices: [
                41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56
            ]
        },
        TeapotPatch {
            mirror_z: false,
            indices: [
                53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 28, 65, 66, 67
            ]
        },

        // spout 
        TeapotPatch {
            mirror_z: false,
            indices: [
                68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83
            ]
        },
        TeapotPatch {
            mirror_z: false,
            indices: [
                80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95
            ]
        },

        // bottom
        TeapotPatch {
            mirror_z: true,
            indices: [
                118, 118, 118, 118, 124, 122, 119, 121, 123, 126, 125, 120, 40, 39, 38, 37
            ]
        }
    ];

    let control_points = vec![
        vec3f(0.0, 0.345, -0.05),
        vec3f(-0.028, 0.345, -0.05),
        vec3f(-0.05, 0.345, -0.028),
        vec3f(-0.05, 0.345, -0.0),
        vec3f(0.0, 0.3028125, -0.334375),
        vec3f(-0.18725, 0.3028125, -0.334375),
        vec3f(-0.334375, 0.3028125, -0.18725),
        vec3f(-0.334375, 0.3028125, -0.0),
        vec3f(0.0, 0.3028125, -0.359375),
        vec3f(-0.20125, 0.3028125, -0.359375),
        vec3f(-0.359375, 0.3028125, -0.20125),
        vec3f(-0.359375, 0.3028125, -0.0),
        vec3f(0.0, 0.27, -0.375),
        vec3f(-0.21, 0.27, -0.375),
        vec3f(-0.375, 0.27, -0.21),
        vec3f(-0.375, 0.27, -0.0),
        vec3f(0.0, 0.13875, -0.4375),
        vec3f(-0.245, 0.13875, -0.4375),
        vec3f(-0.4375, 0.13875, -0.245),
        vec3f(-0.4375, 0.13875, -0.0),
        vec3f(0.0, 0.007499993, -0.5),
        vec3f(-0.28, 0.007499993, -0.5),
        vec3f(-0.5, 0.007499993, -0.28),
        vec3f(-0.5, 0.007499993, -0.0),
        vec3f(0.0, -0.105, -0.5),
        vec3f(-0.28, -0.105, -0.5),
        vec3f(-0.5, -0.105, -0.28),
        vec3f(-0.5, -0.105, -0.0),
        vec3f(0.0, -0.105, 0.5),
        vec3f(0.0, -0.2175, -0.5),
        vec3f(-0.28, -0.2175, -0.5),
        vec3f(-0.5, -0.2175, -0.28),
        vec3f(-0.5, -0.2175, -0.0),
        vec3f(0.0, -0.27375, -0.375),
        vec3f(-0.21, -0.27375, -0.375),
        vec3f(-0.375, -0.27375, -0.21),
        vec3f(-0.375, -0.27375, -0.0),
        vec3f(0.0, -0.2925, -0.375),
        vec3f(-0.21, -0.2925, -0.375),
        vec3f(-0.375, -0.2925, -0.21),
        vec3f(-0.375, -0.2925, -0.0),
        vec3f(0.0, 0.17625, 0.4),
        vec3f(-0.075, 0.17625, 0.4),
        vec3f(-0.075, 0.2325, 0.375),
        vec3f(0.0, 0.2325, 0.375),
        vec3f(0.0, 0.17625, 0.575),
        vec3f(-0.075, 0.17625, 0.575),
        vec3f(-0.075, 0.2325, 0.625),
        vec3f(0.0, 0.2325, 0.625),
        vec3f(0.0, 0.17625, 0.675),
        vec3f(-0.075, 0.17625, 0.675),
        vec3f(-0.075, 0.2325, 0.75),
        vec3f(0.0, 0.2325, 0.75),
        vec3f(0.0, 0.12, 0.675),
        vec3f(-0.075, 0.12, 0.675),
        vec3f(-0.075, 0.12, 0.75),
        vec3f(0.0, 0.12, 0.75),
        vec3f(0.0, 0.06375, 0.675),
        vec3f(-0.075, 0.06375, 0.675),
        vec3f(-0.075, 0.007499993, 0.75),
        vec3f(0.0, 0.007499993, 0.75),
        vec3f(0.0, -0.04875001, 0.625),
        vec3f(-0.075, -0.04875001, 0.625),
        vec3f(-0.075, -0.09562501, 0.6625),
        vec3f(0.0, -0.09562501, 0.6625),
        vec3f(-0.075, -0.105, 0.5),
        vec3f(-0.075, -0.18, 0.475),
        vec3f(0.0, -0.18, 0.475),
        vec3f(0.0, 0.02624997, -0.425),
        vec3f(-0.165, 0.02624997, -0.425),
        vec3f(-0.165, -0.18, -0.425),
        vec3f(0.0, -0.18, -0.425),
        vec3f(0.0, 0.02624997, -0.65),
        vec3f(-0.165, 0.02624997, -0.65),
        vec3f(-0.165, -0.12375, -0.775),
        vec3f(0.0, -0.12375, -0.775),
        vec3f(0.0, 0.195, -0.575),
        vec3f(-0.0625, 0.195, -0.575),
        vec3f(-0.0625, 0.17625, -0.6),
        vec3f(0.0, 0.17625, -0.6),
        vec3f(0.0, 0.27, -0.675),
        vec3f(-0.0625, 0.27, -0.675),
        vec3f(-0.0625, 0.27, -0.825),
        vec3f(0.0, 0.27, -0.825),
        vec3f(0.0, 0.28875, -0.7),
        vec3f(-0.0625, 0.28875, -0.7),
        vec3f(-0.0625, 0.2934375, -0.88125),
        vec3f(0.0, 0.2934375, -0.88125),
        vec3f(0.0, 0.28875, -0.725),
        vec3f(-0.0375, 0.28875, -0.725),
        vec3f(-0.0375, 0.298125, -0.8625),
        vec3f(0.0, 0.298125, -0.8625),
        vec3f(0.0, 0.27, -0.7),
        vec3f(-0.0375, 0.27, -0.7),
        vec3f(-0.0375, 0.27, -0.8),
        vec3f(0.0, 0.27, -0.8),
        vec3f(0.0, 0.4575, -0.0),
        vec3f(0.0, 0.4575, -0.2),
        vec3f(-0.1125, 0.4575, -0.2),
        vec3f(-0.2, 0.4575, -0.1125),
        vec3f(-0.2, 0.4575, -0.0),
        vec3f(0.0, 0.3825, -0.0),
        vec3f(0.0, 0.27, -0.35),
        vec3f(-0.196, 0.27, -0.35),
        vec3f(-0.35, 0.27, -0.196),
        vec3f(-0.35, 0.27, -0.0),
        vec3f(0.0, 0.3075, -0.1),
        vec3f(-0.056, 0.3075, -0.1),
        vec3f(-0.1, 0.3075, -0.056),
        vec3f(-0.1, 0.3075, -0.0),
        vec3f(0.0, 0.3075, -0.325),
        vec3f(-0.182, 0.3075, -0.325),
        vec3f(-0.325, 0.3075, -0.182),
        vec3f(-0.325, 0.3075, -0.0),
        vec3f(0.0, 0.27, -0.325),
        vec3f(-0.182, 0.27, -0.325),
        vec3f(-0.325, 0.27, -0.182),
        vec3f(-0.325, 0.27, -0.0),
        vec3f(0.0, -0.33, -0.0),
        vec3f(-0.1995, -0.33, -0.35625),
        vec3f(0.0, -0.31125, -0.375),
        vec3f(0.0, -0.33, -0.35625),
        vec3f(-0.35625, -0.33, -0.1995),
        vec3f(-0.375, -0.31125, -0.0),
        vec3f(-0.35625, -0.33, -0.0),
        vec3f(-0.21, -0.31125, -0.375),
        vec3f(-0.375, -0.31125, -0.21),
    ];

    let create_patch_indices = |tessellation: usize, is_mirrored: bool,  index_offset: usize| -> Vec<usize> {
        let stride = tessellation + 1;
        let mut indices = Vec::new();
        for i in 0..tessellation {
            for j in 0..tessellation {
                let mut arr = [
                    i * stride + j,
                    (i + 1) * stride + j,
                    (i + 1) * stride + j + 1,

                    i * stride + j,
                    (i + 1) * stride + j + 1,
                    i * stride + j + 1,
                ];

                if is_mirrored {
                    arr.reverse();
                }

                for ii in arr {
                    indices.push(ii + index_offset)
                }
            }
        }
        indices
    };

    let create_patch_vertices = |patch: Vec<Vec3f>, tessellation: usize, is_mirrored: bool| -> Vec<Vertex3D> {
        let mut vertices = Vec::new();
        for i in 0..tessellation+1 {
            let u = i as f32 / tessellation as f32;
            for j in 0..tessellation+1 {
                let v = j as f32 / tessellation as f32;

                // Perform four horizontal bezier interpolations between the control points of this patch.
                let p1 = cubic_interpolate(patch[0], patch[1], patch[2], patch[3], u);
                let p2 = cubic_interpolate(patch[4], patch[5], patch[6], patch[7], u);
                let p3 = cubic_interpolate(patch[8], patch[9], patch[10], patch[11], u);
                let p4 = cubic_interpolate(patch[12], patch[13], patch[14], patch[15], u);

                // Perform a vertical interpolation between the results of the
                // previous horizontal interpolations, to compute the position.
                let pos = cubic_interpolate(p1, p2, p3, p4, v);

                // Perform another four bezier interpolations between the control
                // points, but this time vertically rather than horizontally.
                let q1 = cubic_interpolate(patch[0], patch[4], patch[8], patch[12], v);
                let q2 = cubic_interpolate(patch[1], patch[5], patch[9], patch[13], v);
                let q3 = cubic_interpolate(patch[2], patch[6], patch[10], patch[14], v);
                let q4 = cubic_interpolate(patch[3], patch[7], patch[11], patch[15], v);

                let mut tangent1 = cubic_tangent(p1, p2, p3, p4, v);
                let mut tangent2 = cubic_tangent(q1, q2, q3, q4, u);
                let mut normal = cross(tangent1, tangent2);

                if is_mirrored {
                    normal = -normal;
                }

                // this is a hack, because the bezier patches are not very well constructed
                if approx(normal, Vec3f::zero(), 0.001) {
                    normal = Vec3f::unit_y();
                    tangent1 = Vec3f::unit_x();
                    tangent2 = Vec3f::unit_z();
                    if pos.y < 0.0 {
                        normal *= -1.0;
                        tangent1 *= -1.0;
                    }
                }

                let uv = Vec2f {
                    x: if is_mirrored { 1.0 - u } else { u },
                    y: v
                };

                vertices.push(Vertex3D { 
                    position: pos, 
                    texcoord: uv, 
                    normal: normalize(normal), 
                    tangent: tangent1, 
                    bitangent: tangent2
                });
            }
        }
        vertices
    };

    let tessalate_patch = |patch: &TeapotPatch, tesselation: usize, scale: Vec3f, is_mirrored: bool, index_offset: usize| -> (Vec<Vertex3D>, Vec<usize>) {

        let mut patch_verts = Vec::new();
        for i in 0..16 {
            patch_verts.push(control_points[patch.indices[i] as usize] * scale * 2.0);
        } 

        (create_patch_vertices(patch_verts, tesselation, is_mirrored), create_patch_indices(tesselation, is_mirrored, index_offset))
    };

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let mut lid_index_offset = 0;
    let mut spout_index_offset = 0;
    let mut ii = 0;

    for patch in &patches {
        // grab the offset to the lid to join it later and the spount to fill in the gap
        if ii == 4 {
            lid_index_offset = vertices.len();
        }
        else if ii == 8 {
            spout_index_offset = vertices.len();
        }
        ii += 1;

        let (patch_vertices, patch_indices) = tessalate_patch(patch, tessellation, Vec3f::one(), false, vertices.len());
        vertices.extend(patch_vertices);
        indices.extend(patch_indices);

        let (patch_vertices, patch_indices) = tessalate_patch(patch, tessellation, vec3f(-1.0, 1.0, 1.0), true, vertices.len());
        vertices.extend(patch_vertices);
        indices.extend(patch_indices);

        if patch.mirror_z {
            let (patch_vertices, patch_indices) = tessalate_patch(patch, tessellation, vec3f(1.0, 1.0, -1.0), true, vertices.len());
            vertices.extend(patch_vertices);
            indices.extend(patch_indices);

            let (patch_vertices, patch_indices) = tessalate_patch(patch, tessellation, vec3f(-1.0, 1.0, -1.0), false, vertices.len());
            vertices.extend(patch_vertices);
            indices.extend(patch_indices);
        }
    }

    // join the lid and the rim
    let stride = tessellation+1;
    for i in 0..tessellation {
        // a quad per tesselated patch
        for q in 0..4 {
            let v = i + q * stride;
            let lid = lid_index_offset + tessellation + v * stride;
            let rim = v * stride;
            let lid_next = lid_index_offset + tessellation + (v+1) * stride;
            let rim_next = (v+1) * stride;

            // we need to flip te winding on the second 2 quadrants
            if q == 0 || q == 3 {
                indices.extend(vec![
                    rim,
                    lid,
                    lid_next,
                    rim,
                    lid_next,
                    rim_next
                ]);
            }
            else {
                indices.extend(vec![
                    rim,
                    lid_next,
                    lid,
                    rim,
                    rim_next,
                    lid_next
                ]);
            }
        }
    }

    // plug a gap in the spout
    let (mut spout_verts, _) = tessalate_patch(&patches[8], tessellation, Vec3f::one(), false, vertices.len());
    let (spout_verts2, _) = tessalate_patch(&patches[8], tessellation, vec3f(-1.0, 1.0, 1.0), true, vertices.len());
    spout_verts.extend(spout_verts2);

    // get a central vertex pos
    let mut ii = 0;
    let mut loop_count = 0;
    let mut spout_centre = Vec3f::zero();
    for v in spout_verts {
        ii += 1;
        if (ii-1) % stride != tessellation {
            continue;
        }
        spout_centre += v.position;
        loop_count += 1;
    }
    spout_centre /= loop_count as f32;

    let spount_centre_index = vertices.len();
    vertices.push(Vertex3D{
        position: spout_centre,
        texcoord: normalize(spout_centre.xz()) * 0.5 + 0.5,
        normal: Vec3f::unit_y(),
        tangent: Vec3f::unit_x(),
        bitangent: Vec3f::unit_z()
    });

    for i in 0..tessellation {
        for q in 0..2 {
            let v = i + q * stride;
            let vi = spout_index_offset + tessellation + v * stride;
            let vn = spout_index_offset + tessellation + (v+1) * stride;
            if q == 1 {
                indices.extend(vec![
                    vi,
                    spount_centre_index,
                    vn
                ])
            }
            else {
                indices.extend(vec![
                    vi,
                    vn,
                    spount_centre_index
                ])
            }
        }
    }

    create_mesh_3d(dev, vertices, indices)
}