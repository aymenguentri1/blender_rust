//! Port of `uv_coords_isect_udim()` from `source/blender/editors/uvedit/uvedit_islands.cc`.

/// Subset of `Image` that `uv_coords_isect_udim()` reads: `image->source` and `image->tiles`.
#[derive(Clone, Debug, PartialEq)]
pub enum UdimImage {
    /// `image == nullptr`.
    None,
    /// Image whose `source != IMA_SRC_TILED`.
    Single,
    /// `source == IMA_SRC_TILED`, holding each `ImageTile::tile_number`.
    Tiled(Vec<i32>),
}

/// `uv_coords_isect_udim()`: whether `coords` lies on the UDIM grid or on a valid tile.
pub fn uv_coords_isect_udim(image: &UdimImage, udim_grid: [i32; 2], coords: [f32; 2]) -> bool {
    let coords_floor = [coords[0].floor(), coords[1].floor()];

    if coords[0] < udim_grid[0] as f32
        && coords[0] > 0.0
        && coords[1] < udim_grid[1] as f32
        && coords[1] > 0.0
    {
        return true;
    }
    match image {
        UdimImage::Tiled(tiles) => {
            for &tile_number in tiles {
                let tile_index = tile_number - 1001;
                let target_x = tile_index % 10;
                let target_y = tile_index / 10;
                if coords_floor[0] == target_x as f32 && coords_floor[1] == target_y as f32 {
                    return true;
                }
            }
        }
        UdimImage::Single => {
            if coords_floor[0] == 0.0 && coords_floor[1] == 0.0 {
                return true;
            }
        }
        UdimImage::None => {}
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inside_grid_is_true_for_any_image() {
        for img in [UdimImage::None, UdimImage::Single, UdimImage::Tiled(vec![])] {
            assert!(uv_coords_isect_udim(&img, [2, 2], [1.5, 0.5]));
        }
    }

    #[test]
    fn grid_bounds_are_exclusive() {
        let g = [1, 1];
        assert!(!uv_coords_isect_udim(&UdimImage::None, g, [0.0, 0.5]));
        assert!(!uv_coords_isect_udim(&UdimImage::None, g, [1.0, 0.5]));
        assert!(!uv_coords_isect_udim(&UdimImage::None, g, [0.5, 1.0]));
    }

    #[test]
    fn outside_without_image_is_false() {
        assert!(!uv_coords_isect_udim(&UdimImage::None, [1, 1], [5.5, 5.5]));
    }

    #[test]
    fn tiled_matches_floor_of_coords() {
        let img = UdimImage::Tiled(vec![1001, 1011]);
        assert!(uv_coords_isect_udim(&img, [0, 0], [0.5, 0.5])); // tile 1001 -> (0,0)
        assert!(uv_coords_isect_udim(&img, [0, 0], [0.5, 1.5])); // tile 1011 -> (0,1)
        assert!(!uv_coords_isect_udim(&img, [0, 0], [1.5, 0.5])); // tile 1002 absent
    }

    #[test]
    fn untiled_image_accepts_only_tile_zero() {
        assert!(uv_coords_isect_udim(&UdimImage::Single, [0, 0], [0.5, 0.5]));
        assert!(!uv_coords_isect_udim(&UdimImage::Single, [0, 0], [1.5, 0.5]));
        assert!(!uv_coords_isect_udim(&UdimImage::Single, [0, 0], [-0.5, 0.5]));
    }
}
