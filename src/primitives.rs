use crate::gfx;
use crate::pmfx;

use maths_rs::*;
use maths_rs::vec::*;
use maths_rs::Vec2f;
use maths_rs::Vec3f;
use maths_rs::num::*;

pub struct Vertex3D {
    pub position: Vec3f,
    pub texcoord: Vec2f,
    pub normal: Vec3f,
    pub tangent: Vec3f,
    pub bitangent: Vec3f,
}

pub struct Vertex2D {
    pub position: Vec2f,
    pub texcoord: Vec2f,
}

const INV_PHI : f32 = 0.61803398875;

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
                usage: gfx::BufferUsage::Vertex,
                cpu_access: gfx::CpuAccessFlags::NONE,
                num_elements: 4,
                format: gfx::Format::Unknown,
                stride: std::mem::size_of::<Vertex2D>() 
            }, 
            Some(vertices.as_slice())
        ).unwrap(),
        ib: dev.create_buffer(&gfx::BufferInfo {
            usage: gfx::BufferUsage::Index,
            cpu_access: gfx::CpuAccessFlags::NONE,
            num_elements: 6,
            format: gfx::Format::R16u,
            stride: std::mem::size_of::<u16>()
            },
            Some(indices.as_slice())
        ).unwrap(),
        num_indices: 6
    } 
}

