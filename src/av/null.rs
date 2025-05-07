#![allow(warnings)]

use std::marker::PhantomData;

pub struct VideoPlayer<D: crate::gfx::Device> {
    phantom: PhantomData<D>
}

use super::Error;
use crate::os::Size;

impl<D> super::VideoPlayer<D> for VideoPlayer<D> where D: crate::gfx::Device {
    fn create(device: &D) -> Result<Self, Error> {
        Ok(Self {
            phantom: PhantomData
        })
    }

    fn set_source(&mut self, filepath: String) -> Result<(), Error> {
        unimplemented!()
    }

    fn update(&mut self, device: &mut D) -> Result<(), Error> {
        unimplemented!()
    }

    fn play(&self) -> Result<(), Error> {
        unimplemented!()
    }

    fn pause(&self) -> Result<(), Error> {
        unimplemented!()
    }

    fn get_texture(&self) -> &Option<D::Texture> {
        unimplemented!()
    }

    fn is_loaded(&self) -> bool {
        unimplemented!()
    }

    fn is_playing(&self) -> bool  {
        unimplemented!()
    }

    fn is_ended(&self) -> bool  {
        unimplemented!()
    }

    fn get_size(&self) -> Size<u32>  {
        unimplemented!()
    }
}
