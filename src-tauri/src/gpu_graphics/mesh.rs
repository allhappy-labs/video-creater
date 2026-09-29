use super::error::{GpuGraphicsError, GpuGraphicsErrorCode, GpuGraphicsResult};
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub topology: MeshTopology,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshTopology {
    Triangles,
    Lines,
}

pub fn build_cube_mesh(size: f32) -> GpuGraphicsResult<Mesh> {
    if !size.is_finite() || size <= 0.0 {
        return Err(vec![GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid,
            "primitive.size",
            "Cube size is invalid.",
            "Use a finite cube size greater than 0.",
        )]);
    }

    let h = size / 2.0;
    let faces = [
        (
            [0.0, 0.0, 1.0],
            [[-h, -h, h], [h, -h, h], [h, h, h], [-h, h, h]],
        ),
        (
            [0.0, 0.0, -1.0],
            [[h, -h, -h], [-h, -h, -h], [-h, h, -h], [h, h, -h]],
        ),
        (
            [1.0, 0.0, 0.0],
            [[h, -h, h], [h, -h, -h], [h, h, -h], [h, h, h]],
        ),
        (
            [-1.0, 0.0, 0.0],
            [[-h, -h, -h], [-h, -h, h], [-h, h, h], [-h, h, -h]],
        ),
        (
            [0.0, 1.0, 0.0],
            [[-h, h, h], [h, h, h], [h, h, -h], [-h, h, -h]],
        ),
        (
            [0.0, -1.0, 0.0],
            [[-h, -h, -h], [h, -h, -h], [h, -h, h], [-h, -h, h]],
        ),
    ];

    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);
    for (face_index, (normal, positions)) in faces.iter().enumerate() {
        let base = (face_index * 4) as u32;
        for (uv, position) in [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]
            .into_iter()
            .zip(*positions)
        {
            vertices.push(Vertex {
                position,
                normal: *normal,
                uv,
            });
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    Ok(Mesh {
        vertices,
        indices,
        topology: MeshTopology::Triangles,
    })
}

pub fn build_grid_mesh(divisions: u32, size: f32) -> GpuGraphicsResult<Mesh> {
    if divisions == 0 || !size.is_finite() || size <= 0.0 {
        return Err(vec![GpuGraphicsError::new(
            GpuGraphicsErrorCode::GpuGraphicsPrimitiveInvalid,
            "primitive.grid",
            "Grid divisions and size are invalid.",
            "Use divisions greater than 0 and finite size greater than 0.",
        )]);
    }

    let half = size / 2.0;
    let step = size / divisions as f32;
    let mut vertices = Vec::with_capacity(((divisions + 1) * 4) as usize);
    let mut indices = Vec::with_capacity(((divisions + 1) * 4) as usize);

    for index in 0..=divisions {
        let value = -half + step * index as f32;
        let base = vertices.len() as u32;
        vertices.push(Vertex {
            position: [-half, 0.0, value],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
        });
        vertices.push(Vertex {
            position: [half, 0.0, value],
            normal: [0.0, 1.0, 0.0],
            uv: [1.0, 0.0],
        });
        vertices.push(Vertex {
            position: [value, 0.0, -half],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 1.0],
        });
        vertices.push(Vertex {
            position: [value, 0.0, half],
            normal: [0.0, 1.0, 0.0],
            uv: [1.0, 1.0],
        });
        indices.extend_from_slice(&[base, base + 1, base + 2, base + 3]);
    }

    Ok(Mesh {
        vertices,
        indices,
        topology: MeshTopology::Lines,
    })
}