/// Create an indexed unit billboard quad mesh instance with the front face pointing +z 
pub fn create_billboard_mesh<D: gfx::Device>(dev: &mut D) -> pmfx::Mesh<D> {
    // quad veritces
    let vertices: Vec<Vertex3D> = vec![
        // front face
        Vertex3D {
            position: vec3f(-1.0, -1.0, 0.0),
            texcoord: vec2f(-1.0, -1.0),
            normal: vec3f(0.0, 0.0, 1.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(1.0, -1.0, 0.0),
            texcoord: vec2f(1.0, -1.0),
            normal: vec3f(0.0, 0.0, 1.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(1.0, 1.0, 0.0),
            texcoord: vec2f(1.0, 1.0),
            normal: vec3f(0.0, 0.0, 1.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(-1.0, 1.0, 0.0),
            texcoord: vec2f(-1.0, 1.0),
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
                usage: gfx::BufferUsage::Vertex,
                cpu_access: gfx::CpuAccessFlags::NONE,
                num_elements: 4,
                format: gfx::Format::Unknown,
                stride: std::mem::size_of::<Vertex3D>() 
            }, 
            Some(vertices.as_slice())
        ).unwrap(),
        ib: dev.create_buffer(&gfx::BufferInfo {
            usage: gfx::BufferUsage::Index,
            cpu_access: gfx::CpuAccessFlags::NONE,
            num_elements: 6,
            format: gfx::Format::R16u,
            stride: std::mem::size_of::<u16>()
            },
            Some(indices.as_slice())
        ).unwrap(),
        num_indices: 6
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
                    texcoord: vec2f(-1.0, -1.0),
                    normal: Vec3f::unit_y(),
                    tangent: vec3f(1.0, 0.0, 0.0),
                    bitangent: vec3f(0.0, 1.0, 0.0),
                },
                Vertex3D {
                    position: vec3f(pos.x + increment, 0.0, pos.z),
                    texcoord: vec2f(1.0, -1.0),
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
                    texcoord: vec2f(-1.0, 1.0),
                    normal: Vec3f::unit_y(),
                    tangent: vec3f(1.0, 0.0, 0.0),
                    bitangent: vec3f(0.0, 1.0, 0.0),
                }
            ]);
            
            indices.extend(vec![
                quad_base_index    ,  quad_base_index + 2,  quad_base_index + 1,  
                quad_base_index + 2,  quad_base_index + 0,  quad_base_index + 3
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
                position: tp,
                texcoord: vec2f(0.0, 1.0),
                normal: n,
                tangent: t,
                bitangent: b,
            },
            Vertex3D {
                position: p,
                texcoord: vec2f(-1.0, -1.0),
                normal: n,
                tangent: t,
                bitangent: b,
            },
            Vertex3D {
                position: np,
                texcoord: vec2f(1.0, -1.0),
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

/// Create an indexed unit cube mesh instance.
pub fn create_cube_mesh<D: gfx::Device>(dev: &mut D) -> pmfx::Mesh<D> {
    // cube veritces
    let vertices: Vec<Vertex3D> = vec![
        // front face
        Vertex3D {
            position: vec3f(-1.0, -1.0, 1.0),
            texcoord: vec2f(-1.0, -1.0),
            normal: vec3f(0.0, 0.0, 1.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(1.0, -1.0,  1.0),
            texcoord: vec2f(1.0, -1.0),
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
            texcoord: vec2f(-1.0, 1.0),
            normal: vec3f(0.0, 0.0, 1.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        // back face
        Vertex3D {
            position: vec3f(-1.0, -1.0, -1.0),
            texcoord: vec2f(-1.0, -1.0),
            normal: vec3f(0.0, 0.0, -1.0),
            tangent: vec3f(-1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(1.0, -1.0, -1.0),
            texcoord: vec2f(1.0, -1.0),
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
            position: vec3f(-1.0, 1.0, -1.0),
            texcoord: vec2f(-1.0, 1.0),
            normal: vec3f(0.0, 0.0, -1.0),
            tangent: vec3f(-1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        // right face
        Vertex3D {
            position: vec3f(1.0, -1.0, -1.0),
            texcoord: vec2f(-1.0, -1.0),
            normal: vec3f(1.0, 0.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(1.0, 1.0, -1.0),
            texcoord: vec2f(1.0, -1.0),
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
            texcoord: vec2f(-1.0, 1.0),
            normal: vec3f(1.0, 0.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        // left face
        Vertex3D {
            position: vec3f(-1.0, -1.0, -1.0),
            texcoord: vec2f(-1.0, -1.0),
            normal: vec3f(-1.0, 0.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(-1.0, 1.0, -1.0),
            texcoord: vec2f(1.0, -1.0),
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
            position: vec3f(-1.0, -1.0, 1.0),
            texcoord: vec2f(-1.0, 1.0),
            normal: vec3f(-1.0, 0.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        // top face
        Vertex3D {
            position: vec3f(-1.0, 1.0, -1.0),
            texcoord: vec2f(-1.0, -1.0),
            normal: vec3f(0.0, 1.0, 0.0),
            tangent: vec3f(-1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(1.0, 1.0, -1.0),
            texcoord: vec2f(1.0, -1.0),
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
            position: vec3f(-1.0, 1.0, 1.0),
            texcoord: vec2f(-1.0, 1.0),
            normal: vec3f(0.0, 1.0, 0.0),
            tangent: vec3f(-1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        // bottom face
        Vertex3D {
            position: vec3f(-1.0, -1.0, -1.0),
            texcoord: vec2f(-1.0, -1.0),
            normal: vec3f(0.0, -1.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex3D {
            position: vec3f(1.0, -1.0, -1.0),
            texcoord: vec2f(1.0, -1.0),
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
            texcoord: vec2f(-1.0, 1.0),
            normal: vec3f(0.0, -1.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
    ];

    let indices: Vec<u16> = vec![
        0,  2,  1,  2,  0,  3,   // front face
        4,  6,  5,  6,  4,  7,   // back face
        8,  10, 9,  10, 8,  11,  // right face
        12, 14, 13, 14, 12, 15,  // left face
        16, 18, 17, 18, 16, 19,  // top face
        20, 22, 21, 22, 20, 23   // bottom face
    ];

    pmfx::Mesh {
        vb: dev.create_buffer(&gfx::BufferInfo {
                usage: gfx::BufferUsage::Vertex,
                cpu_access: gfx::CpuAccessFlags::NONE,
                num_elements: 24,
                format: gfx::Format::Unknown,
                stride: std::mem::size_of::<Vertex3D>() 
            }, 
            Some(vertices.as_slice())
        ).unwrap(),
        ib: dev.create_buffer(&gfx::BufferInfo {
            usage: gfx::BufferUsage::Index,
            cpu_access: gfx::CpuAccessFlags::NONE,
            num_elements: 36,
            format: gfx::Format::R16u,
            stride: std::mem::size_of::<u16>()
            },
            Some(indices.as_slice())
        ).unwrap(),
        num_indices: 36
    } 
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
        for j in 0..2 {
            
            // vertices
            let mut t0 = corner[i];
            let t1 = corner[n];
            let mut t2 = yextent[j];

            // tex coords
            let mut tc0 = vec2f(1.0, -1.0);
            let tc1 = vec2f(-1.0, -1.0);
            let mut tc2 = vec2f(0.0, 1.0); 

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
                texcoord: uv0,
                normal: n,
                tangent: t,
                bitangent: b,
            },
            Vertex3D {
                position: t1,
                texcoord: uv1,
                normal: n,
                tangent: t,
                bitangent: b,
            },
            Vertex3D {
                position: t2,
                texcoord: Vec2f::zero(),
                normal: n,
                tangent: t,
                bitangent: b,
            }
        ];
        vertices.extend(tri);

        if recurse {
            let half_gr = 1.61803398875 / 2.0;
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
    let h = f32::pi() * 0.83333333333 * 0.5 * INV_PHI;
    let mut vertices = dodecahedron_face_in_axis(Vec3f::unit_y(), vec3f(0.0, -h, 0.0), 0.0, true);
    let bottom_vertices = dodecahedron_face_in_axis(-Vec3f::unit_y(), vec3f(0.0, h, 0.0), f32::pi(), true);
    vertices.extend(bottom_vertices);
    create_faceted_mesh_3d(dev, vertices)
}

/// Create an indexed faceted icosohedron mesh.
pub fn create_icosahedron_mesh<D: gfx::Device>(dev: &mut D) -> pmfx::Mesh<D> {
    let hemi_icosohedron = |axis: Vec3f, pos: Vec3f, start_angle: f32| -> Vec<Vertex3D> {
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
            vertices.extend(tri);
            
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
            vertices.extend(tri);
        }
        vertices
    };
    
    let mut vertices = hemi_icosohedron(Vec3f::unit_y(), Vec3f::unit_y() * 0.5, 0.0);
    let bottom_vertices = hemi_icosohedron(-Vec3f::unit_y(), Vec3f::unit_y() * -0.5, f32::pi());
    vertices.extend(bottom_vertices);

    create_faceted_mesh_3d(dev, vertices)
}