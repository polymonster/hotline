use crate::Mat4f;
use crate::Vec4f;

struct Camera {
    view: Mat4f,
    proj: Mat4f
}

pub fn create_ortho_matrix(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Mat4f {
    Mat4f {
        x: Vec4f::new(2.0 / (right - left), 0.0, 0.0, 0.0),
        y: Vec4f::new(0.0, 2.0 / (top - bottom), 0.0, 0.0),
        z: Vec4f::new(0.0, 0.0, 1.0 / (near - far), 0.0),
        w: Vec4f::new((right + left) / (left - right), (top + bottom) / (bottom - top), near / (near - far), 1.0),
    }
}

