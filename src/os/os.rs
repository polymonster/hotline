use std::{any::Any};

pub trait Platform: 'static + Sized + Any + Send + Sync {
    type Instance: Instance<Self>;
    type Window: Window<Self>;
}

pub trait Instance<P: Platform>: Any + Sized {
    fn create() -> Self;
    fn create_window(&self, info: WindowInfo) -> P::Window;
    fn run(&self) -> bool;
}

pub struct Rect<T> {
    pub x: T,
    pub y: T,
    pub width: T,
    pub height: T
}

pub struct WindowInfo {
    pub title : String,
    pub rect : Rect<i32>,
}

pub trait Window<P: Platform>: Any + Sized {
    fn set_rect(&self, rect : Rect<i32>);
    fn resize(&self, width : i32, height : i32);
    fn close(&self);
}