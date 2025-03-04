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
/// 2 values that are this close will be considered equal.
const BC4_EPSILON: f32 = 1. / (65536.);

pub(crate) fn compress_bc4_block(mut block: [f32; 16], options: Bc4Options) -> [u8; 8] {
    // clamp to 0-1
    block.iter_mut().for_each(|x| *x = x.clamp(0.0, 1.0));

    let mut min = block[0];
    let mut max = block[0];
    for value in block {
        min = min.min(value);
        max = max.max(value);
    }
    let diff = max - min;

    // single color
    if diff < BC4_EPSILON {
        // See if the closest encoded value is good enough. This doesn't improve
        // quality, but it does make the output more compressable for gzip.
        let value = (min + max) * 0.5;
        let closest = EndPoints::new_closest(value, options.snorm);
        if (closest.c0_f - value).abs() < BC4_EPSILON {
            return closest.with_indexes(0);
        }
    }

    // If the colors are far away from 0 and 1, then inter6 is always better
    // than inter4
    const INTER6_THRESHOLD: f32 = 1. / 7.;
    if 0. < min - diff * INTER6_THRESHOLD && max + diff * INTER6_THRESHOLD < 1. {
        return compress_inter6(&block, min, max, options).0;
    }

    // Encode with both inter6 and inter4 and pick the best
    let (inter6, error6) = compress_inter6(&block, min, max, options);
    let (inter4, error4) = compress_inter4(&block, options);

    if error6 < error4 {
        inter6
    } else {
        inter4
    }
}

fn compress_inter6(block: &[f32; 16], min: f32, max: f32, options: Bc4Options) -> ([u8; 8], f32) {
    let endpoints = EndPoints::new_inter6(min, max, options.snorm);

    let (indexes, error) = if options.dither {
        get_inter6_indexes_dither_ordered_diffused(block, endpoints.c0_f, endpoints.c1_f)
    } else {
        get_inter6_indexes(block, endpoints.c0_f, endpoints.c1_f)
    };

    (endpoints.with_indexes(indexes), error)
}

fn get_inter6_indexes(block: &[f32; 16], c0: f32, c1: f32) -> (u64, f32) {
    let index_map: [u8; 8] = [1, 7, 6, 5, 4, 3, 2, 0];

    let mut total_error = 0.0;
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

        let back = blend7 as f32 / 7.0 * (c0 - c1) + c1;
        let error = pixel - back;
        total_error += error * error;
    }

    (indexes, total_error)
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
fn get_inter6_indexes_dither_ordered(block: &[f32; 16], c0: f32, c1: f32) -> (u64, f32) {
    let index_map: [u8; 8] = [1, 7, 6, 5, 4, 3, 2, 0];

    let mut total_error = 0.0;
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

        let back = blend7 as f32 / 7.0 * (c0 - c1) + c1;
        let error = pixel - back;
        total_error += error * error;
    }

    (indexes, total_error)
}
fn get_inter6_indexes_dither_ordered_diffused(block: &[f32; 16], c0: f32, c1: f32) -> (u64, f32) {
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
    let mut total_error = 0.0;

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
            total_error += neg.error * neg.error;
        } else {
            // Pos is closer to 0
            acc_error += pos.error;
            set_pixel_index(&mut indexes, pos.pixel_index as usize, pos.index);
            pos_index += 1;
            total_error += pos.error * pos.error;
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

        let back = blend7 as f32 / 7.0 * (c0 - c1) + c1;
        let error = pixel - back;
        total_error += error * error;
    }

    (indexes, total_error)
}

fn compress_inter4(block: &[f32; 16], options: Bc4Options) -> ([u8; 8], f32) {
    let mut min: f32 = 1.0;
    let mut max: f32 = 0.0;
    for &value in block {
        if value > BC4_MIN_VALUE {
            min = min.min(value);
        }
        if value < 1.0 - BC4_MIN_VALUE {
            max = max.max(value);
        }
    }

    let endpoints = EndPoints::new_inter4(min, max, options.snorm);
    let c0 = endpoints.c0_f;
    let c1 = endpoints.c1_f;
    let palette = [
        c0,
        c1,
        c0 * 0.8 + c1 * 0.2,
        c0 * 0.6 + c1 * 0.4,
        c0 * 0.4 + c1 * 0.6,
        c0 * 0.2 + c1 * 0.8,
        0.0,
        1.0,
    ];

    let (indexes, error) = if options.dither {
        get_indexes_dither_palette(block, palette)
    } else {
        get_inter4_indexes(block, palette)
    };

    (endpoints.with_indexes(indexes), error)
}

