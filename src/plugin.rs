use crate::client::*;
use crate::gfx;
use crate::os;

/// Public trait for defining a plugin in a nother library
pub trait Plugin<D: gfx::Device, A: os::App> {
    /// Create a new instance of the plugin
    fn create() -> Self where Self: Sized;
    /// Called when the plugin is loaded and after a reload has happened, setup resources and state in here
    fn setup(&mut self, client: Client<D, A>) -> Client<D, A>;
    /// Called each and every frame, here put your update and render logic
    fn update(&mut self, client: Client<D, A>) -> Client<D, A>;
    /// Called when the plugin source has been modified and a reload is required, here handle any cleanup logic
    fn reload(&mut self, client: Client<D, A>) -> Client<D, A>;
}

/// Plugins are created on the heap and the instance is passed from the client to the plugin function calls
pub fn new_plugin<T : Plugin<crate::gfx_platform::Device, crate::os_platform::App> + Sized>() -> *mut T {
    unsafe {
        let layout = std::alloc::Layout::from_size_align(
            std::mem::size_of::<T>(),
            8,
        )
        .unwrap();
        std::alloc::alloc_zeroed(layout) as *mut T
    }
}

/// Macro to instantiate a new hotline plugin, simply defined a concrete plugin type:
/// struct EmptyPlugin;
/// 
/// You can implement the `Plugin` trait for `EmptyPlugin`
/// impl Plugin<gfx_platform::Device, os_platform::App> for EmptyPlugin {
/// ..
/// }
/// 
/// Then use this macro to make the plugin loadable from a dll
/// hotline_plugin![EmptyPlugin];
#[macro_export]
macro_rules! hotline_plugin {
    ($input:ident) => {
        // c-abi wrapper for `Plugin::create`
        #[no_mangle]
        pub fn create() -> *mut core::ffi::c_void {
            let ptr = new_plugin::<$input>() as *mut core::ffi::c_void;
            unsafe {
                let plugin = std::mem::transmute::<*mut core::ffi::c_void, *mut $input>(ptr);
                let plugin = plugin.as_mut().unwrap();
                *plugin = $input::create();
            }
            ptr
        }
        
        // c-abi wrapper for `Plugin::update`
        #[no_mangle]
        pub fn update(mut client: client::Client<gfx_platform::Device, os_platform::App>, ptr: *mut core::ffi::c_void) -> client::Client<gfx_platform::Device, os_platform::App> {
            unsafe { 
                let plugin = std::mem::transmute::<*mut core::ffi::c_void, *mut $input>(ptr);
                let plugin = plugin.as_mut().unwrap();
                plugin.update(client)
            }
        }
        
        // c-abi wrapper for `Plugin::setup`
        #[no_mangle]
        pub fn setup(mut client: client::Client<gfx_platform::Device, os_platform::App>, ptr: *mut core::ffi::c_void) -> client::Client<gfx_platform::Device, os_platform::App> {
            unsafe { 
                let plugin = std::mem::transmute::<*mut core::ffi::c_void, *mut $input>(ptr);
                let plugin = plugin.as_mut().unwrap();
                plugin.setup(client)
            }
        }
        
        // c-abi wrapper for `Plugin::reload`
        #[no_mangle]
        pub fn reload(mut client: client::Client<gfx_platform::Device, os_platform::App>, ptr: *mut core::ffi::c_void) -> client::Client<gfx_platform::Device, os_platform::App> {
            unsafe { 
                let plugin = std::mem::transmute::<*mut core::ffi::c_void, *mut $input>(ptr);
                let plugin = plugin.as_mut().unwrap();
                plugin.reload(client)
            }
        }
    }
}