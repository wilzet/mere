@group(0) @binding(0) var meshlet_visibility_buffer: texture_storage_2d<r64uint, write>;
var<immediate> view_size: vec2<u32>;

@compute
@workgroup_size(16, 16, 1)
fn visibility_buffer_clear(@builtin(global_invocation_id) global_id: vec3<u32>) {
    if any(global_id.xy >= view_size) { return; }

    textureStore(meshlet_visibility_buffer, global_id.xy, vec4(0lu));
}