fn get_inter4_indexes(block: &[f32; 16], palette: [f32; 8]) -> (u64, f32) {
    let mut total_error = 0.0;

    let mut indexes = 0_u64;
    for (pixel_index, &pixel) in block.iter().enumerate() {
        // this handles the case where pixel is 0 or 1
        let (mut index, mut min_error) = if pixel > 0.5 {
            (7_u8, 1.0 - pixel)
        } else {
            (6_u8, pixel)
        };

        // for the rest, check the palette
        #[allow(clippy::needless_range_loop)]
        for i in 0..6 {
            let error = (pixel - palette[i]).abs();
            if error < min_error {
                min_error = error;
                index = i as u8;
            }
        }

        set_pixel_index(&mut indexes, pixel_index, index);
        total_error += min_error * min_error;
    }

    (indexes, total_error)
}
fn get_indexes_dither_palette(block: &[f32; 16], palette: [f32; 8]) -> (u64, f32) {
    fn find_with_min_error(value: f32, palette: &[f32; 8]) -> (u8, f32) {
        let mut index: u8 = 0;
        let mut min_error = value - palette[0];
        for i in 1..8 {
            let error = value - palette[i];
            if error.abs() < min_error.abs() {
                min_error = error;
                index = i as u8;
            }
        }
        (index, min_error)
    }

    // Pass 1: Score all pixels by their error
    let get_index_and_error = |value: f32| {
        find_with_min_error(value, &palette)
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
    let mut total_error = 0.0;

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
            total_error += neg.error * neg.error;
        } else {
            // Pos is closer to 0
            acc_error += pos.error;
            set_pixel_index(&mut indexes, pos.pixel_index as usize, pos.index);
            pos_index += 1;
            total_error += pos.error * pos.error;
        }
    }

    // Pass 4: Diffuse error
    let rest = if neg_index < negative.len() {
        &negative[neg_index..]
    } else {
        &positive[pos_index..]
    };
    let mut processed: u16 = 0;
    for record in rest {
        processed |= 1 << record.pixel_index;
    }

    const HILBERT: [u8; 16] = [0, 1, 5, 4, 8, 12, 13, 9, 10, 14, 15, 11, 7, 6, 2, 3];
    let mut diffuse_error = acc_error;
    for pixel_index in HILBERT {
        if processed & (1 << pixel_index) == 0 {
            continue;
        }

        let value = block[pixel_index as usize] + diffuse_error;
        let (index, min_error) = get_index_and_error(value);
        diffuse_error = min_error;

        set_pixel_index(&mut indexes, pixel_index as usize, index);

        total_error += min_error * min_error;
    }

    (indexes, total_error)
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
    /// Creates a new endpoint pair for a BC4 block.
    /// C0 will be the value closest to the given value and C1_f will be 0.
    fn new_closest(value: f32, snorm: bool) -> Self {
        if snorm {
            let closest_s8_norm = (254.0 * value + 0.5) as u8;

            let c0 = s8::from_norm(closest_s8_norm);
            let c1 = s8::from_norm(0);
            let c0_f = s8::uf32(c0);
            let c1_f = s8::uf32(c1);
            debug_assert!(c1_f == 0.0);

            Self { c0, c1, c0_f, c1_f }
        } else {
            // round down min and round up max
            let closest = (255.0 * value + 0.5) as u8;

            let c0 = closest;
            let c1 = 0;
            let c0_f = n8::f32(c0);
            let c1_f = 0.0;

            Self { c0, c1, c0_f, c1_f }
        }
    }
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
    fn new_inter4(e0: f32, e1: f32, snorm: bool) -> Self {
        let mut inter6 = Self::new_inter6(e0, e1, snorm);
        std::mem::swap(&mut inter6.c0, &mut inter6.c1);
        std::mem::swap(&mut inter6.c0_f, &mut inter6.c1_f);
        inter6
    }

    fn with_indexes(&self, indexes: u64) -> [u8; 8] {
        let index_bytes = indexes.to_le_bytes();

        [
            self.c0,
            self.c1,
            index_bytes[0],
            index_bytes[1],
            index_bytes[2],
            index_bytes[3],
            index_bytes[4],
            index_bytes[5],
        ]
    }
}
