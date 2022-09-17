
use serde::{Deserialize, Serialize};
// use serde_json::Result;

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
    vs: String,
    vs_file: String,
    ps: String,
    ps_file: String,
    name: String,
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

pub struct Pmfx<'a, D: gfx::Device> {
    pipelines: Vec<gfx::RenderPipelineInfo<'a, D>>
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

impl<'a, D> Pmfx<'a, D> where D: gfx::Device {
    pub fn create() -> Self {
        Pmfx {
            pipelines: Vec::new()
        }
    }

    pub fn load_shader(&mut self, filepath: &str) {
        let pmfx_data = fs::read(filepath).unwrap();
        let shader: PmfxShader = serde_json::from_slice(&pmfx_data).unwrap();

        for technique in shader.techniques {
            let mut input_layout : Vec<gfx::InputElementInfo> = Vec::new();
            for input in technique.vs_inputs {
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
        }
    }

    pub fn load_pipeline(&mut self) {

    }
}

