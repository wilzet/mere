struct RenderView {
    view_pos: vec4<f32>,
    view_proj: mat4x4<f32>,
    // 6 planes: Left, Right, Top, Bottom, Near, Far
    planes: array<vec4<f32>, 6>,
}

struct MeshUniform {
    model_matrix: mat4x4<f32>,
    previous_model: mat4x4<f32>,
    normal_matrix_0: vec4<f32>,
    normal_matrix_1: vec4<f32>,
    normal_matrix_2: vec4<f32>,
}

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

@group(0) @binding(0) var<storage, read> instances: array<MeshUniform>;
@group(0) @binding(1) var<storage, read> meshlets: array<Meshlet>;

@group(0) @binding(2) var<storage, read> cluster_info: array<ClusterInfo>;
@group(0) @binding(3) var<storage, read> visible_instance_cluster_count: u32;

@group(0) @binding(4) var<storage, read_write> visible_cluster_info: array<ClusterInfo>;
@group(0) @binding(5) var<storage, read_write> indirect_args: DrawIndirectArgs;

@group(1) @binding(0) var<storage, read> render_view: RenderView;

fn should_cull(instance_id: u32, meshlet: Meshlet) -> bool {
    let bounds = meshlet.bounds;
    let m = instances[instance_id].model_matrix;

    let center = (m * vec4(bounds.center_radius.xyz, 1.0)).xyz;

    let scale = max(
        length(m[0].xyz),
        max(length(m[1].xyz), length(m[2].xyz))
    );

    let radius = bounds.center_radius.w * scale;

    for (var i = 0u; i < 6u; i = i + 1u) {
        let plane = render_view.planes[i];
        let distance = dot(plane.xyz, center) + plane.w;

        if (distance < -radius) {
            return true;
        }
    }

    return false;
}

@compute @workgroup_size(128, 1, 1)
fn cull_clusters(
    @builtin(global_invocation_id) global_id: vec3<u32>
) {
    let cluster_id = global_id.x;

    if (cluster_id >= visible_instance_cluster_count) { return; }

    let info = cluster_info[cluster_id];
    let instance_id = info.instance_id;

    let meshlet_id = info.meshlet_id;

    let meshlet = meshlets[meshlet_id];

    if should_cull(instance_id, meshlet) { return; }

    let id  = atomicAdd(&indirect_args.instance_count, 1u);
    visible_cluster_info[id] = info;
}
