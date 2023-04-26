use crate::gfx;
use gfx::Buffer;
use gfx::CmdBuf;

use maths_rs::Vec2f;
use maths_rs::Vec3f;
use maths_rs::Vec4f;
use maths_rs::vec::*;
use maths_rs::num::*;

/// A coherent cpu/gpu buffer back by multiple gpu buffers to allow cpu writes while gpu is inflight 
struct DynamicBuffer<D: gfx::Device> {
    cpu_data: Vec<f32>,
    gpu_data: Vec<D::Buffer>,
    gpu_data_size: Vec<usize>,
    vertex_count: u32
}

/// 2d vertex with position and colour
#[repr(C)]
struct ImDrawVertex2d {
    _position: [f32; 2],
    _color: [f32; 4],
}

/// 3d vertex with position and colour
#[repr(C)]
struct ImDrawVertex3d {
    _position: [f32; 3],
    _color: [f32; 4],
}

/// Information to create an instance of ImDraw
pub struct ImDrawInfo {
    pub initial_buffer_size_2d: usize,
    pub initial_buffer_size_3d: usize
}

/// Immediate mode primitive drawing API struct
pub struct ImDraw<D: gfx::Device> {
    vertices_2d: DynamicBuffer<D>,
    vertices_3d: DynamicBuffer<D>
}

/// Immediate mode primitive drawing API implementation
impl<D> ImDraw<D> where D: gfx::Device {
    fn new_buffer_2d_info(num_elements: usize) -> gfx::BufferInfo {
        gfx::BufferInfo {
            usage: gfx::BufferUsage::VERTEX,
            cpu_access: gfx::CpuAccessFlags::WRITE,
            format: gfx::Format::Unknown,
            stride: std::mem::size_of::<ImDrawVertex2d>(),
            num_elements,
            initial_state: gfx::ResourceState::VertexConstantBuffer
        }
    }

    fn new_buffer_3d_info(num_elements: usize) -> gfx::BufferInfo {
        gfx::BufferInfo {
            usage: gfx::BufferUsage::VERTEX,
            cpu_access: gfx::CpuAccessFlags::WRITE,
            format: gfx::Format::Unknown,
            stride: std::mem::size_of::<ImDrawVertex3d>(),
            num_elements,
            initial_state: gfx::ResourceState::VertexConstantBuffer
        }
    }

    pub fn create(info: &ImDrawInfo) -> Result<Self, super::Error> {
        Ok(ImDraw {
            vertices_2d: DynamicBuffer {
                cpu_data: Vec::with_capacity(info.initial_buffer_size_2d),
                gpu_data: Vec::new(),
                gpu_data_size: Vec::new(),
                vertex_count: 0
            },
            vertices_3d: DynamicBuffer {
                cpu_data: Vec::with_capacity(info.initial_buffer_size_3d),
                gpu_data: Vec::new(),
                gpu_data_size: Vec::new(),
                vertex_count: 0
            },
        })
    }

    pub fn add_vertex_2d(&mut self, v: Vec2f, col: Vec4f) {
        // push position
        for i in 0..2 {
            self.vertices_2d.cpu_data.push(v[i])
        }
        // push colour
        for i in 0..4 {
            self.vertices_2d.cpu_data.push(col[i])
        }
    }

    pub fn add_line_2d(&mut self, start: Vec2f, end: Vec2f, col: Vec4f) {
        self.add_vertex_2d(start, col);
        self.add_vertex_2d(end, col);
    }

    pub fn add_tri_2d(&mut self, p1: Vec2f, p2: Vec2f, p3: Vec2f, col: Vec4f) {
        // edge 1
        self.add_vertex_2d(p1, col);
        self.add_vertex_2d(p2, col);
        // edge 2
        self.add_vertex_2d(p2, col);
        self.add_vertex_2d(p3, col);
        // edge 3
        self.add_vertex_2d(p3, col);
        self.add_vertex_2d(p1, col);
    }

    pub fn add_rect_2d(&mut self, p: Vec2f, size: Vec2f, col: Vec4f) {
        let p1 = p + Vec2f::new(size.x, 0.0);
        let p2 = p + size;
        let p3 = p + Vec2f::new(0.0, size.y);
        // edge 1
        self.add_vertex_2d(p, col);
        self.add_vertex_2d(p1, col);
        // edge 2
        self.add_vertex_2d(p1, col);
        self.add_vertex_2d(p2, col);
        // edge 3
        self.add_vertex_2d(p2, col);
        self.add_vertex_2d(p3, col);
        // edge 4
        self.add_vertex_2d(p3, col);
        self.add_vertex_2d(p, col);
    }

