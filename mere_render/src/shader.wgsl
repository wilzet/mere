// --- Vertex shader ---

struct CameraUniform {
    view_proj: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) tex_coord: vec2<f32>,
}

struct InstanceInput {
    @location(5) model_matrix_0: vec4<f32>,
    @location(6) model_matrix_1: vec4<f32>,
    @location(7) model_matrix_2: vec4<f32>,
    @location(8) model_matrix_3: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(1) color: vec3<f32>,
    @location(2) normal: vec3<f32>,
    @location(0) tex_coord: vec2<f32>,
}

@vertex
fn vs_main(
    model: VertexInput,
    instance: InstanceInput,
) -> VertexOutput {
    let model_matrix = mat4x4(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );
    var out: VertexOutput;
    out.clip_position = camera.view_proj * model_matrix * vec4(model.position, 1.0);
    out.normal = model.normal;
    out.tex_coord = model.tex_coord;
    out.color = model.color;
    return out;
}

// --- Fragment shader ---

@group(1) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(1) @binding(1)
var s_diffuse: sampler;
@group(1) @binding(2)
var t_normal: texture_2d<f32>;
@group(1) @binding(3)
var s_normal: sampler;
@group(1) @binding(4)
var t_roughness_metalness: texture_2d<f32>;
@group(1) @binding(5)
var s_roughness_metalness: sampler;
@group(1) @binding(6)
var<uniform> albedo: vec4<f32>;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return albedo * vec4(in.color, 1.0) * textureSample(t_diffuse, s_diffuse, in.tex_coord); // * vec4(in.normal * 0.5 + 0.5, 1.0);
}
