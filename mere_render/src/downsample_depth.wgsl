@group(0) @binding(0) var mip_0: texture_storage_2d<r64uint, read>;
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

struct Constants {
    max_mip_level: u32,
}
var<immediate> constants: Constants;

var<workgroup> shared_memory: array<f32, 256>;

@compute @workgroup_size(256, 1, 1)
fn downsample_depth_first(
    @builtin(workgroup_id) workgroup_id: vec3<u32>,
    @builtin(local_invocation_index) local_id: u32,
) {
    let block_xy = remap_to_block(local_id % 64);
    let x = block_xy.x + 8 * ((local_id >> 6) % 2);
    let y = block_xy.y + 8 * ((local_id >> 7));

    downsample_mips_0_and_1(vec2(x, y), workgroup_id.xy, local_id);
}

fn downsample_mips_0_and_1(coord: vec2<u32>, workgroup_id: vec2<u32>, local_id: u32) {
    var v: vec4<f32>;

    var texel_1 = workgroup_id * 32 + coord;
    var texel_0 = texel_1 * 2;
    v[0] = reduce_load_mip_0(texel_0);
    textureStore(mip_1, texel_1, vec4(v[0]));

    texel_1 = workgroup_id * 32 + vec2(coord.x + 16, coord.y);
    texel_0 = texel_1 * 2;
    v[1] = reduce_load_mip_0(texel_0);
    textureStore(mip_1, texel_1, vec4(v[1]));

    texel_1 = workgroup_id * 32 + vec2(coord.x, coord.y + 16);
    texel_0 = texel_1 * 2;
    v[2] = reduce_load_mip_0(texel_0);
    textureStore(mip_1, texel_1, vec4(v[2]));

    texel_1 = workgroup_id * 32 + coord + 16;
    texel_0 = texel_1 * 2;
    v[3] = reduce_load_mip_0(texel_0);
    textureStore(mip_1, texel_1, vec4(v[3]));

    if constants.max_mip_level <= 1 { return; }

    let idx = coord.y * 16 + coord.x;
    for (var i = 0u; i < 4; i++) {
        shared_memory[idx] = v[i];
        workgroupBarrier();

        if local_id < 64 {
            v[i] = reduce_4(vec4(
                shared_memory[idx * 2],
                shared_memory[idx * 2 + 1],
                shared_memory[idx * 2 + 16],
                shared_memory[idx * 2 + 1 + 16],
            ));
            texel_1 = workgroup_id * 16 + coord + vec2(i % 2, i / 2) * 8;
            textureStore(mip_2, texel_1, vec4(v[i]));
        }
        workgroupBarrier();
    }

    if local_id < 64 {
        shared_memory[idx] = v[0];
        shared_memory[idx + 8] = v[1];
        shared_memory[idx + 128] = v[2];
        shared_memory[idx + 8 + 128] = v[3];
    }
}

fn downsample_mips_2_to_5(coord: vec2<u32>, workgroup_id: vec2u, local_id: u32) {
    if constants.max_mip_level <= 2u { return; }
    workgroupBarrier();
    downsample_mip_2(coord, workgroup_id, local_id);

    if constants.max_mip_level <= 3u { return; }
    workgroupBarrier();
    downsample_mip_3(coord, workgroup_id, local_id);

    if constants.max_mip_level <= 4u { return; }
    workgroupBarrier();
    downsample_mip_4(coord, workgroup_id, local_id);

    if constants.max_mip_level <= 5u { return; }
    workgroupBarrier();
    downsample_mip_5(workgroup_id, local_id);
}

fn downsample_mip_2(coord: vec2<u32>, workgroup_id: vec2u, local_id: u32) {
    if local_id < 64u {
        let idx = coord.y * 16 + coord.x;
        let v = reduce_4(vec4(
            shared_memory[idx * 2],
            shared_memory[idx * 2 + 1],
            shared_memory[idx * 2 + 16],
            shared_memory[idx * 2 + 1 + 16],
        ));
        textureStore(mip_3, workgroup_id * 8 + coord, vec4(v));
        shared_memory[idx * 2 + coord.y % 2] = v;
    }
}