    pub fn add_vertex_3d(&mut self, v: Vec3f, col: Vec4f) {
        // push position
        for i in 0..3 {
            self.vertices_3d.cpu_data.push(v[i])
        }
        // push colour
        for i in 0..4 {
            self.vertices_3d.cpu_data.push(col[i])
        }
    }

    pub fn add_line_3d(&mut self, start: Vec3f, end: Vec3f, col: Vec4f) {
        self.add_vertex_3d(start, col);
        self.add_vertex_3d(end, col);
    }

    pub fn add_point_3d(&mut self, pos: Vec3f, size: f32, col: Vec4f) {
        self.add_line_3d(pos - Vec3f::unit_x() * size, pos + Vec3f::unit_x() * size, col);
        self.add_line_3d(pos - Vec3f::unit_y() * size, pos + Vec3f::unit_y() * size, col);
        self.add_line_3d(pos - Vec3f::unit_z() * size, pos + Vec3f::unit_z() * size, col);
    }

    pub fn add_circle_3d_xz(&mut self, pos: Vec3f, radius: f32, col: Vec4f) {
        let segs = 16;
        let step = (f32::pi() * 2.0) / segs as f32;
        for i in 0..16 {
            let ix = i as f32 * step;
            let iy = (i + 1) as f32 * step;
            self.add_line_3d(pos + Vec3f::new(f32::sin(ix), 0.0, f32::cos(ix)) * radius, 
                pos + Vec3f::new(f32::sin(iy), 0.0, f32::cos(iy)) * radius, col);
        }
    }

    /// Add a 3D aabb from `aabb_min` to `aabb_max` with designated colour `col`
    pub fn add_aabb_3d(&mut self, aabb_min: Vec3f, aabb_max: Vec3f, col: Vec4f) {
        self.add_line_3d(vec3f(aabb_min.x, aabb_min.y, aabb_min.z), vec3f(aabb_max.x, aabb_min.y, aabb_min.z), col);
        self.add_line_3d(vec3f(aabb_min.x, aabb_min.y, aabb_min.z), vec3f(aabb_min.x, aabb_min.y, aabb_max.z), col);
        self.add_line_3d(vec3f(aabb_min.x, aabb_min.y, aabb_max.z), vec3f(aabb_max.x, aabb_min.y, aabb_max.z), col);
        self.add_line_3d(vec3f(aabb_max.x, aabb_min.y, aabb_max.z), vec3f(aabb_max.x, aabb_min.y, aabb_min.z), col);
        self.add_line_3d(vec3f(aabb_min.x, aabb_max.y, aabb_min.z), vec3f(aabb_max.x, aabb_max.y, aabb_min.z), col);
        self.add_line_3d(vec3f(aabb_min.x, aabb_max.y, aabb_min.z), vec3f(aabb_min.x, aabb_max.y, aabb_max.z), col);
        self.add_line_3d(vec3f(aabb_min.x, aabb_max.y, aabb_max.z), vec3f(aabb_max.x, aabb_max.y, aabb_max.z), col);
        self.add_line_3d(vec3f(aabb_max.x, aabb_max.y, aabb_max.z), vec3f(aabb_max.x, aabb_max.y, aabb_min.z), col);
        self.add_line_3d(vec3f(aabb_min.x, aabb_min.y, aabb_min.z), vec3f(aabb_min.x, aabb_max.y, aabb_min.z), col);
        self.add_line_3d(vec3f(aabb_max.x, aabb_min.y, aabb_min.z), vec3f(aabb_max.x, aabb_max.y, aabb_min.z), col);
        self.add_line_3d(vec3f(aabb_max.x, aabb_min.y, aabb_max.z), vec3f(aabb_max.x, aabb_max.y, aabb_max.z), col);
        self.add_line_3d(vec3f(aabb_min.x, aabb_min.y, aabb_max.z), vec3f(aabb_min.x, aabb_max.y, aabb_max.z), col);
    }

