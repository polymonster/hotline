
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::gfx;
use std::fs;

pub struct Pmfx<D: gfx::Device> {
    pmfx: HashMap<String, PmfxPipeline>,
    pmfx_folders: HashMap<String, String>,
    render_pipelines: HashMap<String, D::RenderPipeline>,
    compute_pipelines: HashMap<String, D::ComputePipeline>,
    shaders: HashMap<String, D::Shader>,
    depth_stencil_states: HashMap<String, gfx::DepthStencilInfo>
}

#[derive(Serialize, Deserialize)]
pub struct Pmxf2 {
    pipelines: HashMap<String, PmfxPipeline>,
    depth_stencil_states: Option<HashMap<String, gfx::DepthStencilInfo>>
}

#[derive(Serialize, Deserialize)]
pub struct PmfxPipeline {
    vs: Option<String>,
    ps: Option<String>,
    cs: Option<String>,
    vertex_layout: Option<gfx::InputLayout>,
    descriptor_layout: gfx::DescriptorLayout,
    blend_state: Option<String>,
    depth_stencil_state: Option<String>,
    raster_state: Option<String>,
    topology: Option<gfx::Topology>
}

fn create_shader_from_file<D: gfx::Device>(device: &D, folder: &Path, file: Option<String>) -> Result<Option<D::Shader>, super::Error> {
    if let Some(shader) = file {
        let shader_filepath = folder.join(shader);
        let shader_data = fs::read(shader_filepath)?;                
        let shader_info = gfx::ShaderInfo {
            shader_type: gfx::ShaderType::Vertex,
            compile_info: None
        };
        Ok(Some(device.create_shader(&shader_info, &shader_data)?))
    }
    else {
        Ok(None)
    }
}

fn info_from_state<T: Default + Copy>(name: &Option<String>, map: &HashMap<String, T>) -> T {
    if let Some(name) = &name {
        if map.contains_key(name) {
            map[name]
        }
        else {
            T::default()
        }
    }
    else {
        T::default()
    }
}

impl<D> Pmfx<D> where D: gfx::Device {
    pub fn create() -> Self {
        Pmfx {
            pmfx: HashMap::new(),
            pmfx_folders: HashMap::new(),
            render_pipelines: HashMap::new(),
            compute_pipelines: HashMap::new(),
            depth_stencil_states: HashMap::new(),
            shaders: HashMap::new()
        }
    }

    /// load a pmfx friom a folder, where the folder contains a pmfx info.json and shader source in separate files
    /// within the directory
    pub fn load(&mut self, filepath: &str) -> Result<(), super::Error> {        
        // get the name for indexing by pmfx name/folder
        let folder = Path::new(filepath);
        let pmfx_name = if let Some(name) = folder.file_name() {
            String::from(name.to_os_string().to_str().unwrap())
        }
        else {
            String::from(filepath)
        };

        //  deserialise pmfx pipelines from file
        let info_filepath = folder.join(format!("{}.json", pmfx_name));
        let pmfx_data = fs::read(info_filepath).unwrap();
        let shader: Pmxf2 = serde_json::from_slice(&pmfx_data).unwrap();

        // track pipelines
        for (name, pipeline) in shader.pipelines {            
            self.pmfx.insert(name.to_string(), pipeline);
            self.pmfx_folders.insert(name.to_string(), String::from(filepath));
        }

        // states ...
        if let Some(states) = shader.depth_stencil_states {
            for (name, state) in states {
                self.depth_stencil_states.insert(name, state);
            }
        }
        
        Ok(())
    }

