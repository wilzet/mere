use half::f16;
use mere_math::{Vec2, Vec3};

pub fn calculate_tangents(
    indices_iter: &[u32],
    positions: &[Vec3],
    normals: &[Vec3],
    tex_coords: &[Vec2],
) -> Vec<u32> {
    let mut tangents = vec![Vec3::ZERO; positions.len()];
    let mut bitangents = vec![Vec3::ZERO; positions.len()];

    for chunk in indices_iter.chunks(3) {
        let i0 = chunk[0] as usize;
        let i1 = chunk[1] as usize;
        let i2 = chunk[2] as usize;

        let edge0 = positions[i1] - positions[i0];
        let edge1 = positions[i2] - positions[i0];

        let delta_uv0 = tex_coords[i1] - tex_coords[i0];
        let delta_uv1 = tex_coords[i2] - tex_coords[i0];

        let det = delta_uv0.x * delta_uv1.y - delta_uv1.x * delta_uv0.y;

        let (tangent, bitangent) = if det.abs() < f32::EPSILON {
            let normal = normals[i0];
            let helper = if normal.x.abs() < 0.9 {
                Vec3::X
            } else {
                Vec3::Y
            };
            let t = normal.cross(helper);
            let b = normal.cross(t);
            (t, b)
        } else {
            let f = 1.0 / det;
            (
                (edge0 * delta_uv1.y - edge1 * delta_uv0.y) * f,
                (edge1 * delta_uv0.x - edge0 * delta_uv1.x) * f,
            )
        };

        tangents[i0] += tangent;
        tangents[i1] += tangent;
        tangents[i2] += tangent;

        bitangents[i0] += bitangent;
        bitangents[i1] += bitangent;
        bitangents[i2] += bitangent;
    }

    tangents
        .into_iter()
        .enumerate()
        .map(|(i, t)| {
            if t.length_squared() == 0.0 {
                return pack_10_10_10_2(Vec3::X, 1);
            }

            let b = bitangents[i];
            let n = normals[i];

            let t_ortho = (t - n * n.dot(t)).normalize();

            let handedness = if n.cross(t_ortho).dot(b) < 0.0 {
                // negative
                0
            } else {
                1
            };

            pack_10_10_10_2(t_ortho, handedness)
        })
        .collect()
}

pub fn pack_11_11_10(value: Vec3) -> u32 {
    let x = ((value.x * 0.5 + 0.5) * 0x7ff as f32) as u32;
    let y = ((value.y * 0.5 + 0.5) * 0x7ff as f32) as u32;
    let z = ((value.z * 0.5 + 0.5) * 0x3ff as f32) as u32;
    x | (y << 11) | (z << 22)
}

pub fn pack_16_16(value: Vec2) -> u32 {
    let x = f16::from_f32(value.x).to_bits() as u32;
    let y = f16::from_f32(value.y).to_bits() as u32;
    x | (y << 16)
}

pub fn pack_10_10_10_2(value: Vec3, extra: u32) -> u32 {
    let x = ((value.x * 0.5 + 0.5) * 0x3ff as f32) as u32;
    let y = ((value.y * 0.5 + 0.5) * 0x3ff as f32) as u32;
    let z = ((value.z * 0.5 + 0.5) * 0x3ff as f32) as u32;
    let last = extra & 0x3;
    x | (y << 10) | (z << 20) | (last << 30)
}
