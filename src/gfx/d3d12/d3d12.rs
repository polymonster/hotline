pub struct Device {
    name: String,
}

impl gfx::Device<GraphicsAPI> for Device {
    fn create() -> Device {
        println!("create d3d12 device");
        Device {
            name: String::from("d3d12 device")
        }
    }
}

pub enum GraphicsAPI {}
impl gfx::GraphicsAPI for GraphicsAPI {
    type Device = Device;
}