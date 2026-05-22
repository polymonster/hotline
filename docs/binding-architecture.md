# Bindless binding architecture: D3D12 ↔ Metal

Reference notes on how hotline maps a single bindless shader-resource model onto two very
different GPU binding APIs — D3D12 descriptor heaps and Metal argument buffers — and the two
approaches we went through to make Metal behave like D3D12. Written as background for a blog post.

---

## 1. The goal

Author shaders **once** in HLSL using bindless resource arrays, and run them on both D3D12 and
Metal with identical CPU-side code. A shader looks like this (`shaders/ecs.hlsl`):

```hlsl
// all bindless arrays share register t1, separated by space
Texture2D       textures[]        : register(t1, space7);
TextureCube     cubemaps[]        : register(t1, space9);
Texture2DArray  texture_arrays[]  : register(t1, space10);
Texture3D       volume_textures[] : register(t1, space11);

StructuredBuffer<DrawData>     draws     : register(t0, space0);
StructuredBuffer<MaterialData> materials : register(t0, space2);

SamplerState sampler_wrap_linear : register(s1);
```

The CPU stores **global indices** into these arrays (e.g. a material stores the heap slot of its
albedo texture and the scene's IBL cubemap), and the shader does `textures[material.albedo_id]`
or `cubemaps[ibl_id]`. There is one global resource pool; the index is the only thing that crosses
the CPU/GPU boundary.

---

## 2. Two GPU binding models

### D3D12 — descriptor heaps
A `ID3D12DescriptorHeap` is a flat array of descriptors. The shader-visible CBV/SRV/UAV heap is
bound once; shaders index it directly (`ResourceDescriptorHeap[i]` in SM6.6, or unbounded
descriptor-table ranges pre-6.6). The **heap slot is the global index** — exactly the model above,
natively. A root signature maps each HLSL `register`/`space` to a descriptor-table range or root
constants.

### Metal — argument buffers
Metal has no shader-visible descriptor heap that you index with an arbitrary integer (pre-Metal 3
bindless). Instead you build an **argument buffer**: a GPU buffer whose layout is described by an
`MTLArgumentEncoder`, holding texture/buffer/sampler handles at fixed `[[id(N)]]` slots. A shader
receives argument buffers as `[[buffer(N)]]` parameters. An `MTLHeap` backs the actual resources so
they're resident; the argument buffer holds references into it.

So the porting problem is: **make a flat, integer-indexed global pool work on top of Metal argument
buffers**, where each shader only sees the argument buffers (descriptor sets) it actually uses.

---

## 3. Hotline's abstraction

- `gfx::PipelineLayout` carries `bindings: Vec<DescriptorBinding>`, `push_constants`, and
  `static_samplers`. Each `DescriptorBinding` records `shader_register`, `register_space`,
  `binding_type` (SRV/UAV/CBV/Sampler) and `num_descriptors` (None = unbounded/bindless).
- `gfx::PipelineSlotInfo { index, count }` is the resolved location for a binding key
  `(register, space, descriptor_type)`.
- A single bindless `Heap` holds all textures and all buffers.

The two backends implement `gfx::Device::create_render_pipeline` / `set_heap` / `set_binding`
differently from here.

---

## 4. D3D12 mapping (the reference)

`src/gfx/d3d12.rs` builds a root signature from the pipeline layout. Each binding/space becomes a
descriptor-table range or root parameter; the resolved `PipelineSlotInfo.index` is the **heap
index / root slot**, used directly. There is no per-binding offset fix-up — the global index *is*
the descriptor index. (This is why `PipelineSlotInfo` carries no offset field, and why D3D12 needed
no special handling.)

---

## 5. Metal mapping via SPIRV-Cross (htwv)

Shaders are cross-compiled offline: **HLSL → DXC → SPIR-V → SPIRV-Cross → MSL** in the `htwv` crate
(`hotline-data/htwv/src/macos_impl.rs`). DXC encodes HLSL `register(tN, spaceM)` as SPIR-V
decorations `Binding = N`, `DescriptorSet = M`. htwv then **re-decorates** each resource to assign
the Metal descriptor set (`[[buffer(N)]]`) and the slot within it (`[[id(N)]]`), and feeds matching
`spvc_msl_resource_binding` entries to SPIRV-Cross.

The runtime (`src/gfx/mtl.rs`) must assign the **same** `[[buffer(N)]]` numbering so that
`set_heap` / `set_binding` bind the heap's argument buffers to the slots the MSL expects. This
mirroring is the crux: `build_slot_lookup`, `build_stage_binders` and `build_compute_binder` in
`mtl.rs` reproduce, byte-for-byte, the grouping htwv used in codegen.

The `Heap` keeps **two argument buffers** — one array of all textures, one of all buffer pointers
(`get_texture_argument_buffer` / `get_buffer_argument_buffer`) — each a flat array based at id 0,
indexed by global slot. Samplers live in their own argument buffer at fragment `buffer(0)`. Push
constants are *discrete* descriptor sets (no argument buffer) so they can use
`setVertexBytes`/`setFragmentBytes`.

The open question that produced two approaches: **how do you group HLSL bindings into Metal
descriptor sets?**

---

## 6. Approach 1 — group by `(kind, register)`, compensate with `sub_offset`

Group bindings by register *kind and number only* (`t0`, `t1`, `u0`, `b0`…). All arrays sharing a
register — regardless of space — landed in **one** descriptor set, packed at consecutive ids:

```
descriptor set for t1:
  textures        -> [[id(0)]]
  cubemaps        -> [[id(1)]]
  texture_arrays  -> [[id(2)]]
  volume_textures -> [[id(3)]]
```

**Why this was attractive:** SPIRV-Cross hard-limits argument buffers to
`kMaxArgumentBuffers = 8` (`spirv_msl.hpp`, throws *"Descriptor set index is out of range."* past
it). Packing many spaces into one register's set conserves that scarce budget — you could have up
to 8 *registers*, each holding many spaces.

**The compensation:** with SPIRV-Cross's "unsized array hack", `array[i]` in a packed set lowers to
`arg_buffer[id + i]`. Our heap argument buffer is a flat array based at id 0, so `cubemaps[i]`
(at `id(1)`) actually reads `heap[1 + i]`. To cancel the `+1`, the CPU subtracted the binding's
`sub_offset` from the index it wrote: `index = global_index - sub_offset` (the old
`get_lookup` → `get_sub_binding_offset` path).

**The fatal flaw:** that compensation only ran for **structured buffers routed through
`get_lookup`**. Texture indices live in *shared material/draw data* and are written as raw global
indices — materials are pipeline-agnostic, so you can't bake a pipeline-specific `sub_offset` into
them. Result: the **second and later texture array in a packed set was off by one**. `cubemaps`
(at `id(1)`) read `heap[ibl_id + 1]` and sampled a neighbouring, unrelated texture as a cube — the
classic "cube samples a flat orange, 2D samples a flat sky-blue" symptom. The first array
(`textures` at `id(0)`) worked by luck because its offset was 0.

So Approach 1 only ever worked for shaders using a single bindless texture array.

---

## 7. Approach 2 — group by `(kind, register, space)`, one set per binding (current)

Add **space** to the grouping key. Each `(kind, register, space)` tuple becomes its own descriptor
set, so every bindless array is alone in its set at `[[id(0)]]`:

```
buffer(3): textures  -> [[id(0)]]
buffer(4): cubemaps  -> [[id(0)]]
```

Now `textures[i]` and `cubemaps[i]` both lower to `arg_buffer[i]` — no offset, no compensation.
The CPU writes raw global indices and they just work. `sub_offset`, `get_sub_binding_offset`, and
the `get_lookup` subtraction were all removed; with one binding per set they are permanently 0.

**The cost we accepted:** capacity drops from "8 registers × many spaces" to roughly **8 total
binding groups per stage**, because each binding now consumes a whole descriptor set against
`kMaxArgumentBuffers = 8`. The heaviest current shader (`vs_mesh_material_indirect`) sits at
`buffer(7)` — the last legal slot. To make the ceiling visible instead of cryptic, htwv now emits a
`cargo:warning` when a stage reaches/exceeds the limit (`MAX_DESCRIPTOR_SETS`, kept in sync with
`kMaxArgumentBuffers`; bump it if a future SPIRV-Cross raises the cap).

The two sides stay in lockstep by keying on `(kind, register, space)` in **both**
`hotline-data/htwv/src/macos_impl.rs` (codegen) and the three `mtl.rs` builders (runtime), iterating
`pipeline_layout.bindings` in the same order.

---

## 8. Side-by-side

| Concept                    | D3D12                                  | Metal (Approach 2)                                  |
|----------------------------|----------------------------------------|-----------------------------------------------------|
| Global pool                | Shader-visible descriptor heap         | Two `MTLHeap`-backed argument buffers (tex / buf)   |
| Index semantics            | Heap slot = global index (direct)      | `arg_buffer[i]`, base id 0 = global index (direct)  |
| HLSL `register`/`space`    | Root-sig table range / root param      | One MSL descriptor set per `(kind, register, space)`|
| Per-binding offset fix-up  | None (`PipelineSlotInfo` has no offset)| None (each binding alone at `[[id(0)]]`)            |
| Samplers                   | Static samplers in root sig            | Sampler argument buffer at fragment `buffer(0)`     |
| Push constants             | Root constants                         | Discrete set via `setVertex/FragmentBytes`          |
| Residency                  | Implicit (heap is resident when set)   | `use_heap` (textures) + `use_resource` (buffers)    |
| Hard limit                 | 1M-entry heaps (effectively unbounded) | `kMaxArgumentBuffers = 8` descriptor sets           |

The design intent: **make Metal's indexing match D3D12's "the index is the index" semantics**, so
shared CPU data (material/draw indices) is correct on both backends with no per-backend fix-up.
Approach 1 broke that for textures; Approach 2 restores it, trading capacity for correctness.

---

## 9. Residency: `use_heap` vs `use_resource`

Getting the index right only solves *where* the shader looks. On Metal there's a second, separate
problem: the resource the argument buffer points at must be made **GPU-resident**, or the read
returns garbage. Pointing an argument buffer at a resource does *not* make it resident — that's
explicit, and it differs by how the resource was allocated.

In hotline the two resource classes are allocated differently:

- **Textures** are allocated *from* the bindless `MTLHeap` (`mtl_heap.new_texture`). One
  `encoder.use_heap(&mtl_heap)` in `set_heap` makes the whole heap resident, covering every texture
  the bindless argument buffer might index.
- **Structured buffers** (draw/material/light/etc.) are allocated *from the device*
  (`device.new_buffer`), **not** from that heap. Two reasons they can't just live in the texture
  heap:
  1. **Storage mode.** The texture heap is `Private` (GPU-only, blit-uploaded). The world buffers
     are `Shared` + persistently-mapped and rewritten by the CPU every frame. A `Private` heap
     can't host CPU-writable buffers, so they'd need a *separate* `Shared` heap.
  2. **Sizing.** `MTLHeap` is fixed-size and can't grow; buffer capacities are dynamic
     (`reserve_world_buffers` at runtime, e.g. a 786 KB draw buffer vs a 160 B material buffer).
     Sub-allocating dynamically-sized buffers from a fixed heap means over-allocating wildly or
     recreating + re-encoding the heap on growth.

So the buffers are device-allocated and only *referenced* from the heap's buffer argument buffer.
`use_heap` does nothing for them. They must be made resident explicitly with
`encoder.use_resource(buffer, Read|Write)` (mirroring `use_resource_at(..., Vertex|Fragment)` on the
render encoder) for **every** buffer in the pool — because bindless indices are resolved at runtime
in the shader, any buffer could be the one indexed.

This produced a memorable bug. Without the `use_resource` calls, residency was left to Metal's
implicit heuristics: small/early device allocations happened to stay resident, large ones did not.
Symptoms:

- One demo (small draw buffer) worked; another with the *same shader* but a large (786 KB) draw
  buffer drew nothing — a size-dependent failure, easily mistaken for a CPU-side data difference.
- Hard-coding the world matrix to identity on the CPU **changed nothing** — the decisive clue. If
  the data were the problem, identity would draw at the origin. It didn't, because the GPU was
  never reading that buffer's memory: it wasn't resident.

The fix is a few lines in `set_heap`: alongside each `use_heap`/`use_heap_at`, iterate the heap's
`buffer_slots` and `use_resource` each one. D3D12 has no analogue — a shader-visible descriptor heap
is resident for the duration it's set, so residency never surfaces as a separate step.

---

## 10. Lessons / future

- A compensation that only covers *some* index paths (buffers, not textures) is worse than none —
  it hides the bug for the simple case and surfaces it only with a second array.
- The 8-set limit is now the real constraint. Options if a pipeline needs more: raise
  `kMaxArgumentBuffers` in a newer SPIRV-Cross; or selectively re-pack **only buffer bindings**
  (which *can* be compensated correctly through `get_lookup`, since they don't share CPU data) while
  keeping textures one-set-per-binding.
- Keeping codegen (htwv) and runtime (mtl.rs) grouping identical is essential and fragile — any
  change to the grouping key must be made in all four places at once.
- Indexing correctness and residency are *separate* problems on Metal. A correct bindless index
  still reads garbage if the resource isn't resident — and because failure is residency-heuristic
  driven, it looks data-dependent (works small, fails large), which sends you debugging the wrong
  layer. "Hard-coding the data changes nothing" is the tell that it's residency, not data.

### Key files
- `hotline-data/htwv/src/macos_impl.rs` — HLSL→MSL, descriptor-set assignment, limit warning
- `src/gfx/mtl.rs` — `build_slot_lookup`, `build_stage_binders`, `build_compute_binder`, `set_heap`
- `src/gfx/d3d12.rs` — reference root-signature mapping
- `src/pmfx.rs` — `get_lookup` / world-buffer indices
- `shaders/ecs.hlsl` — the bindless array declarations
