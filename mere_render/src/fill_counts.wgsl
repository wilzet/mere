struct DispatchIndirectArgs {
    x: u32,
    y: u32,
    z: u32,
}

struct DrawIndirectArgs {
    vertex_count: u32,
    instance_count: u32,
    first_vertex: u32,
    first_instance: u32,
}

@group(0) @binding(0) var<storage, read_write> cluster_indirect_args: DispatchIndirectArgs;
@group(0) @binding(1) var<storage, read_write> raster_indirect_args: DrawIndirectArgs;
@group(0) @binding(2) var<storage, read_write> previous_raster_counts: u32;
@group(0) @binding(3) var<storage, read_write> second_pass_cluster_count: u32;

const CLUSTER_BLOCK_SIZE: u32 = 128;

@compute
@workgroup_size(1, 1, 1)
fn fill_counts() {
    let total = second_pass_cluster_count;
    let workgroups = (total + CLUSTER_BLOCK_SIZE - 1) / CLUSTER_BLOCK_SIZE;

    cluster_indirect_args.x = workgroups;
    cluster_indirect_args.y = 1;

    previous_raster_counts += raster_indirect_args.instance_count;
    raster_indirect_args.instance_count = 0;
}