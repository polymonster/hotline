/// Operating system module.
pub mod os;

/// Graphics and compute module.
pub mod gfx;

/// Hardware accelerated audio and video decoding
pub mod av;

/// Image reading/writing module support for (png, jpg, bmp, tiff, dds)
pub mod image;

/// Imgui rendering and platform implementation using imgui_sys
pub mod imgui;

/// Immediate mode primitive rendering API
pub mod imdraw;

/// High level graphics
pub mod pmfx;

/// Geometry primitives
pub mod primitives;

/// Use bitmask for flags
#[macro_use]
extern crate bitflags;

/// Generic errors for modules to define their own
pub struct Error {
    pub msg: String,
}

/// Generic debug for errors
impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

// conversion for windows-rs win32 errors
#[cfg(target_os = "windows")]
impl From<windows::core::Error> for Error {
    fn from(err: windows::core::Error) -> Error {
        Error {
            msg: err.message().to_string_lossy(),
        }
    }
}

// std errors
impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error {
            msg: err.to_string()
        }
    }
}

/// information to create a hotline context which will create an app, window, device
pub struct HotlineInfo {
    /// name for the app and window title
    pub name: String,
    /// window rect {pos_x pos_y, width, height}
    pub window_rect: os::Rect<i32>,
    /// signify if the app is DPI aware or not
    pub dpi_aware: bool,
    /// clear colour of the default swap chain
    pub clear_colour: Option<gfx::ClearColour>,
    /// optional name of gpu adaptor, use None for the default / primary device
    pub adapter_name: Option<String>,
    /// number of buffers in the swap chain (2 for double buffered, 3 for tripple etc)
    pub num_buffers: u32,
    /// size of the default device heap for shader resources (textures, buffers, etc)
    pub shader_heap_size: usize, 
    /// size of the default device heap for render targets
    pub render_target_heap_size: usize,
    /// size of the default device heap for depth stencil targets
    pub depth_stencil_heap_size: usize
}

impl Default for HotlineInfo {
    fn default() -> Self {
        HotlineInfo {
            name: "hotline".to_string(),
            window_rect: os::Rect {
                x: 100,
                y: 100,
                width: 1280,
                height: 720
            },
            dpi_aware: true,
            clear_colour: Some(gfx::ClearColour {
                r: 0.45,
                g: 0.55,
                b: 0.60,
                a: 1.00,
            }),
            num_buffers: 2,
            adapter_name: None,
            shader_heap_size: 1024,
            render_target_heap_size: 128,
            depth_stencil_heap_size: 64
        }
    }
}

pub struct Context<D: gfx::Device, A: os::App> {
    pub app: A,
    pub device: D,
    pub main_window: A::Window,
    pub swap_chain: D::SwapChain,
    pub pmfx: pmfx::Pmfx<D>,
    pub cmd_buf: D::CmdBuf,
    pub imdraw: imdraw::ImDraw<D>
}

impl<D, A> Context<D, A> where D: gfx::Device, A: os::App {
    pub fn create(info: HotlineInfo) -> Result<Self, Error> {
        let mut app = A::create(os::AppInfo {
            name: info.name.to_string(),
            num_buffers: info.num_buffers,
            dpi_aware: info.dpi_aware,
            window: false,
        });
    
        let mut device = D::create(&gfx::DeviceInfo {
            adapter_name: info.adapter_name,
            shader_heap_size: info.shader_heap_size,
            render_target_heap_size: info.render_target_heap_size,
            depth_stencil_heap_size: info.depth_stencil_heap_size,
        });
    
        let window = app.create_window(os::WindowInfo {
            title: info.name.to_string(),
            rect: info.window_rect,
            style: os::WindowStyleFlags::NONE,
            parent_handle: None,
        });
    
        let swap_chain_info = gfx::SwapChainInfo {
            num_buffers: info.num_buffers,
            format: gfx::Format::RGBA8n,
            clear_colour: info.clear_colour
        };
    
        let swap_chain = device.create_swap_chain::<A>(&swap_chain_info, &window)?;
        let cmd_buf = device.create_cmd_buf(info.num_buffers );
    
        let imdraw_info = imdraw::ImDrawInfo {
            initial_buffer_size_2d: 1024,
            initial_buffer_size_3d: 1024
        };
        let imdraw : imdraw::ImDraw<D> = imdraw::ImDraw::create(&imdraw_info).unwrap();
    
        Ok(Context {
            app,
            device,
            main_window: window,
            swap_chain,
            cmd_buf,
            pmfx: pmfx::Pmfx::create(),
            imdraw
        })
    }

    pub fn new_frame(&mut self) {

    }

    pub fn swap_buffers(&mut self) {

    }
}