struct Aabb {
    center_x: f32,
    center_y: f32,
    center_z: f32,
    extents_x: f32,
    extents_y: f32,
    extents_z: f32,
} 

struct ClusterInfo {
    instance_id: u32,
    meshlet_id: u32,
}

@group(0) @binding(0) var<storage, read> instance_aabbs: array<Aabb>;
@group(0) @binding(1) var<storage, read> instance_meshlet_offsets: array<u32>;
@group(0) @binding(2) var<storage, read> instance_meshlet_counts: array<u32>;

@group(0) @binding(3) var<storage, read_write> cluster_info: array<ClusterInfo>;
@group(0) @binding(4) var<storage, read_write> visible_instance_cluster_count: atomic<u32>;

var<workgroup> shared_cluster_base: u32;

fn should_cull_instance(instance_id: u32, aabb: Aabb) -> bool {
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