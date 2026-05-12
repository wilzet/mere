struct RenderView {
    world_position: vec4<f32>,
    viewport: vec4<f32>,
    view_proj: mat4x4<f32>,
    // 6 planes: Left, Right, Top, Bottom, Near, Far
    planes: array<vec4<f32>, 6>,
}

struct MaterialProperties {
    color: vec4<f32>,
    normal_scale: f32,
    roughness: f32,
    metalness: f32,
}

// --- Vertex shader ---

struct Vertex {
    position: array<f32, 3>,
}

struct VertexAttributes {
    normal: u32,
    uv: u32,
    tangent: u32,
}

struct FullVertex {
    position: vec3<f32>,
    normal: vec3<f32>,
    uv: vec2<f32>,
    tangent: vec4<f32>,
}

struct BoundingSphere {
    center_x: f32,
    center_y: f32,
    center_z: f32,
    radius: f32,
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
    previous_model: mat3x4<f32>,
    inverse_transpose_a: array<vec4<f32>, 2>,
    inverse_transpose_b: vec4<f32>,
}

struct ClusterInfo {
    instance_id: u32,
    meshlet_id: u32,
}

@group(0) @binding(0) var<storage, read> main_camera: RenderView;

@group(1) @binding(0) var<storage, read> vertices: array<Vertex>;
@group(1) @binding(1) var<storage, read> vertex_attributes: array<VertexAttributes>;
@group(1) @binding(2) var<storage, read> meshlet_vertex_indices: array<u32>;
@group(1) @binding(3) var<storage, read> meshlet_indices: array<u32>;

@group(1) @binding(4) var<storage, read> meshlets: array<Meshlet>;
@group(1) @binding(5) var<storage, read> instances: array<MeshUniform>;
@group(1) @binding(6) var<storage, read> cluster_info: array<ClusterInfo>;

@group(1) @binding(7) var visibility_buffer: texture_storage_2d<r64uint, read>;

@vertex
fn vs_main(
    @builtin(vertex_index) index_id: u32,
) -> @builtin(position) vec4<f32> {
    let vertex_index = index_id % 3;
    let material_id = index_id / 3;
    let material_depth = f32(material_id) / 65535.0;
    let uv = vec2<f32>(vec2(vertex_index >> 1, vertex_index & 1)) * 2.0;
    return vec4(uv_to_ndc(uv), material_depth, 1.0);
}

// --- Fragment shader ---

const PI: f32 = 3.14159265359;

struct PartialDerivatives {
    barycentrics: vec3<f32>,
    ddx: vec3<f32>,
    ddy: vec3<f32>,
}

fn compute_partial_derivatives(vertex_world_positions: array<vec4<f32>, 3>, ndc_uv: vec2<f32>, half_screen_size: vec2<f32>) -> PartialDerivatives {
    var result: PartialDerivatives;

    let vertex_clip_position_0 = main_camera.view_proj * vec4(vertex_world_positions[0].xyz, 1.0);
    let vertex_clip_position_1 = main_camera.view_proj * vec4(vertex_world_positions[1].xyz, 1.0);
    let vertex_clip_position_2 = main_camera.view_proj * vec4(vertex_world_positions[2].xyz, 1.0);

    let inv_w = 1.0 / vec3(vertex_clip_position_0.w, vertex_clip_position_1.w, vertex_clip_position_2.w);
    let ndc_0 = vertex_clip_position_0.xy * inv_w[0];
    let ndc_1 = vertex_clip_position_1.xy * inv_w[1];
    let ndc_2 = vertex_clip_position_2.xy * inv_w[2];

    let inv_det = 1.0 / determinant(mat2x2(ndc_2 - ndc_1, ndc_0 - ndc_1));
    result.ddx = vec3(ndc_1.y - ndc_2.y, ndc_2.y - ndc_0.y, ndc_0.y - ndc_1.y) * inv_det * inv_w;
    result.ddy = vec3(ndc_2.x - ndc_1.x, ndc_0.x - ndc_2.x, ndc_1.x - ndc_0.x) * inv_det * inv_w;

    var ddx_sum = dot(result.ddx, vec3(1.0));
    var ddy_sum = dot(result.ddy, vec3(1.0));

    let delta_v = ndc_uv - ndc_0;
    let interp_inv_w = inv_w.x + delta_v.x * ddx_sum + delta_v.y * ddy_sum;
    let interp_w = 1.0 / interp_inv_w;

    result.barycentrics = vec3(
        inv_w.x + delta_v.x * result.ddx.x + delta_v.y * result.ddy.x,
        delta_v.x * result.ddx.y + delta_v.y * result.ddy.y,
        delta_v.x * result.ddx.z + delta_v.y * result.ddy.z,
    ) * interp_w;

    result.ddx *= half_screen_size.x;
    result.ddy *= -half_screen_size.y;

    ddx_sum *= half_screen_size.x;
    ddy_sum *= -half_screen_size.y;

    let interp_ddx_w = 1.0 / (interp_inv_w + ddx_sum);
    let interp_ddy_w = 1.0 / (interp_inv_w + ddy_sum);

    result.ddx = interp_ddx_w * (result.barycentrics * interp_inv_w + result.ddx) - result.barycentrics;
    result.ddy = interp_ddy_w * (result.barycentrics * interp_inv_w + result.ddy) - result.barycentrics;

    return result;
}

