use crate::gfx;
use gfx::d3d12;
use gfx::Device;

use windows::{
    core::*, Win32::Foundation::*, Win32::Graphics::Direct3D::Fxc::*, Win32::Graphics::Direct3D::*,
    Win32::Graphics::Direct3D11::*, Win32::Graphics::Dxgi::Common::*, Win32::Graphics::Dxgi::*,
    Win32::System::LibraryLoader::*, Win32::System::Threading::*,
    Win32::System::WindowsProgramming::*,
};

struct VideoPlayer {

}

impl super::VideoPlayer<d3d12::Device> for VideoPlayer {
    fn create(device: &d3d12::Device) {

        let factory = d3d12::get_dxgi_factory(&device);
        let adapter = d3d12::get_hardware_adapter(factory, &Some(device.get_adapter_info().name.to_string()));

        /*
        let mut dxgi_factory_flags: u32 = 0;
        if cfg!(debug_assertions) {
            let mut debug: Option<ID3D11Debug> = None;
            if let Some(debug) = D3D11GetDebugInterface(&mut debug).ok().and_then(|_| debug) {
                debug.EnableDebugLayer();
                println!("hotline::gfx::d3d12: enabling debug layer");
            }
            dxgi_factory_flags = DXGI_CREATE_FACTORY_DEBUG;
        }

        // create dxgi factory
        let dxgi_factory = CreateDXGIFactory2(dxgi_factory_flags)

        D3D11CreateDevice()
        */
    }
}