fn downsample_mip_3(coord: vec2<u32>, workgroup_id: vec2u, local_id: u32) {
    if local_id < 16u {
        let idx = coord.y * 16 + coord.x;
        let v = reduce_4(vec4(
            shared_memory[idx * 4],
            shared_memory[idx * 4 + 2],
            shared_memory[idx * 4 + 1 + 32],
            shared_memory[idx * 4 + 3 + 32],
        ));
        textureStore(mip_4, workgroup_id * 4 + coord, vec4(v));
        shared_memory[idx * 4 + coord.y] = v;
    }
}

fn downsample_mip_4(coord: vec2<u32>, workgroup_id: vec2u, local_id: u32) {
    if local_id < 4u {
        let idx = coord.y * 16 + coord.x;
        let v = reduce_4(vec4(
            shared_memory[idx * 8 + coord.y * 2],
            shared_memory[idx * 8 + 4 + coord.y * 2],
            shared_memory[idx * 8 + 1 + coord.y * 2 + 64],
            shared_memory[idx * 8 + 4 + 1 + coord.y * 2 + 64],
        ));
        textureStore(mip_5, workgroup_id * 2 + coord, vec4(v));
        shared_memory[coord.x + 2 * coord.y] = v;
    }
}

fn downsample_mip_5(workgroup_id: vec2<u32>, local_id: u32) {
    if local_id < 1u {
        let v = reduce_4(vec4(
            shared_memory[0],
            shared_memory[1],
            shared_memory[2],
            shared_memory[3],
        ));
        textureStore(mip_6, workgroup_id, vec4(v));
    }
}

fn reduce_load_mip_0(coord: vec2<u32>) -> f32 {
    let a = load_mip_0(coord);
    let b = load_mip_0(vec2(coord.x, coord.y + 1));
    let c = load_mip_0(vec2(coord.x + 1, coord.y));
    let d = load_mip_0(coord + 1);
    return reduce_4(vec4(a, b, c, d));
}

fn load_mip_0(coord: vec2<u32>) -> f32 {
    let actual_size = textureDimensions(mip_0).xy;
    let virtual_size = vec2(
        next_power_of_two(actual_size.x),
        next_power_of_two(actual_size.y),
    );
    let virtual_uv = (vec2<f32>(coord) + 0.5) * vec2<f32>(actual_size) / vec2<f32>(virtual_size);
    let depth = load_mip_0_gather(virtual_uv);

    return reduce_4(depth);
}

fn load_mip_0_gather(uv: vec2<f32>) -> vec4<f32> {
    let uv0 = vec2<u32>(floor(uv - 0.5));
    let uv1 = uv0 + 1u;
    return vec4(
        bitcast<f32>(u32(textureLoad(mip_0, vec2(uv0.x, uv0.y)).r >> 32)),
        bitcast<f32>(u32(textureLoad(mip_0, vec2(uv0.x, uv1.y)).r >> 32)),
        bitcast<f32>(u32(textureLoad(mip_0, vec2(uv1.x, uv0.y)).r >> 32)),
        bitcast<f32>(u32(textureLoad(mip_0, vec2(uv1.x, uv1.y)).r >> 32))
    );
}

fn reduce_4(v: vec4<f32>) -> f32 {
    return min(min(v.x, v.y), min(v.z, v.w));
}

fn remap_to_block(value: u32) -> vec2u {
    return vec2(
        insertBits(extractBits(value, 2u, 3u), value, 0u, 1u),
        insertBits(extractBits(value, 3u, 3u), extractBits(value, 1u, 2u), 0u, 2u),
    );
}

fn next_power_of_two(value: u32) -> u32 {
    return 1u << (32 - countLeadingZeros(value));
}
