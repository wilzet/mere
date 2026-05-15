struct RenderView {
    world_position: vec4<f32>,
    viewport: vec4<f32>,
    view_proj: mat4x4<f32>,
    previous_view_proj: mat4x4<f32>,
    // 6 planes: Left, Right, Top, Bottom, Near, Far
    planes: array<vec4<f32>, 6>,
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

struct MeshUniform {
    model_matrix: mat3x4<f32>,
    previous_model_matrix: mat3x4<f32>,
    inverse_transpose_a: array<vec4<f32>, 2>,
    inverse_transpose_b: vec4<f32>,
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

@group(0) @binding(4) var depth_pyramid: texture_2d<f32>;

@group(0) @binding(5) var<storage, read_write> visible_cluster_info: array<ClusterInfo>;
@group(0) @binding(6) var<storage, read_write> indirect_args: DrawIndirectArgs;
@group(0) @binding(7) var<storage, read> previous_raster_count: u32;

@group(1) @binding(0) var<uniform> render_view: RenderView;

struct ScreenAabb {
    min: vec3<f32>,
    max: vec3<f32>,
}

// Helper to find the min of 8 vectors
fn min8(p0: vec3<f32>, p1: vec3<f32>, p2: vec3<f32>, p3: vec3<f32>,
    p4: vec3<f32>, p5: vec3<f32>, p6: vec3<f32>, p7: vec3<f32>) -> vec3<f32> {
    return min(min(min(p0, p1), min(p2, p3)), min(min(p4, p5), min(p6, p7)));
}

// Helper to find the max of 8 vectors
fn max8(p0: vec3<f32>, p1: vec3<f32>, p2: vec3<f32>, p3: vec3<f32>,
    p4: vec3<f32>, p5: vec3<f32>, p6: vec3<f32>, p7: vec3<f32>) -> vec3<f32> {
    return max(max(max(p0, p1), max(p2, p3)), max(max(p4, p5), max(p6, p7)));
}

fn project_aabb(clip_from_local: mat4x4<f32>, near: f32, bounds: BoundingSphere, out: ptr<function, ScreenAabb>) -> bool {
    let center = bounds.center_radius.xyz;
    let half_extents = vec3(bounds.center_radius.w);

    let extents = half_extents * 2;
    let sx = clip_from_local * vec4<f32>(extents.x, 0.0, 0.0, 0.0);
    let sy = clip_from_local * vec4<f32>(0.0, extents.y, 0.0, 0.0);
    let sz = clip_from_local * vec4<f32>(0.0, 0.0, extents.z, 0.0);

    let p0 = clip_from_local * vec4<f32>(center - half_extents, 1.0);
    let p1 = p0 + sz;
    let p2 = p0 + sy;
    let p3 = p2 + sz;
    let p4 = p0 + sx;
    let p5 = p4 + sz;
    let p6 = p4 + sy;
    let p7 = p6 + sz;

    // Near plane check
    // If any point is in front of the near plane, the AABB might be bisected by the camera.
    let min_w = min(min(min(p0.w, p1.w), min(p2.w, p3.w)), min(min(p4.w, p5.w), min(p6.w, p7.w)));
    if min_w < near {
        return false;
    }

    let dp0 = p0.xyz / p0.w;
    let dp1 = p1.xyz / p1.w;
    let dp2 = p2.xyz / p2.w;
    let dp3 = p3.xyz / p3.w;
    let dp4 = p4.xyz / p4.w;
    let dp5 = p5.xyz / p5.w;
    let dp6 = p6.xyz / p6.w;
    let dp7 = p7.xyz / p7.w;

    let ndc_min = min8(dp0, dp1, dp2, dp3, dp4, dp5, dp6, dp7);
    let ndc_max = max8(dp0, dp1, dp2, dp3, dp4, dp5, dp6, dp7);
    var vaabb = vec4<f32>(ndc_min.xy, ndc_max.xy);
    vaabb = vaabb.xwzy * vec4<f32>(0.5, -0.5, 0.5, -0.5) + 0.5;

    (*out).min = vec3<f32>(vaabb.xy, ndc_min.z);
    (*out).max = vec3<f32>(vaabb.zw, ndc_max.z);

    return true;
}

fn gather_hzb_row(x: vec4<u32>, y: u32, mip: u32) -> f32 {
    let d0 = textureLoad(depth_pyramid, vec2(x.x, y), mip).r;
    let d1 = textureLoad(depth_pyramid, vec2(x.y, y), mip).r;
    let d2 = textureLoad(depth_pyramid, vec2(x.z, y), mip).r;
    let d3 = textureLoad(depth_pyramid, vec2(x.w, y), mip).r;
    return min(min(d0, d1), min(d2, d3));
}

fn is_occluded(instance_id: u32, meshlet: Meshlet) -> bool {
    let projection = render_view.view_proj;
    var near: f32;
    if projection[3][3] == 1.0 {
        near = projection[3][2] / projection[2][2];
    } else {
        near = projection[3][2];
    }

    let m = instances[instance_id].model_matrix;
    let model_matrix = transpose(mat4x4(
        m[0],
        m[1],
        m[2],
        vec4(0.0, 0.0, 0.0, 1.0),
    ));

    let clip_from_local = projection * model_matrix;
    var screen_aabb = ScreenAabb(vec3(0.0), vec3(0.0));
    if project_aabb(clip_from_local, near, meshlet.bounds, &screen_aabb) {
        let hzb_size = vec2<f32>(textureDimensions(depth_pyramid).xy);

        let aabb_min_px = screen_aabb.min.xy * hzb_size;
        let aabb_max_px = screen_aabb.max.xy * hzb_size;

        let min_texel = vec2<u32>(max(aabb_min_px, vec2<f32>(0.0)));
        let max_texel = vec2<u32>(min(aabb_max_px, hzb_size - 1.0));
        let max_size = max(max_texel.x - min_texel.x, max_texel.y - min_texel.y);

        // Target a mip where the AABB is roughly 4x4 texels
        var mip = max(0, firstLeadingBit(max_size) - 3);

        // Check if the AABB spans more than 4 texels at this mip
        if any((max_texel >> vec2(mip)) > ((min_texel >> vec2(mip)) + 3)) {
            mip += 1;
        }

        let smin = min_texel >> vec2(mip);
        let smax = max_texel >> vec2(mip);

        // Cover a 4x4 area with four 2x2 gathers:
        let cx = min(smin.x + vec4(0, 1, 2, 3), smax.xxxx);
        let cy = min(smin.y + vec4(0, 1, 2, 3), smax.yyyy);

        let d0 = gather_hzb_row(cx, cy.x, mip);
        let d1 = gather_hzb_row(cx, cy.y, mip);
        let d2 = gather_hzb_row(cx, cy.z, mip);
        let d3 = gather_hzb_row(cx, cy.w, mip);

        let curr_depth = min(min(d0, d1), min(d2, d3));

        return screen_aabb.max.z <= curr_depth;
    }

    return false;
}

@compute @workgroup_size(128, 1, 1)
fn cull_clusters(
    @builtin(global_invocation_id) global_id: vec3<u32>
) {
    let cluster_id = global_id.x;

    if cluster_id >= visible_instance_cluster_count { return; }

    let info = cluster_info[cluster_id];
    let instance_id = info.instance_id;

    let meshlet_id = info.meshlet_id;

    let meshlet = meshlets[meshlet_id];

    if is_occluded(instance_id, meshlet) { return; }

    let id = atomicAdd(&indirect_args.instance_count, 1u);
    let offset = previous_raster_count;
    visible_cluster_info[offset + id] = info;
}