struct VertexOutput {
    position: vec4<f32>,
    world_position: vec4<f32>,
    world_normal: vec3<f32>,
    uv: vec2<f32>,
    ddx_uv: vec2<f32>,
    ddy_uv: vec2<f32>,
    world_tangent: vec4<f32>,
    cluster_id: u32,
    instance_id: u32,
    triangle_id: u32,
}

fn get_vertex_output(frag_coord: vec4<f32>) -> VertexOutput {
    let packed_ids = u32(textureLoad(visibility_buffer, vec2<u32>(frag_coord.xy)).r);
    let cluster_id = packed_ids >> 7;
    let info = cluster_info[cluster_id];
    let meshlet_id = info.meshlet_id;
    let meshlet = meshlets[meshlet_id];

    let triangle_id = extractBits(packed_ids, 0, 7);
    let index_ids = triangle_id * 3 + vec3(0, 1, 2);
    let vertex_ids = vec3(get_meshlet_vertex_id(meshlet, index_ids[0]), get_meshlet_vertex_id(meshlet, index_ids[1]), get_meshlet_vertex_id(meshlet, index_ids[2]));
    let vertex_0 = get_vertex(vertex_ids[0]);
    let vertex_1 = get_vertex(vertex_ids[1]);
    let vertex_2 = get_vertex(vertex_ids[2]);

    let instance_id = info.instance_id;
    let instance = instances[instance_id];

    let model_matrix = transpose(mat4x4(
        instance.model_matrix[0],
        instance.model_matrix[1],
        instance.model_matrix[2],
        vec4(0.0, 0.0, 0.0, 1.0),
    ));
    let world_position_0 = model_matrix * vec4(vertex_0.position, 1.0);
    let world_position_1 = model_matrix * vec4(vertex_1.position, 1.0);
    let world_position_2 = model_matrix * vec4(vertex_2.position, 1.0);

    let frag_coord_ndc = frag_coord_to_ndc(frag_coord);
    let partial_derivatives = compute_partial_derivatives(
        array(world_position_0, world_position_1, world_position_2),
        frag_coord_ndc,
        main_camera.viewport.zw / 2.0
    );

    let world_position = mat3x4(world_position_0, world_position_1, world_position_2) * partial_derivatives.barycentrics;

    let n_a = instance.inverse_transpose_a;
    let n_b = instance.inverse_transpose_b;
    let normal_matrix = mat3x3(
        n_a[0].xyz,
        vec3(n_a[0].w, n_a[1].xy),
        vec3(n_a[1].zw, n_b.x),
    );
    let world_normal = mat3x3(
        normalize(normal_matrix * vertex_0.normal),
        normalize(normal_matrix * vertex_1.normal),
        normalize(normal_matrix * vertex_2.normal),
    ) * partial_derivatives.barycentrics;

    let uv = mat3x2(vertex_0.uv, vertex_1.uv, vertex_2.uv) * partial_derivatives.barycentrics;
    let ddx_uv = mat3x2(vertex_0.uv, vertex_1.uv, vertex_2.uv) * partial_derivatives.ddx;
    let ddy_uv = mat3x2(vertex_0.uv, vertex_1.uv, vertex_2.uv) * partial_derivatives.ddy;

    let tangent_matrix = mat3x3(
        model_matrix[0].xyz,
        model_matrix[1].xyz,
        model_matrix[2].xyz,
    );
    let world_tangent_xyz = mat3x3(
        normalize(tangent_matrix * vertex_0.tangent.xyz),
        normalize(tangent_matrix * vertex_1.tangent.xyz),
        normalize(tangent_matrix * vertex_2.tangent.xyz),
    ) * partial_derivatives.barycentrics;
    let world_tangent_w = dot(
        vec3(vertex_0.tangent.w, vertex_1.tangent.w, vertex_2.tangent.w),
        partial_derivatives.barycentrics,
    );
    let world_tangent = vec4(world_tangent_xyz, world_tangent_w);

    return VertexOutput(
        frag_coord,
        world_position,
        world_normal,
        uv,
        ddx_uv,
        ddy_uv,
        world_tangent,
        instance_id ^ meshlet_id,
        instance_id,
        instance_id ^ meshlet_id ^ triangle_id,
    );
}

