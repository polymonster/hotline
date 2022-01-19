use crate::os::Window;

#[cfg(target_os = "windows")]
use crate::os::win32 as platform;

use windows::{
    core::*, Win32::Foundation::*, Win32::Graphics::Direct3D::Fxc::*, Win32::Graphics::Direct3D::*,
    Win32::Graphics::Direct3D12::*, Win32::Graphics::Dxgi::Common::*, Win32::Graphics::Dxgi::*,
    Win32::System::Threading::*, Win32::System::WindowsProgramming::*,
};

use super::*;
use std::ffi::CStr;
use std::str;

/// Indicates the number of backbuffers used for swap chains and command buffers.
const NUM_BB: u32 = 2;

pub struct Device {
    name: String,
    adapter: IDXGIAdapter1,
    dxgi_factory: IDXGIFactory4,
    device: ID3D12Device,
    command_allocator: ID3D12CommandAllocator,
    command_list: ID3D12GraphicsCommandList,
    command_queue: ID3D12CommandQueue,
    shader_heap: ID3D12DescriptorHeap,
}

pub struct SwapChain {
    width: i32,
    height: i32,
    flags: u32,
    frame_index: i32,
    bb_index: i32,
    swap_chain: IDXGISwapChain3,
    rtv_heap: ID3D12DescriptorHeap,
    rtv_handles: Vec<D3D12_CPU_DESCRIPTOR_HANDLE>,
    fence: ID3D12Fence,
    fence_last_signalled_value: u64,
    fence_event: HANDLE,
    frame_fence_value: [u64; NUM_BB as usize],
    readback_buffer: Option<ID3D12Resource>,
}

pub struct Pipeline {
    pso: ID3D12PipelineState,
    root_signature: ID3D12RootSignature,
}

pub struct CmdBuf {
    bb_index: usize,
    command_allocator: Vec<ID3D12CommandAllocator>,
    command_list: Vec<ID3D12GraphicsCommandList>,
    in_flight_barriers: Vec<Vec<D3D12_RESOURCE_BARRIER>>,
}

pub struct Buffer {
    resource: ID3D12Resource,
    vbv: Option<D3D12_VERTEX_BUFFER_VIEW>,
    ibv: Option<D3D12_INDEX_BUFFER_VIEW>,
}

pub struct Shader {
    blob: ID3DBlob,
}

pub struct Texture {
    resource: ID3D12Resource,
    srv: D3D12_CPU_DESCRIPTOR_HANDLE,
    gpu: D3D12_GPU_DESCRIPTOR_HANDLE,
}

#[derive(Clone)]
pub struct ReadBackRequest {
    pub resource: Option<ID3D12Resource>,
    pub fence_value: u64,
    pub size: usize,
    pub row_pitch: usize,
    pub slice_pitch: usize,
}

fn to_dxgi_format(format: super::Format) -> DXGI_FORMAT {
    match format {
        super::Format::Unknown => DXGI_FORMAT_UNKNOWN,
        super::Format::R16n => DXGI_FORMAT_R16_UNORM,
        super::Format::R16u => DXGI_FORMAT_R16_UINT,
        super::Format::R16i => DXGI_FORMAT_R16_SINT,
        super::Format::R16f => DXGI_FORMAT_R16_FLOAT,
        super::Format::R32u => DXGI_FORMAT_R32_UINT,
        super::Format::R32i => DXGI_FORMAT_R32_SINT,
        super::Format::R32f => DXGI_FORMAT_R32_FLOAT,
        super::Format::RG32u => DXGI_FORMAT_R32G32_UINT,
        super::Format::RG32i => DXGI_FORMAT_R32G32_SINT,
        super::Format::RG32f => DXGI_FORMAT_R32G32_FLOAT,
        super::Format::RGB32u => DXGI_FORMAT_R32G32B32_UINT,
        super::Format::RGB32i => DXGI_FORMAT_R32G32B32_SINT,
        super::Format::RGB32f => DXGI_FORMAT_R32G32B32_FLOAT,
        super::Format::RGBA32u => DXGI_FORMAT_R32G32B32A32_UINT,
        super::Format::RGBA32i => DXGI_FORMAT_R32G32B32A32_SINT,
        super::Format::RGBA32f => DXGI_FORMAT_R32G32B32A32_FLOAT,
    }
}

fn to_d3d12_compile_flags(flags: &super::ShaderCompileFlags) -> u32 {
    let mut d3d12_flags = 0;
    if flags.contains(super::CompileFlags::SkipOptimization) {
        d3d12_flags |= D3DCOMPILE_SKIP_OPTIMIZATION;
    }
    if flags.contains(super::CompileFlags::Debug) {
        d3d12_flags |= D3DCOMPILE_DEBUG;
    }
    d3d12_flags
}

fn to_d3d12_shader_visibility(visibility: super::ShaderVisibility) -> D3D12_SHADER_VISIBILITY {
    match visibility {
        super::ShaderVisibility::All => D3D12_SHADER_VISIBILITY_ALL,
        super::ShaderVisibility::Vertex => D3D12_SHADER_VISIBILITY_VERTEX,
        super::ShaderVisibility::Fragment => D3D12_SHADER_VISIBILITY_PIXEL,
    }
}

fn to_d3d12_sampler_boarder_colour(col: Option<u32>) -> D3D12_STATIC_BORDER_COLOR {
    if col.is_some() {
        return D3D12_STATIC_BORDER_COLOR::from(col.unwrap() as i32);
    }
    D3D12_STATIC_BORDER_COLOR_TRANSPARENT_BLACK
}

fn to_d3d12_filter(filter: super::SamplerFilter) -> D3D12_FILTER {
    match filter {
        super::SamplerFilter::Point => D3D12_FILTER_MIN_MAG_MIP_POINT,
        super::SamplerFilter::Linear => D3D12_FILTER_MIN_MAG_MIP_LINEAR,
        super::SamplerFilter::Anisotropic => D3D12_FILTER_ANISOTROPIC,
    }
}

fn to_d3d12_address_mode(mode: super::SamplerAddressMode) -> D3D12_TEXTURE_ADDRESS_MODE {
    match mode {
        super::SamplerAddressMode::Wrap => D3D12_TEXTURE_ADDRESS_MODE_WRAP,
        super::SamplerAddressMode::Mirror => D3D12_TEXTURE_ADDRESS_MODE_MIRROR,
        super::SamplerAddressMode::Clamp => D3D12_TEXTURE_ADDRESS_MODE_CLAMP,
        super::SamplerAddressMode::Border => D3D12_TEXTURE_ADDRESS_MODE_BORDER,
        super::SamplerAddressMode::MirrorOnce => D3D12_TEXTURE_ADDRESS_MODE_MIRROR_ONCE,
    }
}

