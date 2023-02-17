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
// const M_PHI : f32 = 1.61803398875;

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
    let angle_step = (f32::pi() *2.0) / 3.0;
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
        vertices.extend(get_face_vertices(p, np, tp));
    }

    // make the base face
    vertices.extend(get_face_vertices(base_positions[0], base_positions[2], base_positions[1]));

    // generate indices
    create_faceted_mesh_3d(dev, vertices)
}

/// Create a an indexed unit cube mesh instance
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