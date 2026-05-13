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

@group(1) @binding(0) var<uniform> render_view: RenderView;

fn is_outside_frustum(instance_id: u32, meshlet: Meshlet) -> bool {
    let bounds = meshlet.bounds;
    let m = instances[instance_id].model_matrix;
    let model_matrix = transpose(mat4x4(
        m[0],
        m[1],
        m[2],
        vec4(0.0, 0.0, 0.0, 1.0),
    ));

    let center = (model_matrix * vec4(bounds.center_radius.xyz, 1.0)).xyz;

    let scale = max(
        length(m[0].xyz),
        max(length(m[1].xyz), length(m[2].xyz))
    );

    let radius = bounds.center_radius.w * scale;

    for (var i = 0u; i < 6u; i = i + 1u) {
        let plane = render_view.planes[i];
        let distance = dot(plane.xyz, center) + plane.w;

        if distance < -radius {
            return true;
        }
    }

    return false;
}

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

fn is_occluded(instance_id: u32, meshlet: Meshlet) -> bool {
    let projection = render_view.previous_view_proj;
    var near: f32;
    if projection[3][3] == 1.0 {
        near = projection[3][2] / projection[2][2];
    } else {
        near = projection[3][2];
    }

    let prev_m = instances[instance_id].previous_model_matrix;
    let model_matrix = transpose(mat4x4(
        prev_m[0],
        prev_m[1],
        prev_m[2],
        vec4(0.0, 0.0, 0.0, 1.0),
    ));

    let clip_from_local = projection * model_matrix;
    var screen_aabb = ScreenAabb(vec3(0.0), vec3(0.0));
    if project_aabb(clip_from_local, near, meshlet.bounds, &screen_aabb) {
        let view_size = render_view.viewport.zw;
        let rect_size_px = (screen_aabb.max.xy - screen_aabb.min.xy) * view_size;

        // We want a mip where the AABB covers ~2x2 texels to minimize samples
        let mip = i32(max(0.0, ceil(log2(max(rect_size_px.x, rect_size_px.y)))));
        let depth_size = vec2<f32>(textureDimensions(depth_pyramid, mip));
        let pixel_coords = vec2<u32>(screen_aabb.min.xy * depth_size);

        // Gather the 4 texels covering the area
        let d0 = textureLoad(depth_pyramid, pixel_coords, mip).r;
        let d1 = textureLoad(depth_pyramid, pixel_coords + vec2(1, 0), mip).r;
        let d2 = textureLoad(depth_pyramid, pixel_coords + vec2(0, 1), mip).r;
        let d3 = textureLoad(depth_pyramid, pixel_coords + vec2(1, 1), mip).r;

        // For Reversed-Z: The object is occluded if its NEAREST point
        // is LESS than the FARTHEST point in the depth buffer
        let max_depth_in_pyramid = max(max(d0, d1), max(d2, d3));

        return screen_aabb.max.z < max_depth_in_pyramid;
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

    if is_outside_frustum(instance_id, meshlet) { return; }

    if is_occluded(instance_id, meshlet) { return; }

    let id = atomicAdd(&indirect_args.instance_count, 1u);
    visible_cluster_info[id] = info;
}
