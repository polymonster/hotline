use maths_rs::Mat4f;
use maths_rs::Vec4f;

pub fn create_ortho_matrix(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Mat4f {
    Mat4f::from((
        Vec4f::new(2.0 / (right - left), 0.0, 0.0, 0.0),
        Vec4f::new(0.0, 2.0 / (top - bottom), 0.0, 0.0),
        Vec4f::new(0.0, 0.0, 1.0 / (near - far), 0.0),
        Vec4f::new((right + left) / (left - right), (top + bottom) / (bottom - top), near / (near - far), 1.0),
    ))
}

fn create_perspective_matrix_internal_lh(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Mat4f {
    Mat4f::from((
        Vec4f::new((2.0 * near) / (right - left), 0.0, (right + left) / (right - left), 0.0),
        Vec4f::new(0.0, (2.0 * near) / (top - bottom), (top + bottom) / (top - bottom), 0.0),
        Vec4f::new(0.0, 0.0, (-far - near) / (far - near), (-(2.0 * near) * far) / (far - near)),
        Vec4f::new(0.0, 0.0, -1.0, 0.0)
    ))
}

pub fn create_perspective_projection_lh_yup(fov: f32, aspect: f32, near: f32, far: f32) -> Mat4f {
    let tfov = f32::tan(fov * 0.5);
    let right = tfov * aspect * near;
    let left = -right;
    let top = tfov * near;
    let bottom = -top;
    create_perspective_matrix_internal_lh(left, right, top, bottom, near, far)
}
