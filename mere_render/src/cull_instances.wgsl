struct RenderView {
    view_pos: vec4<f32>,
    view_proj: mat4x4<f32>,
    // 6 planes: Left, Right, Top, Bottom, Near, Far
    planes: array<vec4<f32>, 6>,
}

struct Aabb {
    center_x: f32,
    center_y: f32,
    center_z: f32,
    extents_x: f32,
    extents_y: f32,
    extents_z: f32,
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

@group(0) @binding(0) var<storage, read> instances: array<MeshUniform>;
@group(0) @binding(1) var<storage, read> instance_aabbs: array<Aabb>;
@group(0) @binding(2) var<storage, read> instance_meshlet_offsets: array<u32>;
@group(0) @binding(3) var<storage, read> instance_meshlet_counts: array<u32>;

@group(0) @binding(4) var<storage, read_write> cluster_info: array<ClusterInfo>;
@group(0) @binding(5) var<storage, read_write> visible_instance_cluster_count: atomic<u32>;

@group(1) @binding(0) var<storage, read> render_view: RenderView;

var<workgroup> shared_cluster_base: u32;

fn should_cull_instance(instance_id: u32, local_aabb: Aabb) -> bool {
    let m = instances[instance_id].model_matrix;
    
    let local_center = vec3<f32>(local_aabb.center_x, local_aabb.center_y, local_aabb.center_z);
    let local_extents = vec3<f32>(local_aabb.extents_x, local_aabb.extents_y, local_aabb.extents_z);

    let world_center = (m * vec4<f32>(local_center, 1.0)).xyz;

    // Transform extents: dot local extents with absolute rows of the matrix
    let row0 = abs(vec3<f32>(m[0].x, m[1].x, m[2].x));
    let row1 = abs(vec3<f32>(m[0].y, m[1].y, m[2].y));
    let row2 = abs(vec3<f32>(m[0].z, m[1].z, m[2].z));
    
    let world_extents = vec3<f32>(
        dot(row0, local_extents),
        dot(row1, local_extents),
        dot(row2, local_extents)
    );

    for (var i = 0u; i < 6u; i = i + 1u) {
        let plane = render_view.planes[i];
        let radius = dot(abs(plane.xyz), world_extents);
        let distance = dot(plane.xyz, world_center) + plane.w;

        if (distance < -radius) {
            return true; 
        }
    }
    return false;
}

@compute @workgroup_size(1024, 1, 1)
fn cull_instances(
    @builtin(workgroup_id) block_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
) {
    let instance_id = block_id.x;
    let aabb = instance_aabbs[instance_id];

    if should_cull_instance(instance_id, aabb) { return; }

    let meshlet_count = instance_meshlet_counts[instance_id];
    let meshlet_base = instance_meshlet_offsets[instance_id];

    if local_id.x == 0 {
        shared_cluster_base = atomicAdd(&visible_instance_cluster_count, meshlet_count);
    }

    workgroupBarrier();
    let cluster_base = shared_cluster_base;

    for (var i = local_id.x; i < meshlet_count; i += 1024u) {
        let cluster_id = cluster_base + i;
        let meshlet_id = meshlet_base + i;

        cluster_info[cluster_id] = ClusterInfo(instance_id, meshlet_id);
    }
}