use bevy_math::{DVec3, UVec3};
use copc_streaming::Aabb;

const GRID_SIZE: f64 = 32.0;
const GRID_SIZE_UINT: u32 = GRID_SIZE as u32;
const GRID_SIZE_SPLAT: UVec3 = UVec3::splat(GRID_SIZE as u32 - 1);

fn to_index(position: &DVec3, size: &DVec3) -> usize {
    let index = (GRID_SIZE * position / size)
        .as_uvec3()
        .min(GRID_SIZE_SPLAT);

    (index.x + GRID_SIZE_UINT * index.y + GRID_SIZE_UINT * GRID_SIZE_UINT * index.z) as usize
}

pub fn compute_density(points: &Vec<las::Point>, aabb: &Aabb) -> u32 {
    let size = DVec3::new(
        aabb.max[0] - aabb.min[0],
        aabb.max[1] - aabb.min[1],
        aabb.max[2] - aabb.min[2],
    );

    let mut grid = vec![0_u32; (GRID_SIZE_UINT * GRID_SIZE_UINT * GRID_SIZE_UINT) as usize];
    let mut num_occupied_cells = 0;

    for point in points {
        let position = DVec3::new(
            point.x - aabb.min[0],
            point.y - aabb.min[1],
            point.z - aabb.min[2],
        );

        let index = to_index(&position, &size);
        grid[index] += 1;
        if grid[index] == 1 {
            num_occupied_cells += 1;
        }
    }

    if num_occupied_cells == 0 {
        0
    } else {
        (points.len() / num_occupied_cells) as u32
    }
}