    fn create_shader<'stack>(shaders: &'stack mut HashMap<String, D::Shader>, device: &D, folder: &Path, file: &Option<String>) -> Result<(), super::Error> {
        if let Some(file) = file {
            if !shaders.contains_key(file) {
                println!("hotline_rs::pmfx:: compiling shader: {}", file);
                let shader = create_shader_from_file(device, folder, Some(file.to_string()));
                if let Some(shader) = shader.unwrap() {
                    println!("hotline_rs::pmfx:: success: {}", file);
                    shaders.insert(file.to_string(), shader);
                    Ok(())
                }
                else {
                    Ok(())
                }
            }
            else {
                Ok(())
            }
        }
        else {
            Ok(())
        }
    }

    pub fn get_shader<'stack>(&'stack self, file: &Option<String>) -> Option<&'stack D::Shader> {
        if let Some(file) = file {
            if self.shaders.contains_key(file) {
                Some(&self.shaders[file])
            }
            else {
                None
            }
        }
        else {
            None
        }
    }

    /// Create a RenderPipeline instance for the combination of pmfx_pipeline settings and an associated RenderPass
    pub fn create_pipeline(&mut self, device: &D, pipeline_name: &str, pass: &D::RenderPass) -> Result<(), super::Error> {        
        // grab the pmfx pipeline info
        if self.pmfx.contains_key(pipeline_name) {
            let pipeline = &self.pmfx[pipeline_name];
            let shaders = &mut self.shaders;

            // TODO: shader array
            let folder = self.pmfx_folders[pipeline_name].to_string();
            Self::create_shader(shaders, device, Path::new(&folder), &pipeline.vs)?;
            Self::create_shader(shaders, device, Path::new(&folder), &pipeline.ps)?;
            Self::create_shader(shaders, device, Path::new(&folder), &pipeline.cs)?;

            // TODO: infer compute or graphics pipeline from pmfx
            let cs = self.get_shader(&pipeline.cs);
            if let Some(cs) = cs {
                let pso = device.create_compute_pipeline(&gfx::ComputePipelineInfo {
                    cs,
                    descriptor_layout: pipeline.descriptor_layout.clone(),
                })?;
                println!("hotline_rs::pmfx:: compiled compute pipeline: {}", pipeline_name);
                self.compute_pipelines.insert(pipeline_name.to_string(), pso);
            }
            else {
                let vertex_layout = pipeline.vertex_layout.as_ref().unwrap();
                let pso = device.create_render_pipeline(&gfx::RenderPipelineInfo {
                    vs: self.get_shader(&pipeline.vs),
                    fs: self.get_shader(&pipeline.ps),
                    input_layout: vertex_layout.to_vec(),
                    descriptor_layout: pipeline.descriptor_layout.clone(),
                    raster_info: gfx::RasterInfo::default(),
                    depth_stencil_info: info_from_state(&pipeline.depth_stencil_state, &self.depth_stencil_states),
                    blend_info: gfx::BlendInfo {
                        alpha_to_coverage_enabled: false,
                        independent_blend_enabled: false,
                        render_target: vec![gfx::RenderTargetBlendInfo::default()],
                    },
                    topology: 
                        if let Some(topology) = pipeline.topology {
                            topology
                        }
                        else {
                            gfx::Topology::TriangleList
                        },
                    patch_index: 0,
                    pass,
                })?;
                println!("hotline_rs::pmfx:: compiled render pipeline: {}", pipeline_name);
                self.render_pipelines.insert(pipeline_name.to_string(), pso);
            }
            Ok(())
        }
        else {
            Err(super::Error {
                msg: format!("hotline_rs::pmfx:: could not find pipeline: {}", pipeline_name),
            })
        }
    }

    /// Fetch a prebuilt RenderPipeline
    pub fn get_render_pipeline<'stack>(&'stack self, pipeline_name: &str) -> Option<&'stack D::RenderPipeline> {
        if self.render_pipelines.contains_key(pipeline_name) {
            Some(&self.render_pipelines[pipeline_name])
        }
        else {
            None
        }
    }

    /// Fetch a prebuilt ComputePipeline
    pub fn get_compute_pipeline<'stack>(&'stack self, pipeline_name: &str) -> Option<&'stack D::ComputePipeline> {
        if self.compute_pipelines.contains_key(pipeline_name) {
            Some(&self.compute_pipelines[pipeline_name])
        }
        else {
            None
        }
    }
}

