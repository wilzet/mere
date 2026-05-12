@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(vec2(vertex_index >> 1, vertex_index & 1)) * 2.0;
    return vec4(uv_to_ndc(uv), 0.0, 1.0);
}

fn uv_to_ndc(uv: vec2<f32>) -> vec2<f32> {
    return uv * vec2(2.0, -2.0) + vec2(-1.0, 1.0);
}

struct ClusterInfo {
    instance_id: u32,
    meshlet_id: u32,
}

@group(0) @binding(0) var<storage, read> cluster_info: array<ClusterInfo>;
@group(0) @binding(1) var<storage, read> instance_material_ids: array<u32>;
@group(0) @binding(2) var visibility_buffer: texture_storage_2d<r64uint, read>;

@fragment
fn fs_main(
    @builtin(position) position: vec4<f32>,
) -> @builtin(frag_depth) f32 {
    let visibility = textureLoad(visibility_buffer, vec2<u32>(position.xy)).r;

    let depth = visibility >> 32;
    if depth == 0lu { discard; }

    let cluster_id = u32(visibility) >> 7;
    let instance_id = cluster_info[cluster_id].instance_id;
    let material_id = instance_material_ids[instance_id];
    return f32(material_id) / 65535.0;
}