fn to_d3d12_address_comparison_func(func: Option<super::ComparisonFunc>) -> D3D12_COMPARISON_FUNC {
    if func.is_some() {
        return match func.unwrap() {
            super::ComparisonFunc::Never => D3D12_COMPARISON_FUNC_NEVER,
            super::ComparisonFunc::Less => D3D12_COMPARISON_FUNC_LESS,
            super::ComparisonFunc::Equal => D3D12_COMPARISON_FUNC_EQUAL,
            super::ComparisonFunc::LessEqual => D3D12_COMPARISON_FUNC_LESS_EQUAL,
            super::ComparisonFunc::Greater => D3D12_COMPARISON_FUNC_GREATER,
            super::ComparisonFunc::NotEqual => D3D12_COMPARISON_FUNC_NOT_EQUAL,
            super::ComparisonFunc::GreaterEqual => D3D12_COMPARISON_FUNC_GREATER_EQUAL,
            super::ComparisonFunc::Always => D3D12_COMPARISON_FUNC_ALWAYS,
        }
    }
    D3D12_COMPARISON_FUNC_ALWAYS
}

fn transition_barrier(
    resource: &ID3D12Resource,
    state_before: D3D12_RESOURCE_STATES,
    state_after: D3D12_RESOURCE_STATES,
) -> D3D12_RESOURCE_BARRIER {
    let trans = std::mem::ManuallyDrop::new(D3D12_RESOURCE_TRANSITION_BARRIER {
        pResource: Some(resource.clone()),
        StateBefore: state_before,
        StateAfter: state_after,
        Subresource: D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES,
    });
    D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 { Transition: trans },
    }
}

fn get_hardware_adapter(factory: &IDXGIFactory4) -> Result<IDXGIAdapter1> {
    unsafe {
        for i in 0.. {
            let adapter = factory.EnumAdapters1(i)?;
            let desc = adapter.GetDesc1()?;

            if (DXGI_ADAPTER_FLAG::from(desc.Flags) & DXGI_ADAPTER_FLAG_SOFTWARE)
                != DXGI_ADAPTER_FLAG_NONE
            {
                // Don't select the Basic Render Driver adapter. If you want a
                // software adapter, pass in "/warp" on the command line.
                continue;
            }

            // Check to see whether the adapter supports Direct3D 12, but don't
            // create the actual device yet.
            if D3D12CreateDevice(
                &adapter,
                D3D_FEATURE_LEVEL_12_1,
                std::ptr::null_mut::<Option<ID3D12Device>>(),
            )
            .is_ok()
            {
                return Ok(adapter);
            }
        }
    }
    unreachable!()
}

fn create_root_signature(device: &ID3D12Device) -> Result<ID3D12RootSignature> {
    let mut range = D3D12_DESCRIPTOR_RANGE {
        RangeType: D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
        NumDescriptors: 1,
        BaseShaderRegister: 0,
        RegisterSpace: 0,
        OffsetInDescriptorsFromTableStart: 0,
    };

    let mut params: [D3D12_ROOT_PARAMETER; 2] = [
        D3D12_ROOT_PARAMETER {
            ParameterType: D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS,
            Anonymous: D3D12_ROOT_PARAMETER_0 {
                Constants: D3D12_ROOT_CONSTANTS {
                    ShaderRegister: 0,
                    RegisterSpace: 0,
                    Num32BitValues: 4,
                },
            },
            ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
        },
        D3D12_ROOT_PARAMETER {
            ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
            Anonymous: D3D12_ROOT_PARAMETER_0 {
                DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE {
                    NumDescriptorRanges: 1,
                    pDescriptorRanges: &mut range,
                },
            },
            ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
        },
    ];

    let mut sampler = D3D12_STATIC_SAMPLER_DESC {
        Filter: D3D12_FILTER_MIN_MAG_MIP_LINEAR,
        AddressU: D3D12_TEXTURE_ADDRESS_MODE_WRAP,
        AddressV: D3D12_TEXTURE_ADDRESS_MODE_WRAP,
        AddressW: D3D12_TEXTURE_ADDRESS_MODE_WRAP,
        MipLODBias: 0.0,
        MaxAnisotropy: 0,
        ComparisonFunc: D3D12_COMPARISON_FUNC_ALWAYS,
        BorderColor: D3D12_STATIC_BORDER_COLOR_TRANSPARENT_BLACK,
        MinLOD: 0.0,
        MaxLOD: 0.0,
        ShaderRegister: 0,
        RegisterSpace: 0,
        ShaderVisibility: D3D12_SHADER_VISIBILITY_PIXEL,
    };

    let desc = D3D12_ROOT_SIGNATURE_DESC {
        NumParameters: params.len() as u32,
        Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
        pParameters: params.as_mut_ptr(),
        NumStaticSamplers: 1,
        pStaticSamplers: &mut sampler,
        ..Default::default()
    };

    let mut signature = None;

    let signature = unsafe {
        D3D12SerializeRootSignature(
            &desc,
            D3D_ROOT_SIGNATURE_VERSION_1,
            &mut signature,
            std::ptr::null_mut(),
        )
    }
    .map(|()| signature.unwrap())?;

    unsafe {
        device.CreateRootSignature(0, signature.GetBufferPointer(), signature.GetBufferSize())
    }
}

fn create_read_back_buffer(device: &Device, size: u64) -> Option<ID3D12Resource> {
    let mut readback_buffer: Option<ID3D12Resource> = None;
    unsafe {
        // readback buffer
        if !device
            .device
            .CreateCommittedResource(
                &D3D12_HEAP_PROPERTIES {
                    Type: D3D12_HEAP_TYPE_READBACK,
                    ..Default::default()
                },
                D3D12_HEAP_FLAG_NONE,
                &D3D12_RESOURCE_DESC {
                    Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                    Width: size,
                    Height: 1,
                    DepthOrArraySize: 1,
                    MipLevels: 1,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    Format: DXGI_FORMAT_UNKNOWN,
                    Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                    ..Default::default()
                },
                D3D12_RESOURCE_STATE_COPY_DEST,
                std::ptr::null(),
                &mut readback_buffer,
            )
            .is_ok()
        {
            panic!("hotline::gfx::d3d12: failed to create readback buffer");
        }
    }
    readback_buffer
}

