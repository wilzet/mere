struct RenderView {
    view_pos: vec4<f32>,
    view_proj: mat4x4<f32>,
    // 6 planes: Left, Right, Top, Bottom, Near, Far
    planes: array<vec4<f32>, 6>,
}

// --- Vertex shader ---

@group(0) @binding(0) var<storage, read> main_camera: RenderView;

struct Vertex {
    position: array<f32, 3>,
}

struct BoundingSphere {
    center_x: f32,
    center_y: f32,
    center_z: f32,
    radius: f32,
}

struct Meshlet {
    vertex_offset: u32,
    vertex_count: u32,
    index_offset: u32,
    index_count: u32,
    bounds: BoundingSphere,
    parent_bounds: BoundingSphere,
}

struct MeshUniform {
    model_matrix: mat4x4<f32>,
    previous_model: mat4x4<f32>,
    normal_matrix_0: vec4<f32>,
    normal_matrix_1: vec4<f32>,
    normal_matrix_2: vec4<f32>,
}

struct ClusterInfo {
    instance_id: u32,
    meshlet_id: u32,
}

@group(1) @binding(0) var<storage, read> vertices: array<Vertex>;
@group(1) @binding(1) var<storage, read> meshlet_vertex_indices: array<u32>;
@group(1) @binding(2) var<storage, read> meshlet_indices: array<u32>;

@group(1) @binding(3) var<storage, read> meshlets: array<Meshlet>;
@group(1) @binding(4) var<storage, read> instances: array<MeshUniform>;
@group(1) @binding(5) var<storage, read> cluster_info: array<ClusterInfo>;

@group(1) @binding(6) var visibility_buffer: texture_storage_2d<r64uint, atomic>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) @interpolate(flat) packed_id: u32,
};

@vertex
fn vs_main(
    @builtin(vertex_index) index_id: u32,
    @builtin(instance_index) cluster_id: u32,
) -> VertexOutput {
    let info = cluster_info[cluster_id];
    let instance_id = info.instance_id;
    let meshlet_id = info.meshlet_id;

    let instance = instances[instance_id];
    let meshlet = meshlets[meshlet_id];

    if index_id >= meshlet.index_count { return dummy_vertex(); }

    let byte_offset = meshlet.index_offset + index_id;
    let word_offset = byte_offset / 4u;
    let bit_offset = (byte_offset % 4u) * 8u;

    // Fetch the 32-bit word and shift/mask to get the u8
    let packed_indices = meshlet_indices[word_offset];
    let local_index = (packed_indices >> bit_offset) & 0xFFu;

    let global_index = meshlet_vertex_indices[meshlet.vertex_offset + local_index];
    let v = vertices[global_index];

    var out: VertexOutput;
    let position = vec4(v.position[0], v.position[1], v.position[2], 1.0);
    let world_position = instance.model_matrix * position;

    out.clip_position = main_camera.view_proj * world_position;
    out.packed_id = (cluster_id << 7) | (index_id / 3);

    return out;
}

fn dummy_vertex() -> VertexOutput {
    return VertexOutput(
        vec4(divide(0.0, 0.0)),
        0,
    );
}

fn divide(a: f32, b: f32) -> f32 {
    return a / b;
}

// --- Fragment Shader ---

@fragment
fn fs_main(in: VertexOutput) {
    let depth = bitcast<u32>(in.clip_position.z);
    let visibility = (u64(depth) << 32) | u64(in.packed_id);

    textureAtomicMax(visibility_buffer, vec2<u32>(in.clip_position.xy), visibility);
}