fn get_meshlet_vertex_id(meshlet: Meshlet, index_id: u32) -> u32 {
    let byte_index = meshlet.index_offset + index_id;
    let word_offset = byte_index / 4u;
    let bit_offset = (byte_index % 4u) * 8u;

    let packed = meshlet_indices[word_offset];
    let local_index = (packed >> bit_offset) & 0xFFu;

    return meshlet_vertex_indices[meshlet.vertex_offset + local_index];
}

fn get_vertex(index: u32) -> FullVertex {
    let vertex = vertices[index];
    let attributes = vertex_attributes[index];

    let position = vec3(vertex.position[0], vertex.position[1], vertex.position[2]);
    let normal = unpack_11_11_10(attributes.normal);
    let uv = unpack2x16float(attributes.uv);
    let tangent = unpack_10_10_10_2(attributes.tangent);

    return FullVertex(
        position,
        normal,
        uv,
        tangent,
    );
}

fn unpack_11_11_10(value: u32) -> vec3<f32> {
    let x = f32(value & 0x7ff) / f32(0x7ff);
    let y = f32((value >> 11) & 0x7ff) / f32(0x7ff);
    let z = f32(value >> 22) / f32(0x3ff);
    return vec3(x, y, z) * 2.0 - 1.0;
}

fn unpack_10_10_10_2(value: u32) -> vec4<f32> {
    let x = f32(value & 0x3ff) / f32(0x3ff);
    let y = f32((value >> 10) & 0x3ff) / f32(0x3ff);
    let z = f32((value >> 20) & 0x3ff) / f32(0x3ff);
    let w = f32(value >> 30);
    return vec4(x, y, z, w) * 2.0 - 1.0;
}

fn uv_to_ndc(uv: vec2<f32>) -> vec2<f32> {
    return uv * vec2(2.0, -2.0) + vec2(-1.0, 1.0);
}

fn frag_coord_to_ndc(frag_coord: vec4<f32>) -> vec2<f32> {
    return vec2(uv_to_ndc((frag_coord.xy - main_camera.viewport.xy) / main_camera.viewport.zw));
}

@group(2) @binding(0) var t_diffuse: texture_2d<f32>;
@group(2) @binding(1) var s_diffuse: sampler;
@group(2) @binding(2) var t_normal: texture_2d<f32>;
@group(2) @binding(3) var s_normal: sampler;
@group(2) @binding(4) var t_rough_metal: texture_2d<f32>;
@group(2) @binding(5) var s_rough_metal: sampler;
@group(2) @binding(6) var<uniform> properties: MaterialProperties;

fn distributionGGX(N: vec3<f32>, H: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let NdotH = max(dot(N, H), 0.0);
    let NdotH2 = NdotH * NdotH;
    let denom = (NdotH2 * (a2 - 1.0) + 1.0);
    return a2 / (PI * denom * denom);
}

fn geometrySchlickGGX(NdotV: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return NdotV / (NdotV * (1.0 - k) + k);
}

fn geometrySmith(NdotV: f32, NdotL: f32, roughness: f32) -> f32 {
    return geometrySchlickGGX(NdotV, roughness) * geometrySchlickGGX(NdotL, roughness);
}