fn create_swap_chain_rtv(
    swap_chain: &IDXGISwapChain3,
    device: &ID3D12Device,
) -> (ID3D12DescriptorHeap, Vec<D3D12_CPU_DESCRIPTOR_HANDLE>) {
    unsafe {
        // create rtv heap
        let rtv_heap_result = device.CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
            NumDescriptors: NUM_BB,
            Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
            ..Default::default()
        });
        if !rtv_heap_result.is_ok() {
            panic!("hotline::gfx::d3d12: failed to create rtv heap for swap chain");
        }

        let rtv_heap: ID3D12DescriptorHeap = rtv_heap_result.unwrap();
        let rtv_descriptor_size =
            device.GetDescriptorHandleIncrementSize(D3D12_DESCRIPTOR_HEAP_TYPE_RTV) as usize;
        let rtv_handle = rtv_heap.GetCPUDescriptorHandleForHeapStart();

        // render targets for the swap chain
        let mut handles: Vec<D3D12_CPU_DESCRIPTOR_HANDLE> = Vec::new();

        for i in 0..NUM_BB {
            let render_target: ID3D12Resource = swap_chain.GetBuffer(i).unwrap();
            let h = D3D12_CPU_DESCRIPTOR_HANDLE {
                ptr: rtv_handle.ptr + i as usize * rtv_descriptor_size,
            };
            device.CreateRenderTargetView(&render_target, std::ptr::null_mut(), &h);
            handles.push(h);
        }
        (rtv_heap, handles)
    }
}

impl super::Buffer<Device> for Buffer {}
impl super::Shader<Device> for Shader {}
impl super::Pipeline<Device> for Pipeline {}
impl super::Texture<Device> for Texture {}

impl Device {
    fn create_input_layout(&self, layout: &super::InputLayout, d3d12_elems : &mut Vec<D3D12_INPUT_ELEMENT_DESC>) -> D3D12_INPUT_LAYOUT_DESC {
        for elem in layout {
            d3d12_elems.push(D3D12_INPUT_ELEMENT_DESC{
                SemanticName: PSTR(elem.semantic.as_ptr()  as _),
                SemanticIndex: elem.index,
                Format: to_dxgi_format(elem.format),
                InputSlot: elem.input_slot,
                AlignedByteOffset: elem.aligned_byte_offset,
                InputSlotClass: match elem.input_slot_class {
                    super::InputSlotClass::PerVertex => D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
                    super::InputSlotClass::PerInstance => D3D12_INPUT_CLASSIFICATION_PER_INSTANCE_DATA
                },
                InstanceDataStepRate: elem.step_rate,
            })
        }
        D3D12_INPUT_LAYOUT_DESC {
            pInputElementDescs: d3d12_elems.as_mut_ptr(),
            NumElements: d3d12_elems.len() as u32
        }
    }

    fn create_root_signature(&mut self, layout: &super::DescriptorLayout) -> Result<ID3D12RootSignature> {
        let mut root_params : Vec<D3D12_ROOT_PARAMETER> = Vec::new();
        let mut static_samplers : Vec<D3D12_STATIC_SAMPLER_DESC> = Vec::new();
        // push constants
        if layout.constants.is_some() {
            let constants_set = layout.constants.as_ref();
            for constants in constants_set.unwrap() {
                root_params.push(D3D12_ROOT_PARAMETER {
                    ParameterType: D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS,
                    Anonymous: D3D12_ROOT_PARAMETER_0 {
                        Constants: D3D12_ROOT_CONSTANTS {
                            ShaderRegister: constants.shader_register,
                            RegisterSpace: constants.register_space,
                            Num32BitValues: constants.num_values,
                        },
                    },
                    ShaderVisibility: to_d3d12_shader_visibility(constants.visibility)
                });
            }   
        }
        // tables for (SRV, UAV, CBV an Samplers)
        if layout.tables.is_some() {
            let table_info = layout.tables.as_ref();
            let mut descriptor_offset = 0;
            for table in table_info.unwrap() {
                let count = if table.num_descriptors.is_some() { table.num_descriptors.unwrap() } else { u32::MAX };
                let mut range = D3D12_DESCRIPTOR_RANGE {
                    RangeType: match table.table_type {
                        super::DescriptorTableType::ShaderResource => D3D12_DESCRIPTOR_RANGE_TYPE_SRV,
                        super::DescriptorTableType::UnorderedAccess => D3D12_DESCRIPTOR_RANGE_TYPE_UAV,
                        super::DescriptorTableType::ConstantBuffer => D3D12_DESCRIPTOR_RANGE_TYPE_CBV,
                        super::DescriptorTableType::Sampler => D3D12_DESCRIPTOR_RANGE_TYPE_SAMPLER,
                    },
                    NumDescriptors: count,
                    BaseShaderRegister: table.shader_register,
                    RegisterSpace: table.register_space,
                    OffsetInDescriptorsFromTableStart: descriptor_offset,
                };
                descriptor_offset = descriptor_offset + count;
                root_params.push(D3D12_ROOT_PARAMETER{
                    ParameterType: D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE,
                    Anonymous: D3D12_ROOT_PARAMETER_0 {
                        DescriptorTable: D3D12_ROOT_DESCRIPTOR_TABLE {
                            NumDescriptorRanges: 1,
                            pDescriptorRanges: &mut range,
                        },
                    },
                    ShaderVisibility: to_d3d12_shader_visibility(table.visibility),
                })
            }
        }
        // immutable samplers
        if layout.samplers.is_some() {
            let samplers = layout.samplers.as_ref();
            for sampler in samplers.unwrap() {
                static_samplers.push( D3D12_STATIC_SAMPLER_DESC {
                    Filter: to_d3d12_filter(sampler.filter),
                    AddressU: to_d3d12_address_mode(sampler.address_u),
                    AddressV: to_d3d12_address_mode(sampler.address_v),
                    AddressW: to_d3d12_address_mode(sampler.address_w),
                    MipLODBias: sampler.mip_lod_bias,
                    MaxAnisotropy: sampler.max_aniso,
                    ComparisonFunc: to_d3d12_address_comparison_func(sampler.comparison),
                    BorderColor: to_d3d12_sampler_boarder_colour(sampler.border_colour),
                    MinLOD: sampler.min_lod,
                    MaxLOD: sampler.max_lod,
                    ShaderRegister: sampler.shader_register,
                    RegisterSpace: sampler.register_space,
                    ShaderVisibility: to_d3d12_shader_visibility(sampler.shader_visibility),
                })
            }
        }

        // desc
        let desc = D3D12_ROOT_SIGNATURE_DESC {
            NumParameters: root_params.len() as u32,
            Flags: D3D12_ROOT_SIGNATURE_FLAG_ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT,
            pParameters: root_params.as_mut_ptr(),
            NumStaticSamplers: static_samplers.len() as u32,
            pStaticSamplers: static_samplers.as_mut_ptr(),
            ..Default::default()
        };
        
        // create signature
        unsafe {
            let mut signature = None;
            let signature = D3D12SerializeRootSignature(
                &desc,
                D3D_ROOT_SIGNATURE_VERSION_1,
                &mut signature,
                std::ptr::null_mut(),
            ).map(|()| signature.unwrap())?;

            self.device.CreateRootSignature(0, signature.GetBufferPointer(), signature.GetBufferSize())
        }
    }
}

