
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::Hash;
use std::path::Path;

use crate::gfx;
use std::fs;

pub struct Pmfx<D: gfx::Device> {
    pmfx: HashMap<String, PmfxPipeline>,
    pmfx_folders: HashMap<String, String>,
    pipelines: HashMap<String, D::RenderPipeline>,
    shaders: HashMap<String, D::Shader>,
    blend_info: HashMap<String, PmfxPipeline>,
    depth_stencil_info: HashMap<String, PmfxPipeline>,
    raster_info: HashMap<String, PmfxPipeline>,
}

#[derive(Serialize, Deserialize)]
pub struct Pmxf2 {
    pipelines: HashMap<String, PmfxPipeline>,
}

#[derive(Serialize, Deserialize)]
pub struct PmfxPipeline {
    vs: Option<String>,
    ps: Option<String>,
    cs: Option<String>,
    vertex_layout: Option<gfx::InputLayout>,
    descriptor_layout: gfx::DescriptorLayout,
    blend_info: Option<String>,
    depth_stencil_info: Option<String>,
    raster_info: Option<String>
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

impl<D> Pmfx<D> where D: gfx::Device {
    pub fn create() -> Self {
        Pmfx {
            pmfx: HashMap::new(),
            pmfx_folders: HashMap::new(),
            pipelines: HashMap::new(),
            blend_info: HashMap::new(),
            depth_stencil_info: HashMap::new(),
            raster_info: HashMap::new(),
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

        // blend states ...

        Ok(())
    }

    fn create_shader<'stack>(shaders: &'stack mut HashMap<String, D::Shader>, device: &D, folder: &Path, file: &Option<String>) -> Result<(), super::Error> {
        if let Some(file) = file {
            if !shaders.contains_key(file) {
                println!("hotline::pmfx:: compiling shader: {}", file);
                let shader = create_shader_from_file(device, folder, Some(file.to_string()));
                if let Some(shader) = shader.unwrap() {
                    println!("hotline::pmfx:: success: {}", file);
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
            Self::create_shader(shaders, device, &Path::new(&folder), &pipeline.vs)?;
            Self::create_shader(shaders, device, &Path::new(&folder), &pipeline.ps)?;
            Self::create_shader(shaders, device, &Path::new(&folder), &pipeline.cs)?;

            // TODO: infer compute or graphics pipeline from pmfx
            let cs = self.get_shader(&pipeline.cs);
            if let Some(_cs) = cs {
                // compute pipeline
            }
            else {
                let vertex_layout = pipeline.vertex_layout.as_ref().unwrap();
                let pso = device.create_render_pipeline(&gfx::RenderPipelineInfo {
                    vs: self.get_shader(&pipeline.vs),
                    fs: self.get_shader(&pipeline.ps),
                    input_layout: vertex_layout.to_vec(),
                    descriptor_layout: pipeline.descriptor_layout.clone(),
                    raster_info: gfx::RasterInfo::default(),
                    depth_stencil_info: gfx::DepthStencilInfo::default(),
                    blend_info: gfx::BlendInfo {
                        alpha_to_coverage_enabled: false,
                        independent_blend_enabled: false,
                        render_target: vec![gfx::RenderTargetBlendInfo::default()],
                    },
                    topology: gfx::Topology::LineList,
                    patch_index: 0,
                    pass: pass,
                })?;
                println!("hotline::pmfx:: compiled pipeline: {}", pipeline_name);
                self.pipelines.insert(pipeline_name.to_string(), pso);
            }
            Ok(())
        }
        else {
            Err(super::Error {
                msg: String::from(format!("hotline::pmfx:: could not find pipeline: {}", pipeline_name)),
            })
        }
    }

    /// Fetch a prebuilt RenderPipeline or create a new one on the fly if it does not exist
    pub fn get_pipeline<'stack>(&'stack self, pipeline_name: &str) -> Option<&'stack D::RenderPipeline> {
        if self.pipelines.contains_key(pipeline_name) {
            Some(&self.pipelines[pipeline_name])
        }
        else {
            None
        }
    }
}

