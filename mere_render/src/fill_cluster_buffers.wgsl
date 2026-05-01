struct ComputeParams {
    cluster_count: u32,
};

@group(0) @binding(0) var<uniform> params: ComputeParams;
@group(0) @binding(1) var<storage, read> instance_meshlet_counts_prefix_sum: array<u32>;
@group(0) @binding(2) var<storage, read> meshlet_instance_meshlet_slice_starts: array<u32>;

// Output Buffers
@group(0) @binding(3) var<storage, read_write> meshlet_cluster_instance_ids: array<u32>;
@group(0) @binding(4) var<storage, read_write> meshlet_cluster_meshlet_ids: array<u32>;

@compute @workgroup_size(128, 1, 1)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>
) {
    let cluster_id = global_id.x;
    
    if (cluster_id >= params.cluster_count) {
        return;
    }


    var left = 0u;
    var right = arrayLength(&instance_meshlet_counts_prefix_sum) - 1u;
    while (left <= right) {
        let mid = (left + right) / 2u;
        if (instance_meshlet_counts_prefix_sum[mid] <= cluster_id) {
            left = mid + 1u;
        } else {
            right = mid - 1u;
        }
    }
    
    let instance_id = right;
    let meshlet_id_local = cluster_id - instance_meshlet_counts_prefix_sum[instance_id];
    let meshlet_id = meshlet_id_local + meshlet_instance_meshlet_slice_starts[instance_id];

    // Write to persistent buffers for the Render Pass
    meshlet_cluster_instance_ids[cluster_id] = instance_id;
    meshlet_cluster_meshlet_ids[cluster_id] = meshlet_id;
}