impl super::Device for Device {
    type SwapChain = SwapChain;
    type CmdBuf = CmdBuf;
    type Buffer = Buffer;
    type Shader = Shader;
    type Pipeline = Pipeline;
    type Texture = Texture;
    type ReadBackRequest = ReadBackRequest;

    fn create() -> Device {
        unsafe {
            // enable debug layer
            let mut dxgi_factory_flags: u32 = 0;
            if cfg!(debug_assertions) {
                let mut debug: Option<ID3D12Debug> = None;
                if let Some(debug) = D3D12GetDebugInterface(&mut debug).ok().and_then(|_| debug) {
                    debug.EnableDebugLayer();
                    println!("hotline::gfx::d3d12: enabling debug layer");
                }
                dxgi_factory_flags = DXGI_CREATE_FACTORY_DEBUG;
            }

            // create dxgi factory
            let dxgi_factory = CreateDXGIFactory2(dxgi_factory_flags)
                .expect("hotline::gfx::d3d12: failed to create dxgi factory");

            // create adapter
            let adapter = get_hardware_adapter(&dxgi_factory)
                .expect("hotline::gfx::d3d12: failed to get hardware adapter");

            // create device
            let mut d3d12_device: Option<ID3D12Device> = None;
            let device_result =
                D3D12CreateDevice(adapter.clone(), D3D_FEATURE_LEVEL_11_0, &mut d3d12_device)
                    .expect("hotline::gfx::d3d12: failed to create d3d12 device");
            let device = d3d12_device.unwrap();

            // create command allocator
            let command_allocator = device
                .CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)
                .expect("hotline::gfx::d3d12: failed to create command allocator");

            // create command list
            let command_list = device
                .CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &command_allocator, None)
                .expect("hotline::gfx::d3d12: failed to create command list");

            // create queue
            let desc = D3D12_COMMAND_QUEUE_DESC {
                Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
                NodeMask: 1,
                ..Default::default()
            };
            let command_queue = device
                .CreateCommandQueue(&desc)
                .expect("hotline::gfx::d3d12: failed to create command queue");

