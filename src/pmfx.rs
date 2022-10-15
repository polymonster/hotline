
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::gfx;

use std::fs;

#[derive(Serialize, Deserialize)]
struct PmfxDependency {
    name: String,
    timestamp: f64
}

#[derive(Serialize, Deserialize)]
struct PmfxInputStruct {
    name: String,
    semantic_index: u32,
    semantic_id: u32,
    size: u32,
    element_size: u32,
    num_elements: u32,
    offset: u32
}

#[derive(Serialize, Deserialize)]
struct PmfxCBuffer {
    name: String,
    location: u32,
    space: i32
}

#[derive(Serialize, Deserialize)]
struct PmfxDescriptiorTable {
    name: String,
    data_type: String,
    dimension: String,
    table_type: String,
    unit: u32,
    space: i32
}

#[derive(Serialize, Deserialize)]
struct PmfxSampler {
    name: String,
    unit: u32
}

#[derive(Serialize, Deserialize)]
struct PmfxTechnique {
    name: String,
    vs: Option<String>,
    vs_file: Option<String>,
    ps: Option<String>,
    ps_file: Option<String>,
    cs: Option<String>,
    cs_file: Option<String>,
    vs_inputs: Vec<PmfxInputStruct>,
    instance_inputs: Vec<PmfxInputStruct>,
    vs_outputs: Vec<PmfxInputStruct>,
    cbuffers: Vec<PmfxCBuffer>,
    permutation_id: u32,
    permutation_option_mask: u32
}

#[derive(Serialize, Deserialize)]
struct PmfxShader {
    cmdline: String,
    files: Vec<PmfxDependency>,
    techniques: Vec<PmfxTechnique>
}

pub struct RenderTechnique<D: gfx::Device> {
    pub vs: Option<D::Shader>,
    pub fs: Option<D::Shader>,
    pub input_layout: Vec<gfx::InputElementInfo>,
    pub descriptor_layout: u32,
}

pub struct Pmfx<D: gfx::Device> {
    //_pipelines: Vec<gfx::RenderPipelineInfo<'a, D>>,
    render_techniques: HashMap<String, HashMap<String, RenderTechnique<D>>>
}

const SEMANTIC_NAMES : [&str; 9] = [
    "SV_POSITION",
    "POSITION",
    "TEXCOORD",
    "NORMAL",
    "TANGENT",
    "BITANGENT",
    "BLENDWEIGHTS",
    "COLOR",
    "BLENDINDICES"
];

fn get_format_for_semantic(semantic_size: u32, num_elements: u32) -> gfx::Format {
    match semantic_size {
        4 => match num_elements {
            1 => gfx::Format::R32f,
            2 => gfx::Format::RG32f,
            3 => gfx::Format::RGB32f,
            4 => gfx::Format::RGBA32f,
            _ => panic!("hotline::pmfx: unsupported vertex element size!")
        }
        1 => match num_elements {
            4 => gfx::Format::RGBA8u,
            _ => panic!("hotline::pmfx: unsupported vertex element size!")
        }
        _ => panic!("hotline::pmfx: unsupported sematic size!")
    }
}

fn create_shader_from_file<D: gfx::Device>(device: &D, folder: &Path, file: Option<String>) -> Result<Option<D::Shader>, super::Error> {
    if let Some(shader) = file {
        let shader_filepath = folder.join(shader);
        let shader_data = fs::read(shader_filepath)?;                
        let shader_info = gfx::ShaderInfo {
            shader_type: gfx::ShaderType::Vertex,
            compile_info: None
        };
        device.create_shader(&shader_info, &shader_data)?;
    }
    Ok(None)
}

fn create_input_layout_from_technique(vs_inputs: Vec<PmfxInputStruct>, instance_inputs: Vec<PmfxInputStruct>) -> Vec<gfx::InputElementInfo> {
    let mut input_layout : Vec<gfx::InputElementInfo> = Vec::new();
    for input in vs_inputs {
        input_layout.push(gfx::InputElementInfo {
            semantic: String::from(SEMANTIC_NAMES[input.semantic_id as usize]),
            index: input.semantic_index,
            format: get_format_for_semantic(input.element_size, input.num_elements),
            input_slot: input.semantic_index,
            aligned_byte_offset: input.offset,
            input_slot_class: gfx::InputSlotClass::PerVertex,
            step_rate: 0,
        });
    }
    // TODO: instancing
    assert_eq!(instance_inputs.len(), 0);
    input_layout
}

impl<D> Pmfx<D> where D: gfx::Device {
    pub fn create() -> Self {
        Pmfx {
            //_pipelines: Vec::new()
            render_techniques: HashMap::new()
        }
    }

    /// load a pmfx friom a folder, where the folder contains a pmfx info.json and shader source in separate files
    /// within the directory
    pub fn load(&mut self, device: &D, filepath: &str) -> Result<(), super::Error> {
        let folder = Path::new(filepath);
        
        // get the name for indexing by pmfx name/folder
        let pmfx_name = if let Some(name) = folder.file_name() {
            String::from(name.to_os_string().to_str().unwrap())
        }
        else {
            String::from(filepath)
        };

        let mut pmfx_techniques = HashMap::new();
        
        //  parse pmfx itself
        let info_filepath = folder.join("info.json");
        let pmfx_data = fs::read(info_filepath).unwrap();
        let shader: PmfxShader = serde_json::from_slice(&pmfx_data).unwrap();

        // into techniques
        for technique in shader.techniques {
            if let Some(_cs_file) = technique.cs {
                // compute technique
                panic!();
            }
            else {
                // render technique
                let rt = RenderTechnique::<D> {
                    input_layout: create_input_layout_from_technique(technique.vs_inputs, technique.instance_inputs),
                    vs: create_shader_from_file(device, &folder, technique.vs_file)?,
                    fs: create_shader_from_file(device, &folder, technique.ps_file)?,
                    descriptor_layout: 0
                };

                pmfx_techniques.insert(technique.name, rt);
            }
        }

        self.render_techniques.insert(pmfx_name, pmfx_techniques);
        Ok(())
    }

    pub fn get_technique(&self, pmfx_name: &str, technique_name: &str) -> Option<&RenderTechnique<D>> {
        if let Some(pmfx) = self.render_techniques.get(pmfx_name) {
            if let Some(technique) = pmfx.get(technique_name) {
                Some(technique)
            }
            else {
                None
            }
        }
        else {
            None
        }
    }
}

