use crate::n8;

#[derive(Debug, Clone, Copy)]
pub struct Bc4Options {
    pub dither: bool,
}

/// The smallest non-zero value that can be represented in a BC4 block.
///
/// This is also the smallest distance of 2 adjacent representable values.
const BC4_MIN_VALUE: f32 = 1. / (255. * 7.);

pub(crate) fn compress_bc4u_block(mut block: [f32; 16], options: Bc4Options) -> [u8; 8] {
    // clamp to 0-1
    block.iter_mut().for_each(|x| *x = x.clamp(0.0, 1.0));

    let mut min = block[0];
    let mut max = block[0];
    for value in block {
        min = min.min(value);
        max = max.max(value);
    }

    // round down c0 and round up c1
    let mut min_u8 = (255.0 * min) as u8;
    let mut max_u8 = 255 - (255.0 * (1.0 - max)) as u8;
    if min_u8 == max_u8 {
        if min_u8 == 0 {
            max_u8 = 1;
        } else {
            min_u8 -= 1;
        }
    }
    debug_assert!(min_u8 < max_u8);

    // This uses the path for 6 interpolated colors
    let c0 = max_u8;
    let c1 = min_u8;

    let c0f = n8::f32(c0);
    let c1f = n8::f32(c1);
    debug_assert!(c0f > c1f);
    let dist = c0f - c1f;

    let index_map: [u8; 8] = [1, 7, 6, 5, 4, 3, 2, 0];

    let mut indexes = 0_u64;
    for &pixel in block.iter().rev() {
        let blend = (pixel - c1f) / dist;
        debug_assert!((0.0..=1.0).contains(&blend));
        let blend7 = ((blend * 7.0 + 0.5) as u8).min(7);
        let index = index_map[blend7 as usize];
        indexes = (indexes << 3) | index as u64;
    }

    let index_bytes = indexes.to_le_bytes();

    [
        c0,
        c1,
        index_bytes[0],
        index_bytes[1],
        index_bytes[2],
        index_bytes[3],
        index_bytes[4],
        index_bytes[5],
        // index_bytes[6],
        // index_bytes[7],
    ]
}
