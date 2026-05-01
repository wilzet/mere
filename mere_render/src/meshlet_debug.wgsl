struct Camera {
    view_pos: vec4<f32>,
    view_proj: mat4x4<f32>,
}

struct Light {
    position: vec3<f32>,
    color: vec3<f32>,
}

struct MaterialProperties {
    color: vec4<f32>,
    normal_scale: f32,
    roughness: f32,
    metalness: f32,
}

// --- Vertex shader ---

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var<uniform> light: Light;

struct Vertex {
    position: array<f32, 3>,
    normal: u32,
    tex_coord: u32,
    tangent: u32,
}

struct Meshlet {
    vertex_offset: u32,
    vertex_count: u32,
    index_offset: u32,
    index_count: u32,
}

struct Instance {
    model_matrix: mat4x4<f32>,
    normal_matrix_0: vec3<f32>,
    normal_matrix_1: vec3<f32>,
    normal_matrix_2: vec3<f32>,
}

@group(3) @binding(0)
var<storage, read> vertices: array<Vertex>;

@group(3) @binding(1)
var<storage, read> meshlet_vertex_indices: array<u32>;

@group(3) @binding(2)
var<storage, read> meshlet_indices: array<u32>;

@group(3) @binding(3)
var<storage, read> meshlets: array<Meshlet>;

@group(3) @binding(4)
var<storage, read> instances: array<Instance>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
};

fn hash_color(id: u32) -> vec3<f32> {
    let x = f32(id);
    return fract(vec3<f32>(
        sin(x * 12.9898) * 43758.5453,
        sin(x * 78.233) * 43758.5453,
        sin(x * 39.425) * 43758.5453
    ));
}

@vertex
fn vs_main(
    @builtin(vertex_index) vid: u32,
    @builtin(instance_index) iid: u32,
) -> VertexOutput {
    let meshlet = meshlets[iid];
    let instance = instances[0];

    // guard against overflow (important!)
    if vid >= meshlet.index_count {
        // push off-screen
        var out: VertexOutput;
        out.clip_position = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        out.color = vec3<f32>(0.0);
        return out;
    }

    let byte_offset = meshlet.index_offset + vid;
    let word_offset = byte_offset / 4u;
    let bit_offset = (byte_offset % 4u) * 8u;

    // Fetch the 32-bit word and shift/mask to get the u8
    let packed_indices = meshlet_indices[word_offset];
    let local_index = (packed_indices >> bit_offset) & 0xFFu;
    // ---------------------------

    let global_index = meshlet_vertex_indices[meshlet.vertex_offset + local_index];
    let v = vertices[global_index];

    var out: VertexOutput;
    let position = vec4(v.position[0], v.position[1], v.position[2], 1.0);
    let world_position = instance.model_matrix * position;

    out.clip_position = camera.view_proj * world_position;

    // DEBUG: color per meshlet
    out.color = hash_color(iid);

    return out;
}

// --- Fragment Shader ---

@group(2) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(2) @binding(1)
var s_diffuse: sampler;
@group(2) @binding(2)
var t_normal: texture_2d<f32>;
@group(2) @binding(3)
var s_normal: sampler;
@group(2) @binding(4)
var t_rough_metal: texture_2d<f32>;
@group(2) @binding(5)
var s_rough_metal: sampler;
@group(2) @binding(6)
var<uniform> properties: MaterialProperties;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}
