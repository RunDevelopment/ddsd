use crate::{n8, s8};

#[derive(Debug, Clone, Copy)]
pub struct Bc4Options {
    pub dither: bool,
    pub snorm: bool,
}

/// The smallest non-zero value that can be represented in a BC4 block.
///
/// This is also the smallest distance of 2 adjacent representable values.
const BC4_MIN_VALUE: f32 = 1. / (255. * 7.);

pub(crate) fn compress_bc4_block(mut block: [f32; 16], options: Bc4Options) -> [u8; 8] {
    // clamp to 0-1
    block.iter_mut().for_each(|x| *x = x.clamp(0.0, 1.0));

    let mut min = block[0];
    let mut max = block[0];
    for value in block {
        min = min.min(value);
        max = max.max(value);
    }

    // This uses the path for 6 interpolated colors
    let endpoints = EndPoints::new_inter6(min, max, options.snorm);
    let indexes = get_inter6_indexes(&block, endpoints.c0_f, endpoints.c1_f);
    let index_bytes = indexes.to_le_bytes();

    [
        endpoints.c0,
        endpoints.c1,
        index_bytes[0],
        index_bytes[1],
        index_bytes[2],
        index_bytes[3],
        index_bytes[4],
        index_bytes[5],
    ]
}

fn get_inter6_indexes(block: &[f32; 16], c0: f32, c1: f32) -> u64 {
    let index_map: [u8; 8] = [1, 7, 6, 5, 4, 3, 2, 0];

    let mut indexes = 0_u64;
    for &pixel in block.iter().rev() {
        let blend = (pixel - c1) / (c0 - c1);
        debug_assert!(
            (-0.001..=1.001).contains(&blend),
            "blend {} for pixel {} c0 {} c1 {}",
            blend,
            pixel,
            c0,
            c1
        );
        let blend7 = ((blend * 7.0 + 0.5) as u8).min(7);
        let index = index_map[blend7 as usize];
        indexes = (indexes << 3) | index as u64;
    }

    indexes
}

struct EndPoints {
    c0: u8,
    c1: u8,
    c0_f: f32,
    c1_f: f32,
}
impl EndPoints {
    fn new_inter6(e0: f32, e1: f32, snorm: bool) -> Self {
        let min = e0.min(e1);
        let max = e0.max(e1);

        // For the 6 interpolation mode, we need c0 > c1
        if snorm {
            // round down min and round up max
            let mut min_s8_norm = (254.0 * min) as u8;
            let mut max_s8_norm = 254 - (254.0 * (1.0 - max)) as u8;

            // make sure they are different
            if min_s8_norm == max_s8_norm {
                if min_s8_norm == 0 {
                    max_s8_norm = 1;
                } else {
                    min_s8_norm -= 1;
                }
            }
            debug_assert!(min_s8_norm < max_s8_norm);

            let mut c0 = s8::from_norm(max_s8_norm);
            let mut c1 = s8::from_norm(min_s8_norm);
            debug_assert!(c0 != c1);
            if c0 as i8 <= c1 as i8 {
                // swap
                std::mem::swap(&mut c0, &mut c1);
            }

            let c0_f = s8::uf32(c0);
            let c1_f = s8::uf32(c1);

            Self { c0, c1, c0_f, c1_f }
        } else {
            // round down min and round up max
            let mut min_u8 = (255.0 * min) as u8;
            let mut max_u8 = 255 - (255.0 * (1.0 - max)) as u8;

            // make sure they are different
            if min_u8 == max_u8 {
                if min_u8 == 0 {
                    max_u8 = 1;
                } else {
                    min_u8 -= 1;
                }
            }
            debug_assert!(min_u8 < max_u8);

            let c0 = max_u8;
            let c1 = min_u8;
            let c0_f = n8::f32(c0);
            let c1_f = n8::f32(c1);

            Self { c0, c1, c0_f, c1_f }
        }
    }
}
