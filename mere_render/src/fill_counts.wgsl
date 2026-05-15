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
@group(0) @binding(2) var<storage, read_write> visible_instance_cluster_count: u32;
@group(0) @binding(3) var<storage, read_write> previous_raster_counts: u32;

@compute
@workgroup_size(1, 1, 1)
fn fill_counts() {
    cluster_indirect_args.x = 0;
    visible_instance_cluster_count = 0;

    previous_raster_counts += raster_indirect_args.instance_count;
    raster_indirect_args.instance_count = 0;
}