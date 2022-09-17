use crate::gfx;
use gfx::Buffer;
use gfx::CmdBuf;

use maths_rs::Vec2f;
use maths_rs::Vec4f;

/// A coherent cpu/gpu buffer back by multiple gpu buffers to allow cpu writes while gpu is inflight 
struct DynamicBuffer<D: gfx::Device> {
    cpu_data: Vec<f32>,
    gpu_data: Vec<D::Buffer>,
    gpu_data_size: Vec<usize>,
    vertex_count: u32
}

/// 2d vertex with position and colour
struct ImDrawVertex2d {
    position: [f32; 2],
    color: [f32; 4],
}

/// 3d vertex with position and colour
struct ImDrawVertex3d {
    position: [f32; 3],
    color: [f32; 4],
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
            usage: gfx::BufferUsage::Vertex,
            cpu_access: gfx::CpuAccessFlags::WRITE,
            format: gfx::Format::Unknown,
            stride: std::mem::size_of::<ImDrawVertex2d>(),
            num_elements: num_elements,
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

    pub fn submit(&mut self, device: &mut D, buffer_index: usize) -> Result<(), super::Error> {
        if self.vertices_2d.cpu_data.len() > 0 {
            let num_elems = self.vertices_2d.cpu_data.len() / 6;
            if buffer_index >= self.vertices_2d.gpu_data.len() {
                // push a new buffer
                self.vertices_2d.gpu_data.push(
                    device.create_buffer::<u8>(&Self::new_buffer_2d_info(num_elems), None)?
                );
                self.vertices_2d.gpu_data_size.push(num_elems);
            }
            else {
                if num_elems > self.vertices_2d.gpu_data_size[buffer_index] {
                    // resize buffer
                    self.vertices_2d.gpu_data[buffer_index] = device.create_buffer::<u8>(
                        &Self::new_buffer_2d_info(num_elems), None)?;
                }
            }
            // update buffer
            self.vertices_2d.gpu_data[buffer_index].update(0, self.vertices_2d.cpu_data.as_slice())?;
            self.vertices_2d.gpu_data_size[buffer_index] = num_elems;
            self.vertices_2d.vertex_count = num_elems as u32;
            self.vertices_2d.cpu_data.clear();
        }
        Ok(())     
    }

    pub fn draw(&mut self, cmd: &mut D::CmdBuf, buffer_index: usize) {
        cmd.set_vertex_buffer(&self.vertices_2d.gpu_data[buffer_index], 0);
        cmd.draw_instanced(self.vertices_2d.vertex_count, 1, 0, 0);
    }
}