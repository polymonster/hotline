use std::collections::HashMap;
use std::error::Error;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::string::FromUtf8Error;

use glob::glob;
use serde::{Deserialize, Serialize};

use crate::spirv_cross_bindings::*;

fn print_cstr(msg: *const ::std::os::raw::c_char) {
    println!("{}", cstr_to_string(msg).unwrap());
}

unsafe extern "C" fn error_callback(
    _: *mut ::std::os::raw::c_void,
    error: *const ::std::os::raw::c_char,
) {
    print_cstr(error);
}

fn cstr_to_string(msg: *const ::std::os::raw::c_char) -> Result<String, FromUtf8Error> {
    let mut buf: Vec<u8> = Vec::new();
    unsafe {
        let mut msg_iter = msg;
        loop {
            if *msg_iter != 0 {
                buf.push(*msg_iter as u8);
            } else {
                break;
            }
            msg_iter = msg_iter.offset(1);
        }
    }
    String::from_utf8(buf)
}

fn load_spirv_file(path: &str) -> Vec<u32> {
    println!("{}", path);

    let mut file = File::open(path).expect("failed to open .spv file");
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .expect("failed to read .spv file");

    // Convert byte buffer to u32 vector
    assert!(
        buffer.len() % 4 == 0,
        ".spv file must align to 32-bit words"
    );
    buffer
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
enum ShaderStage {
    Vertex,
    Fragment,
    Compute,
    All,
}

#[derive(Serialize, Deserialize, Clone)]
struct Resource {
    name: String,
    visibility: ShaderStage,
}

#[derive(Serialize, Deserialize, Clone)]
struct PipelineLayout {
    bindings: Vec<Resource>,
    push_constants: Vec<Resource>,
    static_samplers: Vec<Resource>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Pipeline {
    vs: Option<String>,
    ps: Option<String>,
    cs: Option<String>,
    lib: Option<Vec<String>>,
    pipeline_layout: PipelineLayout,
}
type PipelinePermutations = HashMap<String, Pipeline>;

#[derive(Serialize, Deserialize, Clone)]
struct Pmfx {
    pipelines: HashMap<String, PipelinePermutations>,
}

fn compile_shader_spirv(
    filepath: &str,
    input_dir: &str,
    output_dir: &str,
    pipeline: &Pipeline,
    stage: ShaderStage,
) -> Result<(), Box<dyn Error>> {
    unsafe {
        let temp_spirv = filepath
            .replace(".vsc", ".spirv")
            .replace(".psc", ".spirv")
            .replace(".csc", ".spirv");

        let spirv_file = format!("{}/{}", input_dir, temp_spirv);
        let output_file = format!("{}/{}", output_dir, filepath);

        let spirv_binary = load_spirv_file(&spirv_file);

        let mut ctx = std::ptr::null_mut();
        let res = spvc_context_create(&mut ctx);
        assert_eq!(res, spvc_result_SPVC_SUCCESS);

        // set error callback
        spvc_context_set_error_callback(ctx, Some(error_callback), std::ptr::null_mut());

        // parse IR
        let mut ir = std::ptr::null_mut();
        let result =
            spvc_context_parse_spirv(ctx, spirv_binary.as_ptr(), spirv_binary.len(), &mut ir);
        assert_eq!(result, spvc_result_SPVC_SUCCESS);

        // create a pssl compiler
        let mut compiler = std::ptr::null_mut();
        spvc_context_create_compiler(
            ctx,
            spvc_backend_SPVC_BACKEND_MSL,
            ir,
            spvc_capture_mode_SPVC_CAPTURE_MODE_TAKE_OWNERSHIP,
            &mut compiler,
        );

        // create compiler options
        let mut compiler_options = std::ptr::null_mut();
        let result = spvc_compiler_create_compiler_options(compiler, &mut compiler_options);
        assert_eq!(result, spvc_result_SPVC_SUCCESS);

        // set compiler options

        // set MSL version
        spvc_compiler_options_set_uint(
            compiler_options,
            spvc_compiler_option_SPVC_COMPILER_OPTION_MSL_VERSION,
            202300, // example: MSL version 2.3.0
        );

        // Enable MSL argument buffers
        spvc_compiler_options_set_bool(
            compiler_options,
            spvc_compiler_option_SPVC_COMPILER_OPTION_MSL_ARGUMENT_BUFFERS,
            1,
        );

        spvc_compiler_options_set_bool(
            compiler_options,
            spvc_compiler_option_SPVC_COMPILER_OPTION_MSL_FORCE_ACTIVE_ARGUMENT_BUFFER_RESOURCES,
            1,
        );

        // Set argument buffer tier (0 or 1)
        spvc_compiler_options_set_uint(
            compiler_options,
            spvc_compiler_option_SPVC_COMPILER_OPTION_MSL_ARGUMENT_BUFFERS_TIER,
            1,
        );

        let result = spvc_compiler_install_compiler_options(compiler, compiler_options);
        assert_eq!(result, spvc_result_SPVC_SUCCESS);

        // set bindings

        // Assume you already have a valid compiler and resources
        let mut resources: spvc_resources = std::ptr::null_mut();
        let result = spvc_compiler_create_shader_resources(compiler, &mut resources);
        assert_eq!(result, spvc_result_SPVC_SUCCESS);

        let resource_types = vec![
            spvc_resource_type_SPVC_RESOURCE_TYPE_UNIFORM_BUFFER,
            spvc_resource_type_SPVC_RESOURCE_TYPE_STORAGE_BUFFER,
            spvc_resource_type_SPVC_RESOURCE_TYPE_SEPARATE_IMAGE,
            spvc_resource_type_SPVC_RESOURCE_TYPE_STORAGE_IMAGE,
            spvc_resource_type_SPVC_RESOURCE_TYPE_SAMPLED_IMAGE,
            spvc_resource_type_SPVC_RESOURCE_TYPE_SEPARATE_SAMPLERS,
        ];

        let resources: Vec<_> = resource_types
            .into_iter()
            .flat_map(|x| {
                // Choose the resource type you want to query
                let resource_type = x;

                // Prepare output pointers
                let mut resource_list: *const spvc_reflected_resource = std::ptr::null();
                let mut resource_count: usize = 0;

                // Get the list of resources of the given type
                let list_result = spvc_resources_get_resource_list_for_type(
                    resources,
                    resource_type,
                    &mut resource_list,
                    &mut resource_count,
                );
                assert_eq!(list_result, spvc_result_SPVC_SUCCESS);

                (0..resource_count).map(move |i| *resource_list.add(i))
            })
            .collect();

        let samplers_offset = match stage
        {
            ShaderStage::Vertex => 2,
            _ => 0
        };

        // put samplers first all in single argument buffer
        for resource in &resources {
            for push_constant in &pipeline.pipeline_layout.static_samplers {
                let name = cstr_to_string(resource.name)?;
                if push_constant.name == name.strip_prefix("type.").unwrap_or(&name) {
                    spvc_compiler_set_decoration(
                        compiler,
                        resource.id,
                        SpvDecoration__SpvDecorationDescriptorSet,
                        samplers_offset as u32,
                    );
                }
            }
        }

        // put push constants next - use discrete descriptor sets (no argument buffers)
        // This enables using setVertexBytes/setFragmentBytes at runtime
        let mut binding_offset = samplers_offset + 1;
        let exec_model = match stage {
            ShaderStage::Vertex => SpvExecutionModel__SpvExecutionModelVertex,
            ShaderStage::Fragment => SpvExecutionModel__SpvExecutionModelFragment,
            ShaderStage::Compute => SpvExecutionModel__SpvExecutionModelGLCompute,
            _ => SpvExecutionModel__SpvExecutionModelVertex,
        };

        for resource in &resources {
            for push_constant in &pipeline.pipeline_layout.push_constants {
                // Only process push constants visible to this shader stage
                if push_constant.visibility != stage && push_constant.visibility != ShaderStage::All {
                    continue;
                }
                let name = cstr_to_string(resource.name)?;
                if push_constant.name == name.strip_prefix("type.").unwrap_or(&name) {
                    let desc_set = binding_offset as u32;
                    spvc_compiler_set_decoration(
                        compiler,
                        resource.id,
                        SpvDecoration__SpvDecorationDescriptorSet,
                        desc_set,
                    );
                    // Mark as discrete so it uses direct buffer binding, not argument buffer
                    spvc_compiler_msl_add_discrete_descriptor_set(compiler, desc_set);

                    // Explicitly map to Metal buffer index (without this, SPIRV-Cross uses buffer(0))
                    // Get the original SPIR-V binding number from the resource
                    let spirv_binding = spvc_compiler_get_decoration(
                        compiler,
                        resource.id,
                        SpvDecoration__SpvDecorationBinding,
                    );
                    let mut res_binding: spvc_msl_resource_binding = std::mem::zeroed();
                    spvc_msl_resource_binding_init(&mut res_binding);
                    res_binding.stage = exec_model;
                    res_binding.desc_set = desc_set;
                    res_binding.binding = spirv_binding;
                    res_binding.msl_buffer = desc_set;
                    spvc_compiler_msl_add_resource_binding(compiler, &res_binding);

                    binding_offset += 1
                }
            }
        }

        // set bindings based on pipeline layout
        let mut binding_sub_offset = 0;
        for resource in &resources {
            for (_, binding) in pipeline.pipeline_layout.bindings.iter().enumerate() {
                if binding.visibility == stage || binding.visibility == ShaderStage::All {
                    let name = cstr_to_string(resource.name)?;
                    if &binding.name == name.strip_prefix("type.").unwrap_or(&name) {
                        spvc_compiler_set_decoration(
                            compiler,
                            resource.id,
                            SpvDecoration__SpvDecorationDescriptorSet,
                            binding_offset as u32,
                        );
                        spvc_compiler_set_decoration(
                            compiler,
                            resource.id,
                            SpvDecoration__SpvDecorationBinding,
                            binding_sub_offset as u32,
                        );
                        binding_sub_offset += 1;
                    }
                }
            }
        }

        let mut msl_src = std::ptr::null();
        let result = spvc_compiler_compile(compiler, &mut msl_src);
        if result == spvc_result_SPVC_ERROR_UNSUPPORTED_SPIRV {
            println!("spirv_to_pssl: spirv binary is unsupported");
            spvc_context_destroy(ctx);
        }

        if result != spvc_result_SPVC_SUCCESS {
            return Err(format!("SPIRV-Cross compilation failed for {}", filepath).into());
        }

        let msl_source = cstr_to_string(msl_src)?;

        // Ensure output directory exists
        if let Some(parent) = Path::new(&output_file).parent() {
            fs::create_dir_all(parent)?;
        }

        // Write MSL source to temp .metal file
        let temp_metal_file = format!("{}.metal", output_file);
        fs::write(&temp_metal_file, &msl_source)?;

        // Compile .metal to .air
        let air_file = format!("{}.air", output_file);
        let compile_status = Command::new("xcrun")
            .args([
                "-sdk",
                "macosx",
                "metal",
                "-c",
                "-frecord-sources",
                &temp_metal_file,
                "-o",
                &air_file,
            ])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;

        if !compile_status.success() {
            return Err(format!("Metal compilation failed for {}", temp_metal_file).into());
        }

        // Link .air to final output (metallib)
        let link_status = Command::new("xcrun")
            .args(["-sdk", "macosx", "metal", &air_file, "-o", &output_file])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;

        if !link_status.success() {
            return Err(format!("Metal linking failed for {}", air_file).into());
        }

        // Clean up intermediate files
        // let _ = fs::remove_file(&temp_metal_file);
        let _ = fs::remove_file(&air_file);

        println!("Compiled Metal shader: {}", output_file);
        Ok(())
    }
}

pub fn compile_piepline(
    filepath: &str,
    input_dir: &str,
    output_dir: &str,
) -> Result<(), Box<dyn Error>> {
    let file_data = std::fs::read(filepath).unwrap();
    let file: Pmfx = serde_json::from_slice(&file_data)?;
    for (_, permutation) in file.pipelines {
        for (_, pipeline) in &permutation {
            if let Some(vs) = &pipeline.vs {
                compile_shader_spirv(&vs, input_dir, output_dir, &pipeline, ShaderStage::Vertex)?;
            }
            if let Some(ps) = &pipeline.ps {
                compile_shader_spirv(&ps, input_dir, output_dir, &pipeline, ShaderStage::Fragment)?;
            }
            if let Some(cs) = &pipeline.cs {
                compile_shader_spirv(&cs, input_dir, output_dir, &pipeline, ShaderStage::Compute)?;
            }
        }
    }

    Ok(())
}

pub fn compile_dir(input_dir: &str, output_dir: &str) -> Result<(), Box<dyn Error>> {
    let temp_dir = "target/temp/shaders";

    // Use CARGO_MANIFEST_DIR to locate pmfx.py relative to this crate
    let pmfx_path = format!(
        "{}/third_party/pmfx-shader/pmfx.py",
        env!("CARGO_MANIFEST_DIR")
    );

    let status = Command::new("python3")
        .args(&[
            &pmfx_path,
            "-shader_platform",
            "spirv",
            "-shader_version",
            "6_5",
            "-i",
            input_dir,
            "-o",
            output_dir,
            "-t",
            temp_dir,
            "-num_threads",
            "1",
            "-f",
            "-args",
            "-Zpr",
            "-ignores",
            "raytracing",
            "compute_frustum_cull",
            "mesh_lit_rt_shadow",
            "mesh_lit_rt_shadow2",
            "mip_chain_texture2d",
            "heightmap_mrt_resolve",
        ])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .expect("failed compiling pmfx");

    assert!(status.code().unwrap() == 0);

    for entry in glob(&format!("{output_dir}/**/*.json")).expect("") {
        if let Ok(path) = entry {
            compile_piepline(path.to_str().unwrap(), temp_dir, output_dir)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn run() {
        super::compile_dir("../hotline/shaders", "target/shaders").unwrap();
    }
}
