use std::{any::Any};

pub trait Platform: 'static + Sized + Any {
    type Instance: Instance<Self>;
    type Window: Window<Self>;
}

pub trait Instance<P: Platform>: 'static + Any + Sized {
    fn create() -> Self;
    fn create_window(&self, info: WindowInfo) -> P::Window;
    fn run(&self) -> bool;
}

#[derive(Copy, Clone)]
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
    fn set_rect(&mut self, rect : Rect<i32>);
    fn get_rect(&self) -> Rect<i32>;
    fn set_size(&mut self, width : i32, height : i32);
    fn get_size(&self) -> (i32, i32);
    fn close(&mut self);
}