            // create shader heap (srv, cbv, uav)
            let shader_heap = device
                .CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                    Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
                    NumDescriptors: 1,
                    Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
                    NodeMask: 0,
                })
                .expect("hotline::gfx::d3d12: failed to create shader heap");

            // initialise struct
            Device {
                name: String::from("d3d12 device"),
                adapter: adapter,
                device: device,
                dxgi_factory: dxgi_factory,
                command_allocator: command_allocator,
                command_list: command_list,
                command_queue: command_queue,
                shader_heap: shader_heap,
            }
        }
    }

    fn create_swap_chain(&self, win: &platform::Window) -> SwapChain {
        unsafe {
            // set flags, these could be passed in
            let flags = DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0;

            // create swap chain desc
            let rect = win.get_rect();
            let swap_chain_desc = DXGI_SWAP_CHAIN_DESC1 {
                BufferCount: NUM_BB,
                Width: rect.width as u32,
                Height: rect.height as u32,
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                Flags: flags as u32,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    ..Default::default()
                },
                ..Default::default()
            };

            // create swap chain itself
            let swap_chain_result = self.dxgi_factory.CreateSwapChainForHwnd(
                &self.command_queue,
                win.get_native_handle(),
                &swap_chain_desc,
                std::ptr::null(),
                None,
            );
            if !swap_chain_result.is_ok() {
                panic!("hotline::gfx::d3d12: failed to create swap chain for window");
            }
            let swap_chain: IDXGISwapChain3 = swap_chain_result.unwrap().cast().unwrap();

            // create rtv heap and handles
            let rtv = create_swap_chain_rtv(&swap_chain, &self.device);
            let data_size = (rect.width * rect.height * 4) as u64;

            // initialise struct
            SwapChain {
                width: rect.width,
                height: rect.height,
                flags: flags as u32,
                bb_index: 0,
                fence: self.device.CreateFence(0, D3D12_FENCE_FLAG_NONE).unwrap(),
                fence_last_signalled_value: 0,
                fence_event: CreateEventA(std::ptr::null_mut(), false, false, None),
                swap_chain: swap_chain,
                rtv_heap: rtv.0,
                rtv_handles: rtv.1,
                frame_index: 0,
                frame_fence_value: [0, 0],
                readback_buffer: create_read_back_buffer(&self, data_size),
            }
        }
    }

    fn create_cmd_buf(&self) -> CmdBuf {
        unsafe {
            let mut command_allocators: Vec<ID3D12CommandAllocator> = Vec::new();
            let mut command_lists: Vec<ID3D12GraphicsCommandList> = Vec::new();
            let mut barriers: Vec<Vec<D3D12_RESOURCE_BARRIER>> = Vec::new();

            for _ in 0..NUM_BB as usize {
                // create command allocator
                let command_allocator =
                    self.device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT);
                if !command_allocator.is_ok() {
                    panic!("hotline::gfx::d3d12: failed to create command allocator");
                }
                let command_allocator = command_allocator.unwrap();

                // create command list
                let command_list = self.device.CreateCommandList(
                    0,
                    D3D12_COMMAND_LIST_TYPE_DIRECT,
                    &command_allocator,
                    None,
                );
                if !command_list.is_ok() {
                    panic!("hotline::gfx::d3d12: failed to create command list");
                }
                let command_list = command_list.unwrap();

                command_allocators.push(command_allocator);
                command_lists.push(command_list);

                barriers.push(Vec::new());
            }

            // initialise struct
            CmdBuf {
                bb_index: 1,
                command_allocator: command_allocators,
                command_list: command_lists,
                in_flight_barriers: barriers,
            }
        }
    }

    fn create_pipeline(&self, info: super::PipelineInfo<Device>) -> Pipeline {
        let root_signature = create_root_signature(&self.device).unwrap();

        let mut d3d12_elems : Vec<D3D12_INPUT_ELEMENT_DESC> = Vec::new();
        let input_layout = self.create_input_layout(&info.input_layout, &mut d3d12_elems);

        let vs = info.vs.unwrap().blob;
        let ps = info.fs.unwrap().blob;

        let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
            InputLayout: input_layout,
            pRootSignature: Some(root_signature.clone()),
            VS: D3D12_SHADER_BYTECODE {
                pShaderBytecode: unsafe { vs.GetBufferPointer() },
                BytecodeLength: unsafe { vs.GetBufferSize() },
            },
            PS: D3D12_SHADER_BYTECODE {
                pShaderBytecode: unsafe { ps.GetBufferPointer() },
                BytecodeLength: unsafe { ps.GetBufferSize() },
            },
            RasterizerState: D3D12_RASTERIZER_DESC {
                FillMode: D3D12_FILL_MODE_SOLID,
                CullMode: D3D12_CULL_MODE_NONE,
                ..Default::default()
            },
            BlendState: D3D12_BLEND_DESC {
                AlphaToCoverageEnable: false.into(),
                IndependentBlendEnable: false.into(),
                RenderTarget: [
                    D3D12_RENDER_TARGET_BLEND_DESC {
                        BlendEnable: false.into(),
                        LogicOpEnable: false.into(),
                        SrcBlend: D3D12_BLEND_ONE,
                        DestBlend: D3D12_BLEND_ZERO,
                        BlendOp: D3D12_BLEND_OP_ADD,
                        SrcBlendAlpha: D3D12_BLEND_ONE,
                        DestBlendAlpha: D3D12_BLEND_ZERO,
                        BlendOpAlpha: D3D12_BLEND_OP_ADD,
                        LogicOp: D3D12_LOGIC_OP_NOOP,
                        RenderTargetWriteMask: D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8,
                    },
                    D3D12_RENDER_TARGET_BLEND_DESC::default(),
                    D3D12_RENDER_TARGET_BLEND_DESC::default(),
                    D3D12_RENDER_TARGET_BLEND_DESC::default(),
                    D3D12_RENDER_TARGET_BLEND_DESC::default(),
                    D3D12_RENDER_TARGET_BLEND_DESC::default(),
                    D3D12_RENDER_TARGET_BLEND_DESC::default(),
                    D3D12_RENDER_TARGET_BLEND_DESC::default(),
                ],
            },
            DepthStencilState: D3D12_DEPTH_STENCIL_DESC::default(),
            SampleMask: u32::max_value(),
            PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
            NumRenderTargets: 1,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                ..Default::default()
            },
            ..Default::default()
        };
        desc.RTVFormats[0] = DXGI_FORMAT_R8G8B8A8_UNORM;

        // initialise struct
        Pipeline {
            pso: unsafe { self.device.CreateGraphicsPipelineState(&desc).unwrap() },
            root_signature: root_signature,
        }
    }

    fn create_shader<T: Sized>(&self, info: super::ShaderInfo, data: &[T]) -> Shader {
        let mut shader_blob = None;
        if info.compile_info.is_some() {
            let compile_info = info.compile_info.unwrap();
            let compile_flags = to_d3d12_compile_flags(&compile_info.flags);
            unsafe {
                let mut errors = None;
                let result = D3DCompile(
                    data.as_ptr() as *const core::ffi::c_void,
                    data.len(),
                    PSTR(std::ptr::null_mut() as _),
                    std::ptr::null(),
                    None,
                    PSTR((compile_info.entry_point + "\0").as_ptr() as _),
                    PSTR((compile_info.target + "\0").as_ptr() as _),
                    compile_flags,
                    0,
                    &mut shader_blob,
                    &mut errors,
                );
                if !result.is_ok() {
                    if errors.is_some() {
                        let w = errors.unwrap();
                        let buf = w.GetBufferPointer();
                        let c_str: &CStr = CStr::from_ptr(buf as *const i8);
                        let str_slice: &str = c_str.to_str().unwrap();
                        println!("{}", str_slice);
                    }
                    panic!("shader compile failed!");
                }
            }
        }

        // initialise struct
        Shader {
            blob: shader_blob.unwrap(),
        }
    }

    // TODO: validate and return result
    fn create_buffer<T: Sized>(&self, info: super::BufferInfo, data: &[T]) -> Buffer {
        let mut buf: Option<ID3D12Resource> = None;
        unsafe {
            if !self
                .device
                .CreateCommittedResource(
                    &D3D12_HEAP_PROPERTIES {
                        Type: D3D12_HEAP_TYPE_UPLOAD,
                        ..Default::default()
                    },
                    D3D12_HEAP_FLAG_NONE,
                    &D3D12_RESOURCE_DESC {
                        Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                        Width: data.len() as u64,
                        Height: 1,
                        DepthOrArraySize: 1,
                        MipLevels: 1,
                        SampleDesc: DXGI_SAMPLE_DESC {
                            Count: 1,
                            Quality: 0,
                        },
                        Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                        ..Default::default()
                    },
                    D3D12_RESOURCE_STATE_GENERIC_READ,
                    std::ptr::null(),
                    &mut buf,
                )
                .is_ok()
            {
                // TODO: the error should be passed to the user
                panic!("hotline::gfx::d3d12: failed to create buffer!");
            };
        };
        let buf = buf.unwrap();

        // Copy the triangle data to the vertex buffer.
        unsafe {
            let mut map_data = std::ptr::null_mut();
            let src = data.as_ptr() as *const u8;
            buf.Map(0, std::ptr::null(), &mut map_data);
            std::ptr::copy_nonoverlapping(src, map_data as *mut u8, data.len());
            buf.Unmap(0, std::ptr::null());
        }

        let mut vbv: Option<D3D12_VERTEX_BUFFER_VIEW> = None;
        let mut ibv: Option<D3D12_INDEX_BUFFER_VIEW> = None;

        match info.usage {
            super::BufferUsage::Vertex => {
                vbv = Some(D3D12_VERTEX_BUFFER_VIEW {
                    BufferLocation: unsafe { buf.GetGPUVirtualAddress() },
                    StrideInBytes: info.stride as u32,
                    SizeInBytes: data.len() as u32,
                });
            }
            super::BufferUsage::Index => {
                ibv = Some(D3D12_INDEX_BUFFER_VIEW {
                    BufferLocation: unsafe { buf.GetGPUVirtualAddress() },
                    SizeInBytes: data.len() as u32,
                    Format: to_dxgi_format(info.format),
                })
            }
        }

        // initialise struct
        Buffer {
            resource: buf,
            vbv: vbv,
            ibv: ibv,
        }
    }

    // TODO: validate and return result
    fn create_texture<T: Sized>(&self, info: super::TextureInfo, data: &[T]) -> Texture {
        let mut tex: Option<ID3D12Resource> = None;
        unsafe {
            // create texture resource
            self.device
                .CreateCommittedResource(
                    &D3D12_HEAP_PROPERTIES {
                        Type: D3D12_HEAP_TYPE_DEFAULT,
                        ..Default::default()
                    },
                    D3D12_HEAP_FLAG_NONE,
                    &D3D12_RESOURCE_DESC {
                        Dimension: match info.tex_type {
                            super::TextureType::Texture1D => D3D12_RESOURCE_DIMENSION_TEXTURE1D,
                            super::TextureType::Texture2D => D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                            super::TextureType::Texture3D => D3D12_RESOURCE_DIMENSION_TEXTURE3D,
                        },
                        Alignment: 0,
                        Width: info.width as u64,
                        Height: info.height as u32,
                        DepthOrArraySize: info.depth as u16,
                        MipLevels: info.mip_levels as u16,
                        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                        SampleDesc: DXGI_SAMPLE_DESC {
                            Count: info.samples as u32,
                            Quality: 0,
                        },
                        Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
                        Flags: D3D12_RESOURCE_FLAG_NONE,
                    },
                    D3D12_RESOURCE_STATE_COPY_DEST,
                    std::ptr::null(),
                    &mut tex,
                )
                .expect("hotline::gfx::d3d12: failed to create texture!");

            // create upload buffer
            let block_size = 4; // TODO:
            let upload_pitch =
                super::align_pow2(info.width * 4, D3D12_TEXTURE_DATA_PITCH_ALIGNMENT as u64);
            let upload_size = info.height * upload_pitch;

            let mut upload: Option<ID3D12Resource> = None;
            self.device
                .CreateCommittedResource(
                    &D3D12_HEAP_PROPERTIES {
                        Type: D3D12_HEAP_TYPE_UPLOAD,
                        ..Default::default()
                    },
                    D3D12_HEAP_FLAG_NONE,
                    &D3D12_RESOURCE_DESC {
                        Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
                        Alignment: 0,
                        Width: upload_size,
                        Height: 1,
                        DepthOrArraySize: 1,
                        MipLevels: 1,
                        Format: DXGI_FORMAT_UNKNOWN,
                        SampleDesc: DXGI_SAMPLE_DESC {
                            Count: 1,
                            Quality: 0,
                        },
                        Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
                        Flags: D3D12_RESOURCE_FLAG_NONE,
                    },
                    D3D12_RESOURCE_STATE_GENERIC_READ,
                    std::ptr::null(),
                    &mut upload,
                )
                .expect("hotline::gfx::d3d12: failed to create texture upload buffer!");

            // copy data to upload buffer
            let range = D3D12_RANGE {
                Begin: 0,
                End: upload_size as usize,
            };
            let mut map_data = std::ptr::null_mut();
            let res = upload.clone().unwrap();
            res.Map(0, &range, &mut map_data);
            if map_data != std::ptr::null_mut() {
                for y in 0..info.height {
                    let src = data.as_ptr().offset((y * info.width * 4) as isize) as *const u8;
                    let dst = (map_data as *mut u8).offset((y * upload_pitch) as isize);
                    std::ptr::copy_nonoverlapping(src, dst, (info.width * 4) as usize);
                }
            }
            res.Unmap(0, std::ptr::null());

            // copy resource
            let fence: ID3D12Fence = self.device.CreateFence(0, D3D12_FENCE_FLAG_NONE).unwrap();

            let src = D3D12_TEXTURE_COPY_LOCATION {
                pResource: Some(upload.clone().unwrap()),
                Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    PlacedFootprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                        Offset: 0,
                        Footprint: D3D12_SUBRESOURCE_FOOTPRINT {
                            Width: info.width as u32,
                            Height: info.height as u32,
                            Depth: 1,
                            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                            RowPitch: upload_pitch as u32,
                        },
                    },
                },
            };

            let dst = D3D12_TEXTURE_COPY_LOCATION {
                pResource: Some(tex.clone().unwrap()),
                Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    SubresourceIndex: 0,
                },
            };

            self.command_list.CopyTextureRegion(&dst, 0, 0, 0, &src, std::ptr::null_mut());

            let barrier = transition_barrier(
                &tex.clone().unwrap(),
                D3D12_RESOURCE_STATE_COPY_DEST,
                D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
            );

            // transition to shader resource
            self.command_list.ResourceBarrier(1, &barrier);
            let _: D3D12_RESOURCE_TRANSITION_BARRIER =
                std::mem::ManuallyDrop::into_inner(barrier.Anonymous.Transition);

            self.command_list.Close();

            let cmd = ID3D12CommandList::from(&self.command_list);
            self.command_queue.ExecuteCommandLists(1, &mut Some(cmd));
            self.command_queue.Signal(&fence, 1);

            let event = CreateEventA(std::ptr::null_mut(), false, false, None);
            fence.SetEventOnCompletion(1, event);
            WaitForSingleObject(event, INFINITE);

            // create an srv for the texture
            let ptr = self.shader_heap.GetCPUDescriptorHandleForHeapStart().ptr;
            let handle = D3D12_CPU_DESCRIPTOR_HANDLE { ptr: ptr };

            self.device.CreateShaderResourceView(
                &tex,
                &D3D12_SHADER_RESOURCE_VIEW_DESC {
                    Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    ViewDimension: match info.tex_type {
                        super::TextureType::Texture1D => D3D12_SRV_DIMENSION_TEXTURE1D,
                        super::TextureType::Texture2D => D3D12_SRV_DIMENSION_TEXTURE2D,
                        super::TextureType::Texture3D => D3D12_SRV_DIMENSION_TEXTURE3D,
                    },
                    Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                        Texture2D: D3D12_TEX2D_SRV {
                            MipLevels: info.mip_levels,
                            MostDetailedMip: 0,
                            ..Default::default()
                        },
                    },
                    Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
                },
                &handle,
            );

            // initialise struct
            Texture {
                resource: tex.unwrap(),
                srv: handle,
                gpu: self.shader_heap.GetGPUDescriptorHandleForHeapStart(),
            }
        }
    }

    fn execute(&self, cmd: &CmdBuf) {
        unsafe {
            let command_list = ID3D12CommandList::from(&cmd.command_list[cmd.bb_index]);
            self.command_queue.ExecuteCommandLists(1, &mut Some(command_list));
        }
    }
}

