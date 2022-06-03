
use serde::{Deserialize, Serialize};
use serde_json::Result;

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

impl<'a, D> Pmfx<'a, D> where D: gfx::Device {
    pub fn create() -> Self {
        Pmfx {
            pipelines: Vec::new()
        }
    }

    pub fn load_shader(&mut self, filepath: &str) {
        let pmfx_data = fs::read(filepath).unwrap();
        let shader: PmfxShader = serde_json::from_slice(&pmfx_data).unwrap();

        let a = 0;
    }

    pub fn load_pipeline(&mut self) {

    }
}

