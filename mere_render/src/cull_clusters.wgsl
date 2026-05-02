struct BoundingSphere {
    center_radius: vec4<f32>,
}

struct Meshlet {
    vertex_offset: u32,
    vertex_count: u32,
    index_offset: u32,
    index_count: u32,
    bounds: BoundingSphere,
    parent_bounds: BoundingSphere,
}

struct ClusterInfo {
    instance_id: u32,
    meshlet_id: u32,
}

struct DrawIndirectArgs {
    vertex_count: u32,
    instance_count: atomic<u32>,
    first_vertex: u32,
    first_instance: u32,
}

@group(0) @binding(0) var<storage, read> cluster_info: array<ClusterInfo>;
@group(0) @binding(1) var<storage, read> meshlets: array<Meshlet>;

@group(0) @binding(2) var<storage, read_write> indirect_args: DrawIndirectArgs;

fn should_cull(meshlet: Meshlet) -> bool {
    let bounds = meshlet.bounds;
    return false;
}

@compute @workgroup_size(128, 1, 1)
fn cull_clusters(
    @builtin(global_invocation_id) global_id: vec3<u32>
) {
    let cluster_id = global_id.x;

    if (cluster_id >= arrayLength(&cluster_info)) { return; }

    let info = cluster_info[cluster_id];
    let instance_id = info.instance_id;

    let meshlet_id = info.meshlet_id;

    let meshlet = meshlets[meshlet_id];

    if should_cull(meshlet) { return; }

    atomicAdd(&indirect_args.instance_count, 1u);
}