fn fresnelSchlick(cosTheta: f32, F0: vec3<f32>) -> vec3<f32> {
    return F0 + (1.0 - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

fn hash_color(id: u32) -> vec3<f32> {
    let x = f32(id);
    return fract(vec3<f32>(
        sin(x * 12.9898) * 43758.5453,
        sin(x * 78.233) * 43758.5453,
        sin(x * 39.425) * 43758.5453
    ));
}

struct Debug {
    mode: u32,
}

@group(3) @binding(0) var<uniform> debug: Debug;

@fragment
fn fs_main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    let vertex_output = get_vertex_output(frag_coord);

    switch (debug.mode) {
        case 0: { // CLUSTERS
            let cluster_id = vertex_output.cluster_id;
            let color = hash_color(cluster_id + 1);

            return vec4(color, 1.0);
        }
        case 1: { // SHADED
            let uv = vertex_output.uv.xy;

            let albedo_sample = textureSample(t_diffuse, s_diffuse, uv);
            let albedo = albedo_sample.rgb * properties.color.rgb;
            let alpha = albedo_sample.a;

            if alpha < 0.5 {
                discard;
            }

            let rm = textureSample(t_rough_metal, s_rough_metal, uv);
            let roughness = rm.g * properties.roughness;
            let metalness = rm.b * properties.metalness;

            let raw_normal = textureSample(t_normal, s_normal, uv) * 2.0 - 1.0;
            let N = normalize(vec3(raw_normal.xy * properties.normal_scale, raw_normal.z));

            let world_normal = vertex_output.world_normal;
            let world_tangent = vertex_output.world_tangent;
            let world_bitangent = normalize(cross(world_normal, world_tangent.xyz) * world_tangent.w);
            let tbn_matrix = transpose(mat3x3(world_tangent.xyz, world_bitangent, world_normal));

            const light_pos: vec3<f32> = vec3(-5.0, -5.0, 10.0);
            const light_color: vec3<f32> = vec3(1.0, 1.0, 1.0);

            let world_position = vertex_output.world_position;

            let view_dir_world = main_camera.world_position.xyz - world_position.xyz;
            let light_dir_world = light_pos - world_position.xyz;
            let tangent_view_dir = tbn_matrix * view_dir_world;
            let tangent_light_dir = tbn_matrix * light_dir_world;

            let V = normalize(tangent_view_dir);
            let L = normalize(tangent_light_dir);
            let H = normalize(V + L);

            let NdotV = max(dot(N, V), 0.0);
            let NdotL = max(dot(N, L), 0.0);
            let HdotV = max(dot(N, H), 0.0);

            let F0 = mix(vec3(0.04), albedo, metalness);

            let D = distributionGGX(N, H, roughness);
            let G = geometrySmith(NdotV, NdotL, roughness);
            let F = fresnelSchlick(HdotV, F0);

            let denom = 4.0 * NdotV * NdotL + 0.0001;
            let specular = D * G * F / denom;

            let kS = F;
            let kD = (1.0 - kS) * (1.0 - metalness);

            let diffuse = kD * albedo / PI;
            let lo = (diffuse + specular) * light_color * NdotL;
            let ambient = vec3(0.03) * albedo;

            let color = ambient + lo;

            return vec4(color, properties.color.a);
        }
        case 2: { // TRIANGLES
            let triangle_id = vertex_output.triangle_id;
            let color = hash_color(triangle_id + 1);

            return vec4(color, 1.0);
        }
        case 3: { // INSTANCES
            let instance_id = vertex_output.instance_id;
            let color = hash_color(instance_id + 1);

            return vec4(color, 1.0);
        }
        case 4: { // MATERIALS
            let material_id = u32(vertex_output.position.z * 65535.0);
            let color = hash_color(material_id + 1);

            return vec4(color, 1.0);
        }
        default: {
            break;
        }
    }

    return vec4(1.0, 0.0, 0.0, 1.0);
}

// Code for vertex output and partial derivatives based on:
//  * https://github.com/bevyengine/bevy/blob/489818930b7ec268455fe371b3c5b0fb1c0c46c3/crates/bevy_pbr/src/meshlet/visibility_buffer_resolve.wgsl
//
// Edited by permission of the MIT license:
//
// MIT License
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

