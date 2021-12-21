use std::{any::Any};

pub trait GraphicsAPI: 'static + Sized + Any + Send + Sync {
    type Device: Device<Self>;
}

pub trait Device<G: GraphicsAPI>: Any + Sized {
    fn create() -> Self;
}