impl SwapChain {
    fn wait_for_frame(&mut self, frame_index: usize) {
        unsafe {
            let mut waitable: [HANDLE; 2] =
                [self.swap_chain.GetFrameLatencyWaitableObject(), HANDLE(0)];
            let mut num_waitable = 1;
            let mut fv = self.frame_fence_value[frame_index as usize];

            if fv != 0
            // means no fence was signaled
            {
                fv = 0;
                if !self.fence.SetEventOnCompletion(fv as u64, self.fence_event).is_ok() {
                    panic!("hotline::gfx::d3d12: failed to set on completion event!");
                }
                waitable[1] = self.fence_event;
                num_waitable = 2;
            }

            WaitForMultipleObjects(num_waitable, waitable.as_ptr(), true, INFINITE);
        }
    }
}

impl super::SwapChain<Device> for SwapChain {
    fn new_frame(&mut self) {
        self.wait_for_frame(self.bb_index as usize);
    }

    fn update(&mut self, device: &Device, window: &platform::Window, cmd: &mut CmdBuf) {
        let wh = window.get_size();
        if wh.0 != self.width || wh.1 != self.height {
            unsafe {
                self.wait_for_frame(self.bb_index as usize);
                cmd.reset_internal();

                let res = self.swap_chain.ResizeBuffers(
                    NUM_BB,
                    wh.0 as u32,
                    wh.1 as u32,
                    DXGI_FORMAT_UNKNOWN,
                    self.flags,
                );

                if !res.is_ok() {
                    let err = res.err();
                    if err.is_some() {
                        let eee = err.unwrap();
                        println!("swap chain resize failed {}", eee);
                    }
                } else {
                    println!("resize success!");
                }

                let rtv = create_swap_chain_rtv(&self.swap_chain, &device.device);
                let data_size = (self.width * self.height * 4) as u64;
                self.readback_buffer = create_read_back_buffer(&device, data_size);

                self.rtv_heap = rtv.0;
                self.rtv_handles = rtv.1;
                self.width = wh.0;
                self.height = wh.1;
            }
        } else {
            self.new_frame();
        }
    }

