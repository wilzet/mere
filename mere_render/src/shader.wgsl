struct Camera {
    view_pos: vec4<f32>,
    view_proj: mat4x4<f32>,
}

struct Light {
    position: vec3<f32>,
    color: vec3<f32>,
}

struct MaterialProperties {
    color: vec4<f32>,
    normal_scale: f32,
    roughness: f32,
    metalness: f32,
}

// --- Vertex shader ---

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var<uniform> light: Light;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: u32,
    @location(2) tex_coord: u32,
    @location(3) tangent: u32,
}

struct InstanceInput {
    @location(5) model_matrix_0: vec4<f32>,
    @location(6) model_matrix_1: vec4<f32>,
    @location(7) model_matrix_2: vec4<f32>,
    @location(8) model_matrix_3: vec4<f32>,
    @location(9) normal_matrix_0: vec3<f32>,
    @location(10) normal_matrix_1: vec3<f32>,
    @location(11) normal_matrix_2: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) tangent_view_dir: vec3<f32>,
    @location(2) tangent_light_dir: vec3<f32>,
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

@vertex
fn vs_main(
    model: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;
    let model_matrix = mat4x4(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );
    let normal_matrix = mat3x3(
        instance.normal_matrix_0,
        instance.normal_matrix_1,
        instance.normal_matrix_2,
    );

    let normal = unpack_11_11_10(model.normal);
    let tangent = unpack_10_10_10_2(model.tangent);

    let world_position = model_matrix * vec4(model.position, 1.0);
    let world_normal = normalize(normal_matrix * normal);
    let world_tangent = normalize(normal_matrix * tangent.xyz);
    let world_bitangent = normalize(cross(world_normal, world_tangent) * tangent.w);
    let tangent_matrix = transpose(mat3x3(world_tangent, world_bitangent, world_normal));

    out.clip_position = camera.view_proj * world_position;
    out.tex_coord = unpack2x16float(model.tex_coord);

    let view_dir_world = camera.view_pos.xyz - world_position.xyz;
    let light_dir_world = light.position - world_position.xyz;
    out.tangent_view_dir = tangent_matrix * view_dir_world;
    out.tangent_light_dir = tangent_matrix * light_dir_world;
    return out;
}

// --- Fragment shader ---

const PI: f32 = 3.14159265359;

@group(2) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(2) @binding(1)
var s_diffuse: sampler;
@group(2) @binding(2)
var t_normal: texture_2d<f32>;
@group(2) @binding(3)
var s_normal: sampler;
@group(2) @binding(4)
var t_rough_metal: texture_2d<f32>;
@group(2) @binding(5)
var s_rough_metal: sampler;
@group(2) @binding(6)
var<uniform> properties: MaterialProperties;

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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let albedo_sample = textureSample(t_diffuse, s_diffuse, in.tex_coord);
    let albedo = albedo_sample.rgb * properties.color.rgb;
    let alpha = albedo_sample.a;

    if alpha < 0.5 {
        discard;
    }

    let rm = textureSample(t_rough_metal, s_rough_metal, in.tex_coord);
    let roughness = rm.g * properties.roughness;
    let metalness = rm.b * properties.metalness;

    let raw_normal = textureSample(t_normal, s_normal, in.tex_coord) * 2.0 - 1.0;
    let N = normalize(vec3(raw_normal.xy * properties.normal_scale, raw_normal.z));

    let V = normalize(in.tangent_view_dir);
    let L = normalize(in.tangent_light_dir);
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
    let lo = (diffuse + specular) * light.color * NdotL;
    let ambient = vec3(0.03) * albedo;

    let color = ambient + lo;

    return vec4(color, properties.color.a);
}
