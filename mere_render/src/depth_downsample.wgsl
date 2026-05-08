@group(0) @binding(0) var mip_0: texture_storage_2d<r32uint, read>;
@group(0) @binding(1) var mip_1: texture_storage_2d<r32float, write>;
@group(0) @binding(2) var mip_2: texture_storage_2d<r32float, write>;
@group(0) @binding(3) var mip_3: texture_storage_2d<r32float, write>;
@group(0) @binding(4) var mip_4: texture_storage_2d<r32float, write>;
@group(0) @binding(5) var mip_5: texture_storage_2d<r32float, write>;
@group(0) @binding(6) var mip_6: texture_storage_2d<r32float, read_write>;
@group(0) @binding(7) var mip_7: texture_storage_2d<r32float, write>;
@group(0) @binding(8) var mip_8: texture_storage_2d<r32float, write>;
@group(0) @binding(9) var mip_9: texture_storage_2d<r32float, write>;
@group(0) @binding(10) var mip_10: texture_storage_2d<r32float, write>;
@group(0) @binding(11) var mip_11: texture_storage_2d<r32float, write>;
@group(0) @binding(12) var mip_12: texture_storage_2d<r32float, write>;
@group(0) @binding(13) var s_mips: sampler;

@compute @workgroup_size(1, 1, 1)
fn main() {}