    fn get_backbuffer_index(&self) -> i32 {
        self.bb_index
    }

    fn swap(&mut self, device: &Device) {
        unsafe {
            // present
            if !self.swap_chain.Present(1, 0).is_ok() {
                println!("hotline::gfx::d3d12: warning: present failed!");
            }

            // signal fence
            let fv = self.fence_last_signalled_value + 1;
            if !device.command_queue.Signal(&self.fence, fv as u64).is_ok() {
                println!("hotline::gfx::d3d12: warning: command_queue.Signal failed!");
            }

            // update fence tracking
            self.fence_last_signalled_value = fv;
            self.frame_fence_value[self.bb_index as usize] = fv;

            // swap buffers
            let next_frame_index = self.frame_index + 1;
            self.frame_index = next_frame_index;
            self.bb_index = next_frame_index % NUM_BB as i32;
        }
    }
}

impl CmdBuf {
    fn cmd(&self) -> &ID3D12GraphicsCommandList {
        &self.command_list[self.bb_index]
    }

    fn drop_complete_in_flight_barriers(&mut self, bb: usize) {
        let size = self.in_flight_barriers[bb].len();
        for i in (0..size).rev() {
            let barrier = self.in_flight_barriers[bb].remove(i);
            unsafe {
                let _: D3D12_RESOURCE_TRANSITION_BARRIER =
                    std::mem::ManuallyDrop::into_inner(barrier.Anonymous.Transition);
            }
        }
        self.in_flight_barriers[bb].clear();
    }

    // TODO: how to call super traits?
    fn reset_internal(&mut self) {
        self.drop_complete_in_flight_barriers(self.bb_index);
    }
}

impl super::CmdBuf<Device> for CmdBuf {
    fn reset(&mut self, swap_chain: &SwapChain) {
        let prev_bb = self.bb_index;
        let bb = unsafe { swap_chain.swap_chain.GetCurrentBackBufferIndex() as usize };
        self.bb_index = bb;
        if swap_chain.frame_fence_value[bb] != 0 {
            unsafe {
                if !self.command_allocator[bb].Reset().is_ok() {
                    panic!("hotline::gfx::d3d12: failed to reset command_allocator")
                }
                if !self.command_list[bb].Reset(&self.command_allocator[bb], None).is_ok() {
                    panic!("hotline::gfx::d3d12: to reset command_list")
                };
            }
        }
        self.drop_complete_in_flight_barriers(prev_bb);
    }

    fn clear_debug(&mut self, queue: &SwapChain, r: f32, g: f32, b: f32, a: f32) {
        let bb = unsafe { queue.swap_chain.GetCurrentBackBufferIndex() as usize };

        // Indicate that the back buffer will be used as a render target.
        unsafe {
            let barrier = transition_barrier(
                &queue.swap_chain.GetBuffer(bb as u32).unwrap(),
                D3D12_RESOURCE_STATE_PRESENT,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
            );

            self.command_list[bb].ResourceBarrier(1, &barrier);
            self.in_flight_barriers[bb].push(barrier);

            self.command_list[bb].ClearRenderTargetView(
                queue.rtv_handles[bb],
                [r, g, b, a].as_ptr(),
                0,
                std::ptr::null(),
            );
            self.cmd().OMSetRenderTargets(1, &queue.rtv_handles[bb], false, std::ptr::null());
        }
    }

    fn debug_set_descriptor_heap(&self, device: &Device, tex: &Texture) {
        unsafe {
            self.cmd().SetDescriptorHeaps(1, &Some(device.shader_heap.clone()));
            self.cmd().SetGraphicsRootDescriptorTable(1, &tex.gpu);
        }
    }

    fn set_viewport(&self, viewport: &super::Viewport) {
        let d3d12_vp = D3D12_VIEWPORT {
            TopLeftX: viewport.x,
            TopLeftY: viewport.y,
            Width: viewport.width,
            Height: viewport.height,
            MinDepth: viewport.min_depth,
            MaxDepth: viewport.max_depth,
        };
        unsafe {
            self.cmd().RSSetViewports(1, &d3d12_vp);
        }
    }