    /// Add a 3D obb from corners `obb` where the corners are formed of 0-3 front face, 4-7 back-face with designated colour `col`
    pub fn add_obb_3d(&mut self, obb: Vec<Vec3f>, col: Vec4f) {
        self.add_line_3d(vec3f(obb[0].x, obb[0].y, obb[0].z), vec3f(obb[1].x, obb[1].y, obb[1].z), col);
        self.add_line_3d(vec3f(obb[1].x, obb[1].y, obb[1].z), vec3f(obb[2].x, obb[2].y, obb[2].z), col);
        self.add_line_3d(vec3f(obb[2].x, obb[2].y, obb[2].z), vec3f(obb[3].x, obb[3].y, obb[3].z), col);
        self.add_line_3d(vec3f(obb[3].x, obb[3].y, obb[3].z), vec3f(obb[0].x, obb[0].y, obb[0].z), col);
        self.add_line_3d(vec3f(obb[4].x, obb[4].y, obb[4].z), vec3f(obb[5].x, obb[5].y, obb[5].z), col);
        self.add_line_3d(vec3f(obb[5].x, obb[5].y, obb[5].z), vec3f(obb[6].x, obb[6].y, obb[6].z), col);
        self.add_line_3d(vec3f(obb[6].x, obb[6].y, obb[6].z), vec3f(obb[7].x, obb[7].y, obb[7].z), col);
        self.add_line_3d(vec3f(obb[7].x, obb[7].y, obb[7].z), vec3f(obb[4].x, obb[4].y, obb[4].z), col);
        self.add_line_3d(vec3f(obb[4].x, obb[4].y, obb[4].z), vec3f(obb[0].x, obb[0].y, obb[0].z), col);
        self.add_line_3d(vec3f(obb[5].x, obb[5].y, obb[5].z), vec3f(obb[1].x, obb[1].y, obb[1].z), col);
        self.add_line_3d(vec3f(obb[6].x, obb[6].y, obb[6].z), vec3f(obb[2].x, obb[2].y, obb[2].z), col);
        self.add_line_3d(vec3f(obb[7].x, obb[7].y, obb[7].z), vec3f(obb[3].x, obb[3].y, obb[3].z), col);
    }

    pub fn submit(&mut self, device: &mut D, buffer_index: usize) -> Result<(), super::Error> {
        if !self.vertices_2d.cpu_data.is_empty() {
            let num_elems = self.vertices_2d.cpu_data.len() / 6;
            while buffer_index >= self.vertices_2d.gpu_data.len() {
                // push a new buffer
                self.vertices_2d.gpu_data.push(
                    device.create_buffer::<u8>(&Self::new_buffer_2d_info(num_elems), None)?
                );
                self.vertices_2d.gpu_data_size.push(num_elems);
            }
            if num_elems > self.vertices_2d.gpu_data_size[buffer_index] {
                // resize buffer
                self.vertices_2d.gpu_data[buffer_index] = device.create_buffer::<u8>(
                    &Self::new_buffer_2d_info(num_elems), None)?;
            }
            // update buffer
            self.vertices_2d.gpu_data[buffer_index].update(0, self.vertices_2d.cpu_data.as_slice())?;
            self.vertices_2d.gpu_data_size[buffer_index] = num_elems;
            self.vertices_2d.vertex_count = num_elems as u32;
            self.vertices_2d.cpu_data.clear();
        }
        if !self.vertices_3d.cpu_data.is_empty() {
            let num_elems = self.vertices_3d.cpu_data.len() / 7;
            while buffer_index >= self.vertices_3d.gpu_data.len() {
                // push a new buffer
                self.vertices_3d.gpu_data.push(
                    device.create_buffer::<u8>(&Self::new_buffer_3d_info(num_elems), None)?
                );
                self.vertices_3d.gpu_data_size.push(num_elems);
            }
            if num_elems > self.vertices_3d.gpu_data_size[buffer_index] {
                // resize buffer
                self.vertices_3d.gpu_data[buffer_index] = device.create_buffer::<u8>(
                    &Self::new_buffer_3d_info(num_elems), None)?;
            }
            // update buffer
            self.vertices_3d.gpu_data[buffer_index].update(0, self.vertices_3d.cpu_data.as_slice())?;
            self.vertices_3d.gpu_data_size[buffer_index] = num_elems;
            self.vertices_3d.vertex_count = num_elems as u32;
            self.vertices_3d.cpu_data.clear();
        }
        Ok(())     
    }

    pub fn draw_2d(&mut self, cmd: &D::CmdBuf, buffer_index: usize) {
        if buffer_index < self.vertices_2d.gpu_data.len() {
            cmd.set_vertex_buffer(&self.vertices_2d.gpu_data[buffer_index], 0);
            cmd.draw_instanced(self.vertices_2d.vertex_count, 1, 0, 0);
        }
    }

    pub fn draw_3d(&mut self, cmd: &D::CmdBuf, buffer_index: usize) {
        if buffer_index < self.vertices_3d.gpu_data.len() {
            cmd.set_vertex_buffer(&self.vertices_3d.gpu_data[buffer_index], 0);
            cmd.draw_instanced(self.vertices_3d.vertex_count, 1, 0, 0);
        }
    }
}