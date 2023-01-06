use crate::gfx;

use maths_rs::vec::*;
use maths_rs::Vec2f;
use maths_rs::Vec3f;

pub struct Mesh<D: gfx::Device> {
    pub vb: D::Buffer,
    pub ib: D::Buffer,
    pub num_indices: u32
}

pub struct Vertex {
    pub position: Vec3f,
    pub texcoord: Vec2f,
    pub normal: Vec3f,
    pub tangent: Vec3f,
    pub bitangent: Vec3f,
}

// create a cube mesh index 
pub fn create_cube_mesh<D: gfx::Device>(dev: &mut D) -> Mesh<D> {
    // cube veritces
    let vertices: Vec<Vertex> = vec![
        // front face
        Vertex {
            position: vec3f(-1.0, -1.0, 1.0),
            texcoord: vec2f(-1.0, -1.0),
            normal: vec3f(0.0, 0.0, 1.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex {
            position: vec3f(1.0, -1.0,  1.0),
            texcoord: vec2f(1.0, -1.0),
            normal: vec3f(0.0, 0.0, 1.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex {
            position: vec3f(1.0, 1.0, 1.0),
            texcoord: vec2f(1.0, 1.0),
            normal: vec3f(0.0, 0.0, 1.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex {
            position: vec3f(-1.0, 1.0, 1.0),
            texcoord: vec2f(-1.0, 1.0),
            normal: vec3f(0.0, 0.0, 1.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        // back face
        Vertex {
            position: vec3f(-1.0, -1.0, -1.0),
            texcoord: vec2f(-1.0, -1.0),
            normal: vec3f(0.0, 0.0, -1.0),
            tangent: vec3f(-1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex {
            position: vec3f(1.0, -1.0, -1.0),
            texcoord: vec2f(1.0, -1.0),
            normal: vec3f(0.0, 0.0, -1.0),
            tangent: vec3f(-1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex {
            position: vec3f(1.0, 1.0, -1.0),
            texcoord: vec2f(1.0, 1.0),
            normal: vec3f(0.0, 0.0, -1.0),
            tangent: vec3f(-1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex {
            position: vec3f(-1.0, 1.0, -1.0),
            texcoord: vec2f(-1.0, 1.0),
            normal: vec3f(0.0, 0.0, -1.0),
            tangent: vec3f(-1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        // right face
        Vertex {
            position: vec3f(1.0, -1.0, -1.0),
            texcoord: vec2f(-1.0, -1.0),
            normal: vec3f(1.0, 0.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex {
            position: vec3f(1.0, 1.0, -1.0),
            texcoord: vec2f(1.0, -1.0),
            normal: vec3f(1.0, 0.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex {
            position: vec3f(1.0, 1.0, 1.0),
            texcoord: vec2f(1.0, 1.0),
            normal: vec3f(1.0, 0.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex {
            position: vec3f(1.0, -1.0, 1.0),
            texcoord: vec2f(-1.0, 1.0),
            normal: vec3f(1.0, 0.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        // left face
        Vertex {
            position: vec3f(-1.0, -1.0, -1.0),
            texcoord: vec2f(-1.0, -1.0),
            normal: vec3f(-1.0, 0.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex {
            position: vec3f(-1.0, 1.0, -1.0),
            texcoord: vec2f(1.0, -1.0),
            normal: vec3f(-1.0, 0.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex {
            position: vec3f(-1.0, 1.0, 1.0),
            texcoord: vec2f(1.0, 1.0),
            normal: vec3f(-1.0, 0.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex {
            position: vec3f(-1.0, -1.0, 1.0),
            texcoord: vec2f(-1.0, 1.0),
            normal: vec3f(-1.0, 0.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        // top face
        Vertex {
            position: vec3f(-1.0, 1.0, -1.0),
            texcoord: vec2f(-1.0, -1.0),
            normal: vec3f(0.0, 1.0, 0.0),
            tangent: vec3f(-1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex {
            position: vec3f(1.0, 1.0, -1.0),
            texcoord: vec2f(1.0, -1.0),
            normal: vec3f(0.0, 1.0, 0.0),
            tangent: vec3f(-1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex {
            position: vec3f(1.0, 1.0, 1.0),
            texcoord: vec2f(1.0, 1.0),
            normal: vec3f(0.0, 1.0, 0.0),
            tangent: vec3f(-1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex {
            position: vec3f(-1.0, 1.0, 1.0),
            texcoord: vec2f(-1.0, 1.0),
            normal: vec3f(0.0, 1.0, 0.0),
            tangent: vec3f(-1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        // bottom face
        Vertex {
            position: vec3f(-1.0, -1.0, -1.0),
            texcoord: vec2f(-1.0, -1.0),
            normal: vec3f(0.0, -1.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex {
            position: vec3f(1.0, -1.0, -1.0),
            texcoord: vec2f(1.0, -1.0),
            normal: vec3f(0.0, -1.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex {
            position: vec3f(1.0, -1.0, 1.0),
            texcoord: vec2f(1.0, 1.0),
            normal: vec3f(0.0, -1.0, 0.0),
            tangent: vec3f(1.0, 0.0, 0.0),
            bitangent: vec3f(0.0, 1.0, 0.0),
        },
        Vertex {
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

    Mesh {
        vb: dev.create_buffer(&gfx::BufferInfo {
                usage: gfx::BufferUsage::Vertex,
                cpu_access: gfx::CpuAccessFlags::NONE,
                num_elements: 24,
                format: gfx::Format::Unknown,
                stride: std::mem::size_of::<Vertex>() 
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