    fn set_scissor_rect(&self, scissor_rect: &super::ScissorRect) {
        let d3d12_sr = RECT {
            left: scissor_rect.left,
            top: scissor_rect.top,
            right: scissor_rect.right,
            bottom: scissor_rect.bottom,
        };
        unsafe {
            self.cmd().RSSetScissorRects(1, &d3d12_sr);
        }
    }

    fn set_vertex_buffer(&self, buffer: &Buffer, slot: u32) {
        let cmd = self.cmd();
        if buffer.vbv.is_some() {
            unsafe {
                cmd.IASetVertexBuffers(slot, 1, &buffer.vbv.unwrap());
            }
        }
    }

    fn set_index_buffer(&self, buffer: &Buffer) {
        let cmd = self.cmd();
        if buffer.ibv.is_some() {
            unsafe {
                cmd.IASetIndexBuffer(&buffer.ibv.unwrap());
            }
        }
    }

    fn set_pipeline_state(&self, pipeline: &Pipeline) {
        let cmd = self.cmd();
        unsafe {
            cmd.SetGraphicsRootSignature(&pipeline.root_signature);
            cmd.SetPipelineState(&pipeline.pso);
            cmd.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        }
    }

    fn push_constants<T: Sized>(&self, slot: u32, num_values: u32, dest_offset: u32, data: &[T]) {
        let cmd = self.cmd();
        unsafe {
            cmd.SetGraphicsRoot32BitConstants(
                slot,
                num_values,
                data.as_ptr() as *const ::core::ffi::c_void,
                dest_offset,
            )
        }
    }

    fn draw_instanced(
        &self,
        vertex_count: u32,
        instance_count: u32,
        start_vertex: u32,
        start_instance: u32,
    ) {
        unsafe {
            self.cmd().DrawInstanced(vertex_count, instance_count, start_vertex, start_instance);
        }
    }

    fn draw_indexed_instanced(
        &self,
        index_count: u32,
        instance_count: u32,
        start_index: u32,
        base_vertex: i32,
        start_instance: u32,
    ) {
        unsafe {
            self.cmd().DrawIndexedInstanced(
                index_count,
                instance_count,
                start_index,
                base_vertex,
                start_instance,
            );
        }
    }

    fn close(&mut self, swap_chain: &SwapChain) {
        let bb = unsafe { swap_chain.swap_chain.GetCurrentBackBufferIndex() as usize };
        // Indicate that the back buffer will now be used to present.
        unsafe {
            let barrier = transition_barrier(
                &swap_chain.swap_chain.GetBuffer(bb as u32).unwrap(),
                D3D12_RESOURCE_STATE_RENDER_TARGET,
                D3D12_RESOURCE_STATE_PRESENT,
            );
            self.command_list[bb].ResourceBarrier(1, &barrier);
            self.in_flight_barriers[bb].push(barrier);

            if !self.command_list[bb].Close().is_ok() {
                panic!("hotline: d3d12 failed to close command list.")
            }
        }
    }

    fn read_back_backbuffer(&mut self, swap_chain: &SwapChain) -> ReadBackRequest {
        let bb = self.bb_index;
        let bbz = self.bb_index as u32;
        unsafe {
            let resource = swap_chain.swap_chain.GetBuffer(bbz);
            let r2 = resource.as_ref();

            // transition to copy source
            let barrier = transition_barrier(
                &r2.unwrap(),
                D3D12_RESOURCE_STATE_RENDER_TARGET,
                D3D12_RESOURCE_STATE_COPY_SOURCE,
            );
            self.command_list[bb].ResourceBarrier(1, &barrier);
            self.in_flight_barriers[bb].push(barrier);

            let src = D3D12_TEXTURE_COPY_LOCATION {
                pResource: Some(resource.clone().unwrap()),
                Type: D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    SubresourceIndex: 0,
                },
            };

            let dst = D3D12_TEXTURE_COPY_LOCATION {
                pResource: Some(swap_chain.readback_buffer.clone().unwrap()),
                Type: D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT,
                Anonymous: D3D12_TEXTURE_COPY_LOCATION_0 {
                    PlacedFootprint: D3D12_PLACED_SUBRESOURCE_FOOTPRINT {
                        Offset: 0,
                        Footprint: D3D12_SUBRESOURCE_FOOTPRINT {
                            Width: swap_chain.width as u32,
                            Height: swap_chain.height as u32,
                            Depth: 1,
                            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                            RowPitch: (swap_chain.width * 4) as u32,
                        },
                    },
                },
            };

            self.command_list[bb].CopyTextureRegion(&dst, 0, 0, 0, &src, std::ptr::null_mut());

            let barrier = transition_barrier(
                &r2.unwrap(),
                D3D12_RESOURCE_STATE_COPY_SOURCE,
                D3D12_RESOURCE_STATE_RENDER_TARGET,
            );

            // transition back to render target
            self.command_list[bb].ResourceBarrier(1, &barrier);
            self.in_flight_barriers[bb].push(barrier);

            ReadBackRequest {
                resource: Some(swap_chain.readback_buffer.clone().unwrap()),
                fence_value: swap_chain.frame_index as u64,
                size: (swap_chain.width * swap_chain.height * 4) as usize,
                row_pitch: (swap_chain.width * 4) as usize,
                slice_pitch: (swap_chain.width * swap_chain.height * 4) as usize,
            }
        }
    }
}

impl super::ReadBackRequest<Device> for ReadBackRequest {
    fn is_complete(&self, swap_chain: &SwapChain) -> bool {
        if swap_chain.frame_index as u64 > self.fence_value + 1 {
            return true;
        }
        false
    }

    fn get_data(&self) -> std::result::Result<super::ReadBackData, &str> {
        let range = D3D12_RANGE {
            Begin: 0,
            End: self.size,
        };
        let mut map_data = std::ptr::null_mut();
        unsafe {
            let res = self.resource.as_ref().unwrap();
            res.Map(0, &range, &mut map_data).map_err(|_| "hotline::gfx::d3d12: map failed!")?;
            if map_data != std::ptr::null_mut() {
                let slice = std::slice::from_raw_parts(map_data as *const u8, self.size);
                let rb_data = super::ReadBackData {
                    data: slice,
                    size: self.size,
                    format: super::Format::Unknown,
                    row_pitch: self.row_pitch,
                    slice_pitch: self.size,
                };
                return Ok(rb_data);
            } else {
                return Err("hotline::gfx::d3d12: map failed!");
            }
            // TODO: ownership
            //res.Unmap(0, std::ptr::null());
        }
    }
}
