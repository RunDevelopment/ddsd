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

    let indexes = if options.dither {
        get_inter6_indexes_dither_ordered_diffused(&block, endpoints.c0_f, endpoints.c1_f)
    } else {
        get_inter6_indexes(&block, endpoints.c0_f, endpoints.c1_f)
    };
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
    for (pixel_index, &pixel) in block.iter().enumerate() {
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
        set_pixel_index(&mut indexes, pixel_index, index);
    }

    indexes
}
const BAYER_4: [u8; 16] = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];
const BAYER_4_N: [f32; 16] = {
    let mut bayer = [0.0; 16];
    let mut i = 0;
    while i < 16 {
        bayer[i] = BAYER_4[i] as f32 / 16.0;
        i += 1;
    }
    bayer
};
fn get_inter6_indexes_dither_ordered(block: &[f32; 16], c0: f32, c1: f32) -> u64 {
    let index_map: [u8; 8] = [1, 7, 6, 5, 4, 3, 2, 0];

    let mut indexes = 0_u64;
    for (pixel_index, &pixel) in block.iter().enumerate() {
        let blend = (pixel - c1) / (c0 - c1);
        debug_assert!(
            (-0.001..=1.001).contains(&blend),
            "blend {} for pixel {} c0 {} c1 {}",
            blend,
            pixel,
            c0,
            c1
        );
        let blend7 = ((blend * 7.0 + BAYER_4_N[pixel_index]) as u8).min(7);
        let index = index_map[blend7 as usize];
        set_pixel_index(&mut indexes, pixel_index, index);
    }

    indexes
}
fn get_inter6_indexes_dither_ordered_diffused(block: &[f32; 16], c0: f32, c1: f32) -> u64 {
    let index_map: [u8; 8] = [1, 7, 6, 5, 4, 3, 2, 0];

    // Pass 1: Score all pixels by their error
    let get_index_and_error = |value: f32| {
        let blend = (value - c1) / (c0 - c1);
        let blend7 = ((blend * 7.0 + 0.5) as u8).min(7);
        let index = index_map[blend7 as usize];
        let back = blend7 as f32 / 7.0 * (c0 - c1) + c1;
        let error = value - back;
        (index, error)
    };
    #[derive(Clone, Copy)]
    struct Record {
        pixel_index: u8,
        index: u8,
        error: f32,
    }
    let mut records: [Record; 16] = std::array::from_fn(|i| {
        // go through the pixels in random order
        let pixel_index = BAYER_4[i];
        let value = block[pixel_index as usize];
        let (index, error) = get_index_and_error(value);
        Record {
            pixel_index,
            index,
            error,
        }
    });

    // Pass 2: Separate pixels by negative and positive error
    records.sort_unstable_by(|a, b| {
        a.error
            .partial_cmp(&b.error)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let neg_count = records.iter().take_while(|r| r.error < 0.0).count();
    let (negative, positive) = records.split_at_mut(neg_count);
    negative.reverse();
    // Both negative and positive are sorted by ascending absolute error

    // Pass 3: Diffuse error with balancing
    // The basic idea here is that we want to trade positive with negative error
    let mut acc_error = 0.0;
    let mut indexes = 0;

    let mut neg_index = 0;
    let mut pos_index = 0;
    while neg_index < negative.len() && pos_index < positive.len() {
        let neg = negative[neg_index];
        let pos = positive[pos_index];

        let neg_error = (neg.error + acc_error).abs();
        let pos_error = (pos.error + acc_error).abs();
        if neg_error < pos_error {
            // Neg is closer to 0
            acc_error += neg.error;
            set_pixel_index(&mut indexes, neg.pixel_index as usize, neg.index);
            neg_index += 1;
        } else {
            // Pos is closer to 0
            acc_error += pos.error;
            set_pixel_index(&mut indexes, pos.pixel_index as usize, pos.index);
            pos_index += 1;
        }
    }

    // Pass 4: Diffuse error
    let rest = if neg_index < negative.len() {
        &negative[neg_index..]
    } else {
        &positive[pos_index..]
    };

    for record in rest {
        let pixel_index = record.pixel_index as usize;
        let pixel = block[pixel_index];
        let blend = (pixel - c1) / (c0 - c1);
        let blend7 = ((blend * 7.0 + BAYER_4_N[pixel_index]) as u8).min(7);
        let index = index_map[blend7 as usize];
        set_pixel_index(&mut indexes, pixel_index, index);
    }

    indexes
}

fn set_pixel_index(indexes: &mut u64, pixel_index: usize, index: u8) {
    debug_assert!(index < 8);
    debug_assert!(pixel_index < 16);
    *indexes |= (index as u64) << (pixel_index * 3);
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
