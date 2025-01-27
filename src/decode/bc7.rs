#[cold]
#[inline]
fn unlikely_branch() {}

pub(crate) fn decode_bc7_block(block: [u8; 16]) -> [[u8; 4]; 16] {
    let mut stream = BitStream::new(block);
    // initialize the output to all 0s, aka transparent black
    let mut output = [[0_u8; 4]; 16];

    let mode = extract_mode(&mut stream);

    // This sort of static dispatch is necessary for performance, as it enables
    // the compiler to vectorize the actual pixel interpolation loop, which is
    // the most performance-critical part of the decoding process.

    match mode {
        0 => mode_subset_3::<0>(&mut output, stream),
        1 => mode_subset_2::<1>(&mut output, stream),
        2 => mode_subset_3::<2>(&mut output, stream),
        3 => mode_subset_2::<3>(&mut output, stream),
        4 => mode_4(&mut output, stream),
        5 => mode_5(&mut output, stream),
        6 => mode_6(&mut output, stream),
        7 => mode_subset_2::<7>(&mut output, stream),
        8.. => {
            unlikely_branch();
            // To quote the spec: Mode 8 (LSB 0x00) is reserved and should not
            // be used by the encoder. If this mode is given to the hardware,
            // an all 0 block will be returned.
        }
    }

    output
}

#[inline(always)]
fn mode_subset_2<const MODE: u8>(output: &mut [[u8; 4]; 16], mut stream: BitStream) {
    debug_assert!(MODE == 1 || MODE == 3 || MODE == 7);

    let partition_set_id = extract_partition_set_id(MODE, &mut stream);
    let subset_index_map = PARTITION_SET_2[partition_set_id as usize];

    // get fully decoded endpoints
    let [color0_s0, color1_s0, color0_s1, color1_s1] = get_end_points_4(MODE, &mut stream);

    let index_bits = match MODE {
        1 => 3,
        3 | 7 => 2,
        _ => unreachable!(),
    };

    let indexes = Indexes::new_p2(index_bits, &mut stream, subset_index_map.fixup_index_2);

    for pixel_index in 0..16 {
        let subset_index = subset_index_map.get_subset_index(pixel_index);

        // an `if` turns out to be faster than indexing here
        let [color0, color1] = if subset_index == 0 {
            [color0_s0, color1_s0]
        } else {
            [color0_s1, color1_s1]
        };

        let index = indexes.get_index(pixel_index);

        let r = interpolate_2_or_3(color0[0], color1[0], index, index_bits);
        let g = interpolate_2_or_3(color0[1], color1[1], index, index_bits);
        let b = interpolate_2_or_3(color0[2], color1[2], index, index_bits);
        let a = interpolate_2_or_3(color0[3], color1[3], index, index_bits);

        output[pixel_index as usize] = [r, g, b, a];
    }
}
#[inline(always)]
fn mode_subset_3<const MODE: u8>(output: &mut [[u8; 4]; 16], mut stream: BitStream) {
    debug_assert!(MODE == 0 || MODE == 2);

    let partition_set_id = extract_partition_set_id(MODE, &mut stream);
    let subset_index_map = PARTITION_SET_3[partition_set_id as usize];

    // get fully decoded endpoints
    let endpoints = get_end_points_6(MODE, &mut stream);

    let index_bits = match MODE {
        0 => 3,
        2 => 2,
        _ => unreachable!(),
    };

    let indexes = Indexes::new_p3(
        index_bits,
        &mut stream,
        subset_index_map.fixup_index_2,
        subset_index_map.fixup_index_3,
    );

    for pixel_index in 0..16 {
        // The `.min(2)` allows LLVM to prove that bounds checks are unnecessary
        let subset_index = subset_index_map.get_subset_index(pixel_index).min(2);

        // endpoints are now complete.
        let endpoint_start = endpoints[2 * subset_index as usize];
        let endpoint_end = endpoints[2 * subset_index as usize + 1];

        let index = indexes.get_index(pixel_index);

        let r = interpolate_2_or_3(endpoint_start[0], endpoint_end[0], index, index_bits);
        let g = interpolate_2_or_3(endpoint_start[1], endpoint_end[1], index, index_bits);
        let b = interpolate_2_or_3(endpoint_start[2], endpoint_end[2], index, index_bits);
        let a = interpolate_2_or_3(endpoint_start[3], endpoint_end[3], index, index_bits);

        output[pixel_index as usize] = [r, g, b, a];
    }
}
fn mode_4(output: &mut [[u8; 4]; 16], mut stream: BitStream) {
    // rotation and index mode as one read
    let rotation_and_index_mode = stream.consume_bits(3);
    // extract rotation bits
    let rotation = rotation_and_index_mode & 0b11;
    // index mode
    let index_mode = rotation_and_index_mode & 0b100 != 0;

    // get fully decoded endpoints
    let [color0, color1] = get_end_points_2(4, &mut stream);

    let color_indexes = Indexes::new_p1(2, &mut stream);
    let alpha_indexes = Indexes::new_p1(3, &mut stream);

    for pixel_index in 0..16 {
        let color_index = color_indexes.get_index(pixel_index);
        let alpha_index = alpha_indexes.get_index(pixel_index);

        let mut color_weight = get_weight_2(color_index);
        let mut alpha_weight = get_weight_3(alpha_index);

        if index_mode {
            std::mem::swap(&mut color_weight, &mut alpha_weight);
        }

        output[pixel_index as usize] =
            interpolate_colors_alpha(color0, color1, color_weight, alpha_weight);
    }

    swap_channels(output, rotation);
}
fn mode_5(output: &mut [[u8; 4]; 16], mut stream: BitStream) {
    let rotation = stream.consume_bits(2);

    // get fully decoded endpoints
    let [color0, color1] = get_end_points_2(5, &mut stream);

    let color_indexes = Indexes::new_p1(2, &mut stream);
    let alpha_indexes = Indexes::new_p1(2, &mut stream);

    for pixel_index in 0..16 {
        let color_index = color_indexes.get_index(pixel_index);
        let alpha_index = alpha_indexes.get_index(pixel_index);

        let color_weight = get_weight_2(color_index);
        let alpha_weight = get_weight_2(alpha_index);

        output[pixel_index as usize] =
            interpolate_colors_alpha(color0, color1, color_weight, alpha_weight);
    }

    swap_channels(output, rotation);
}
fn mode_6(output: &mut [[u8; 4]; 16], mut stream: BitStream) {
    // get fully decoded endpoints
    let [color0, color1] = get_end_points_2(6, &mut stream);

    let indexes = Indexes::new_p1(4, &mut stream);

    for pixel_index in 0..16 {
        let index = indexes.get_index(pixel_index);
        let weight = get_weight_4(index);
        output[pixel_index as usize] = interpolate_colors(color0, color1, weight);
    }
}

fn extract_mode(stream: &mut BitStream) -> u8 {
    // instead of doing it in a loopty loop, just count trailing zeros
    let mode = stream.low_u8().trailing_zeros() as u8;
    stream.skip(mode + 1);
    mode
}
fn extract_partition_set_id(mode: u8, stream: &mut BitStream) -> u8 {
    debug_assert!(matches!(mode, 0 | 1 | 2 | 3 | 7));
    let bits = if mode == 0 { 4 } else { 6 };
    stream.consume_bits(bits)
}

#[inline]
fn promote(mut number: u8, number_bits: u8) -> u8 {
    debug_assert!((4..8).contains(&number_bits));
    number <<= 8 - number_bits;
    number |= number >> number_bits;
    number
}
#[inline(always)]
fn get_end_points_2(mode: u8, stream: &mut BitStream) -> [[u8; 4]; 2] {
    #![allow(clippy::needless_range_loop)]
    let mut output: [[u8; 4]; 2] = Default::default();

    match mode {
        4 => {
            let r = [0_u8; 2].map(|_| stream.consume_bits(5));
            let g = [0_u8; 2].map(|_| stream.consume_bits(5));
            let b = [0_u8; 2].map(|_| stream.consume_bits(5));
            let a = [0_u8; 2].map(|_| stream.consume_bits(6));

            for i in 0..2 {
                output[i] = [
                    promote(r[i], 5),
                    promote(g[i], 5),
                    promote(b[i], 5),
                    promote(a[i], 6),
                ];
            }
        }
        5 => {
            let r = [0_u8; 2].map(|_| stream.consume_bits(7));
            let g = [0_u8; 2].map(|_| stream.consume_bits(7));
            let b = [0_u8; 2].map(|_| stream.consume_bits(7));
            let a = [0_u8; 2].map(|_| stream.consume_bits(8));

            for i in 0..2 {
                output[i] = [promote(r[i], 7), promote(g[i], 7), promote(b[i], 7), a[i]];
            }
        }
        6 => {
            let mut r = [0_u8; 2].map(|_| stream.consume_bits(7));
            let mut g = [0_u8; 2].map(|_| stream.consume_bits(7));
            let mut b = [0_u8; 2].map(|_| stream.consume_bits(7));
            let mut a = [0_u8; 2].map(|_| stream.consume_bits(7));

            // each endpoint has its own p
            for i in 0..2 {
                let p = stream.consume_bit() as u8;
                r[i] = r[i] << 1 | p;
                g[i] = g[i] << 1 | p;
                b[i] = b[i] << 1 | p;
                a[i] = a[i] << 1 | p;
            }

            for i in 0..2 {
                output[i] = [r[i], g[i], b[i], a[i]];
            }
        }
        _ => unreachable!(),
    };

    output
}
#[inline(always)]
fn get_end_points_4(mode: u8, stream: &mut BitStream) -> [[u8; 4]; 4] {
    let mut output: [[u8; 4]; 4] = Default::default();

    match mode {
        1 => {
            let mut r = [0_u8; 4].map(|_| stream.consume_bits(6));
            let mut g = [0_u8; 4].map(|_| stream.consume_bits(6));
            let mut b = [0_u8; 4].map(|_| stream.consume_bits(6));

            // p is shared between endpoints of the same subset
            for i in 0..2 {
                let p = stream.consume_bit() as u8;
                let i0 = i * 2;
                let i1 = i0 + 1;
                r[i0] = r[i0] << 1 | p;
                g[i0] = g[i0] << 1 | p;
                b[i0] = b[i0] << 1 | p;
                r[i1] = r[i1] << 1 | p;
                g[i1] = g[i1] << 1 | p;
                b[i1] = b[i1] << 1 | p;
            }

            for i in 0..4 {
                output[i] = [promote(r[i], 7), promote(g[i], 7), promote(b[i], 7), 255];
            }
        }
        3 => {
            let mut r = [0_u8; 4].map(|_| stream.consume_bits(7));
            let mut g = [0_u8; 4].map(|_| stream.consume_bits(7));
            let mut b = [0_u8; 4].map(|_| stream.consume_bits(7));

            // each endpoint has its own p
            for i in 0..4 {
                let p = stream.consume_bit() as u8;
                r[i] = r[i] << 1 | p;
                g[i] = g[i] << 1 | p;
                b[i] = b[i] << 1 | p;
            }

            for i in 0..4 {
                output[i] = [r[i], g[i], b[i], 255];
            }
        }
        7 => {
            let mut r = [0_u8; 4].map(|_| stream.consume_bits(5));
            let mut g = [0_u8; 4].map(|_| stream.consume_bits(5));
            let mut b = [0_u8; 4].map(|_| stream.consume_bits(5));
            let mut a = [0_u8; 4].map(|_| stream.consume_bits(5));

            // each endpoint has its own p
            for i in 0..4 {
                let p = stream.consume_bit() as u8;
                r[i] = r[i] << 1 | p;
                g[i] = g[i] << 1 | p;
                b[i] = b[i] << 1 | p;
                a[i] = a[i] << 1 | p;
            }

            for i in 0..4 {
                output[i] = [
                    promote(r[i], 6),
                    promote(g[i], 6),
                    promote(b[i], 6),
                    promote(a[i], 6),
                ];
            }
        }
        _ => unreachable!(),
    };

    output
}
#[inline(always)]
fn get_end_points_6(mode: u8, stream: &mut BitStream) -> [[u8; 4]; 6] {
    let mut output: [[u8; 4]; 6] = Default::default();

    match mode {
        0 => {
            let mut r = [0_u8; 6].map(|_| stream.consume_bits(4));
            let mut g = [0_u8; 6].map(|_| stream.consume_bits(4));
            let mut b = [0_u8; 6].map(|_| stream.consume_bits(4));

            // each endpoint has its own p
            for i in 0..6 {
                let p = stream.consume_bit() as u8;
                r[i] = r[i] << 1 | p;
                g[i] = g[i] << 1 | p;
                b[i] = b[i] << 1 | p;
            }

            for i in 0..6 {
                output[i] = [promote(r[i], 5), promote(g[i], 5), promote(b[i], 5), 255];
            }
        }
        2 => {
            let r = [0_u8; 6].map(|_| stream.consume_bits(5));
            let g = [0_u8; 6].map(|_| stream.consume_bits(5));
            let b = [0_u8; 6].map(|_| stream.consume_bits(5));

            for i in 0..6 {
                output[i] = [promote(r[i], 5), promote(g[i], 5), promote(b[i], 5), 255];
            }
        }
        _ => unreachable!(),
    };

    output
}

fn swap_channels(pixels: &mut [[u8; 4]; 16], rotation: u8) {
    // Decode the 2 color rotation bits as follows:
    // 00 - Block format is Scalar(A) Vector(RGB) - no swapping
    // 01 - Block format is Scalar(R) Vector(AGB) - swap A and R
    // 10 - Block format is Scalar(G) Vector(RAB) - swap A and G
    // 11 - Block format is Scalar(B) Vector(RGA) - swap A and B
    match rotation {
        1 => pixels.iter_mut().for_each(|p| p.swap(0, 3)),
        2 => pixels.iter_mut().for_each(|p| p.swap(1, 3)),
        3 => pixels.iter_mut().for_each(|p| p.swap(2, 3)),
        _ => {}
    };
}

// Weights are all multiplied by 4 compared to the original ones. This changes
// the interpolation formula from
//   ((64-w)*e0 + w*e1 + 32) >> 6
// to
//   ((256-w)*e0 + w*e1 + 128) >> 8
// The nice thing about this is that intermediate results still fit into u16,
// but the compiler can optimize away the `>> 8`.
const WEIGHTS_2: [u16; 4] = [0, 84, 172, 256];
const WEIGHTS_3: [u16; 8] = [0, 36, 72, 108, 148, 184, 220, 256];
const WEIGHTS_4: [u16; 16] = [
    0, 16, 36, 52, 68, 84, 104, 120, 136, 152, 172, 188, 204, 220, 240, 256,
];

fn interpolate_2_or_3(e0: u8, e1: u8, index: u8, index_bits: u8) -> u8 {
    let weight = match index_bits {
        2 => WEIGHTS_2[index as usize],
        3 => WEIGHTS_3[index as usize],
        _ => unreachable!(),
    };
    let w0 = 256 - weight;
    let w1 = weight;
    ((w0 * e0 as u16 + w1 * e1 as u16 + 128) >> 8) as u8
}

#[inline]
fn get_weight_4(index: u8) -> u16 {
    WEIGHTS_4[index as usize]
}
#[inline]
fn get_weight_3(index: u8) -> u16 {
    WEIGHTS_3[index as usize]
}
#[inline]
fn get_weight_2(index: u8) -> u16 {
    WEIGHTS_2[index as usize]
}
#[inline]
fn interpolate_colors(color0: [u8; 4], color1: [u8; 4], weight: u16) -> [u8; 4] {
    let w0 = 256 - weight;
    let w1 = weight;
    [
        ((w0 * color0[0] as u16 + w1 * color1[0] as u16 + 128) >> 8) as u8,
        ((w0 * color0[1] as u16 + w1 * color1[1] as u16 + 128) >> 8) as u8,
        ((w0 * color0[2] as u16 + w1 * color1[2] as u16 + 128) >> 8) as u8,
        ((w0 * color0[3] as u16 + w1 * color1[3] as u16 + 128) >> 8) as u8,
    ]
}
#[inline]
fn interpolate_colors_alpha(
    color0: [u8; 4],
    color1: [u8; 4],
    color_weight: u16,
    alpha_weight: u16,
) -> [u8; 4] {
    let wc0 = 256 - color_weight;
    let wc1 = color_weight;
    let wa0 = 256 - alpha_weight;
    let wa1 = alpha_weight;
    [
        ((wc0 * color0[0] as u16 + wc1 * color1[0] as u16 + 128) >> 8) as u8,
        ((wc0 * color0[1] as u16 + wc1 * color1[1] as u16 + 128) >> 8) as u8,
        ((wc0 * color0[2] as u16 + wc1 * color1[2] as u16 + 128) >> 8) as u8,
        ((wa0 * color0[3] as u16 + wa1 * color1[3] as u16 + 128) >> 8) as u8,
    ]
}

/// Stores the subset indexes for BC7 modes with 2 subsets.
///
/// Since each subset index is either 0 or 1, they are stored as the bits of
/// u16.
///
/// `fixup_index_2` is the second fixup index. The first fixup index is always 0.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Subset2Map {
    subset_indexes: u16,
    fixup_index_2: u8,
}
impl Subset2Map {
    const fn get_subset_index(self, pixel_index: u8) -> u8 {
        (self.subset_indexes.wrapping_shr(pixel_index as u32) & 0b1) as u8
    }
}
/// Stores the subset indexes for BC7 modes with 3 subsets.
///
/// Since each subset index is either 0, 1 or 2, they are stored as 2 bits in
/// a u32.
///
/// `fixup_index_2` and `fixup_index_3` are the second and third fixup index
/// respectively. The first fixup index is always 0.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Subset3Map {
    subset_indexes: u32,
    fixup_index_2: u8,
    fixup_index_3: u8,
}
impl Subset3Map {
    const fn get_subset_index(self, pixel_index: u8) -> u8 {
        (self.subset_indexes.wrapping_shr(pixel_index as u32 * 2) & 0b11) as u8
    }
}

const fn subset2(data: [u8; 17]) -> Subset2Map {
    let mut output_p2: u16 = 0;
    let mut fixup_index_2 = 0;

    let mut pixel_index = 0;
    let mut data_index = 0;
    while data_index < data.len() {
        let d = data[data_index];
        data_index += 1;

        if d == b'-' {
            fixup_index_2 = pixel_index;
        } else {
            let d = (d - b'0') as u32;
            assert!(d <= 1);
            output_p2 |= (d as u16) << pixel_index;
            pixel_index += 1;
        }
    }
    assert!(pixel_index == 16);
    assert!(fixup_index_2 != 0);

    let result = Subset2Map {
        subset_indexes: output_p2,
        fixup_index_2,
    };

    // the first subset index is always 0
    assert!(result.get_subset_index(0) == 0);

    result
}
const fn subset3(data: [u8; 18]) -> Subset3Map {
    let mut output: u32 = 0;
    let mut fixup_index_2 = 0;
    let mut fixup_index_3 = 0;

    let mut pixel_index = 0;
    let mut data_index = 0;
    while data_index < data.len() {
        let d = data[data_index];
        data_index += 1;

        if d == b'-' {
            if fixup_index_2 == 0 {
                fixup_index_2 = pixel_index;
            } else {
                fixup_index_3 = pixel_index;
            }
        } else {
            let d = (d - b'0') as u32;
            assert!(d <= 2);
            output |= d << (pixel_index * 2);
            pixel_index += 1;
        }
    }
    assert!(pixel_index == 16);
    assert!(fixup_index_2 != 0);
    assert!(fixup_index_3 != 0);

    let result = Subset3Map {
        subset_indexes: output,
        fixup_index_2,
        fixup_index_3,
    };

    // the first subset index is always 0
    assert!(result.get_subset_index(0) == 0);

    result
}

const PARTITION_SET_2: [Subset2Map; 64] = [
    // 0
    subset2(*b"001100110011001-1"),
    subset2(*b"000100010001000-1"),
    subset2(*b"011101110111011-1"),
    subset2(*b"000100110011011-1"),
    subset2(*b"000000010001001-1"),
    subset2(*b"001101110111111-1"),
    subset2(*b"000100110111111-1"),
    subset2(*b"000000010011011-1"),
    subset2(*b"000000000001001-1"),
    subset2(*b"001101111111111-1"),
    subset2(*b"000000010111111-1"),
    subset2(*b"000000000001011-1"),
    subset2(*b"000101111111111-1"),
    subset2(*b"000000001111111-1"),
    subset2(*b"000011111111111-1"),
    subset2(*b"000000000000111-1"),
    // 16
    subset2(*b"000010001110111-1"),
    subset2(*b"01-11000100000000"),
    subset2(*b"00000000-10001110"),
    subset2(*b"01-11001100010000"),
    subset2(*b"00-11000100000000"),
    subset2(*b"00001000-11001110"),
    subset2(*b"00000000-10001100"),
    subset2(*b"011100110011000-1"),
    subset2(*b"00-11000100010000"),
    subset2(*b"00001000-10001100"),
    subset2(*b"01-10011001100110"),
    subset2(*b"00-11011001101100"),
    subset2(*b"00010111-11101000"),
    subset2(*b"00001111-11110000"),
    subset2(*b"01-11000110001110"),
    subset2(*b"00-11100110011100"),
    // 32
    subset2(*b"010101010101010-1"),
    subset2(*b"000011110000111-1"),
    subset2(*b"010110-1001011010"),
    subset2(*b"00110011-11001100"),
    subset2(*b"00-11110000111100"),
    subset2(*b"01010101-10101010"),
    subset2(*b"011010010110100-1"),
    subset2(*b"010110101010010-1"),
    subset2(*b"01-11001111001110"),
    subset2(*b"00010011-11001000"),
    subset2(*b"00-11001001001100"),
    subset2(*b"00-11101111011100"),
    subset2(*b"01-10100110010110"),
    subset2(*b"001111001100001-1"),
    subset2(*b"011001101001100-1"),
    subset2(*b"000001-1001100000"),
    // 48
    subset2(*b"010011-1001000000"),
    subset2(*b"00-10011100100000"),
    subset2(*b"000000-1001110010"),
    subset2(*b"00000100-11100100"),
    subset2(*b"011011001001001-1"),
    subset2(*b"001101101100100-1"),
    subset2(*b"01-10001110011100"),
    subset2(*b"00-11100111000110"),
    subset2(*b"011011001100100-1"),
    subset2(*b"011000110011100-1"),
    subset2(*b"011111101000000-1"),
    subset2(*b"000110001110011-1"),
    subset2(*b"000011110011001-1"),
    subset2(*b"00-11001111110000"),
    subset2(*b"00-10001011101110"),
    subset2(*b"010001000111011-1"),
];
const PARTITION_SET_3: [Subset3Map; 64] = [
    // 0
    subset3(*b"001-100110221222-2"),
    subset3(*b"000-10011-22112221"),
    subset3(*b"00002001-2211221-1"),
    subset3(*b"022-200220011011-1"),
    subset3(*b"00000000-1122112-2"),
    subset3(*b"001-100110022002-2"),
    subset3(*b"002-200221111111-1"),
    subset3(*b"00110011-2211221-1"),
    subset3(*b"00000000-1111222-2"),
    subset3(*b"00001111-1111222-2"),
    subset3(*b"000011-112222222-2"),
    subset3(*b"001200-120012001-2"),
    subset3(*b"011201-120112011-2"),
    subset3(*b"01220-1220122012-2"),
    subset3(*b"001-101121122122-2"),
    subset3(*b"001-12001-22002220"),
    // 16
    subset3(*b"000-100110112112-2"),
    subset3(*b"011-10011-20012200"),
    subset3(*b"00001122-1122112-2"),
    subset3(*b"002-200220022111-1"),
    subset3(*b"011-101110222022-2"),
    subset3(*b"000-10001-22212221"),
    subset3(*b"000000-110122012-2"),
    subset3(*b"00001100-22-102210"),
    subset3(*b"012-20-12200110000"),
    subset3(*b"00120012-1122222-2"),
    subset3(*b"011012-21-12210110"),
    subset3(*b"000001-1012-211221"),
    subset3(*b"00221102-1102002-2"),
    subset3(*b"01100-1102002222-2"),
    subset3(*b"0011012201-22001-1"),
    subset3(*b"00002000-2211222-1"),
    // 32
    subset3(*b"00000002-1122122-2"),
    subset3(*b"022-200220012001-1"),
    subset3(*b"001-100120022022-2"),
    subset3(*b"01200-12001-200120"),
    subset3(*b"000011-1122-220000"),
    subset3(*b"01201201-20-120120"),
    subset3(*b"01202012-1-2010120"),
    subset3(*b"0011220011-22001-1"),
    subset3(*b"001111-222200001-1"),
    subset3(*b"010-101012222222-2"),
    subset3(*b"00000000-2121212-1"),
    subset3(*b"00221-1220022112-2"),
    subset3(*b"002-200110022001-1"),
    subset3(*b"022012-210220122-1"),
    subset3(*b"010122-222222010-1"),
    subset3(*b"00002121-2121212-1"),
    // 48
    subset3(*b"010-101010101222-2"),
    subset3(*b"022-201110222011-1"),
    subset3(*b"00021-1120002111-2"),
    subset3(*b"00002-1122112211-2"),
    subset3(*b"02220-1110111022-2"),
    subset3(*b"00021112-1112000-2"),
    subset3(*b"01100-1100110222-2"),
    subset3(*b"0000000021-12211-2"),
    subset3(*b"01100-1102222222-2"),
    subset3(*b"0022001100-11002-2"),
    subset3(*b"00221122-1122002-2"),
    subset3(*b"0000000000002-11-2"),
    subset3(*b"000-200010002000-1"),
    subset3(*b"022212220222-122-2"),
    subset3(*b"010-122222222222-2"),
    subset3(*b"011-12011-22012220"),
];

struct BitStream {
    state: u128,
    #[cfg(debug_assertions)]
    consumed_bits: u8,
}
impl BitStream {
    pub fn new(block: [u8; 16]) -> Self {
        Self {
            state: u128::from_le_bytes(block),
            #[cfg(debug_assertions)]
            consumed_bits: 0,
        }
    }

    pub fn low_u8(&self) -> u8 {
        self.state as u8
    }

    #[inline(always)]
    pub fn skip(&mut self, n: u8) {
        self.state >>= n;
        #[cfg(debug_assertions)]
        {
            self.consumed_bits += n;
        }
    }

    #[inline]
    pub fn consume_bit(&mut self) -> bool {
        let bit = self.state as u8 & 1 != 0;
        self.skip(1);
        bit
    }

    #[inline]
    pub fn consume_bits(&mut self, count: u8) -> u8 {
        debug_assert!(0 < count && count <= 8);
        let mask = (1_u16 << count).wrapping_sub(1) as u8;
        let bits = self.state as u8 & mask;
        self.skip(count);
        bits
    }
    #[inline]
    pub fn consume_bits_64(&mut self, count: u8) -> u64 {
        debug_assert!(0 < count && count <= 64);
        let mut bits = self.state as u64;
        if count < 64 {
            bits &= (1_u64.wrapping_shl(count as u32)).wrapping_sub(1);
        }
        self.skip(count);
        bits
    }
}
#[cfg(debug_assertions)]
impl Drop for BitStream {
    fn drop(&mut self) {
        // Validate that we consumed all bits
        debug_assert_eq!(self.consumed_bits, 128);
    }
}

/// A list of uncompressed indexes.
///
/// BC7 uses compressed indexes. Instead of using a bit stream and uncompressed
/// the fix-up indexes an the fly, it's faster to decompress all indexes before
/// using them. This allows the compiler to more easily unroll the pixel
/// interpolation loops.
struct Indexes {
    uncompressed: u64,
    bits: u8,
    mask: u64,
}
impl Indexes {
    fn get_mask(bits: u8) -> u64 {
        (1 << bits) - 1
    }

    pub fn new_p1(bits: u8, stream: &mut BitStream) -> Self {
        Self::from_compressed_p1(bits, stream.consume_bits_64(16 * bits - 1))
    }
    pub fn new_p2(bits: u8, stream: &mut BitStream, p2_fixup: u8) -> Self {
        Self::from_compressed_p2(bits, stream.consume_bits_64(16 * bits - 2), p2_fixup)
    }
    pub fn new_p3(bits: u8, stream: &mut BitStream, p2_fixup: u8, p3_fixup: u8) -> Self {
        Self::from_compressed_p3(
            bits,
            stream.consume_bits_64(16 * bits - 3),
            p2_fixup,
            p3_fixup,
        )
    }
    pub fn from_compressed_p1(bits: u8, mut compressed: u64) -> Self {
        debug_assert!(bits <= 4);
        compressed = Self::decompress_single_index(bits, compressed, 0);
        Self {
            uncompressed: compressed,
            bits,
            mask: Self::get_mask(bits),
        }
    }
    pub fn from_compressed_p2(bits: u8, mut compressed: u64, p2_fixup: u8) -> Self {
        debug_assert!(bits <= 4);
        debug_assert!(0 < p2_fixup);
        compressed = Self::decompress_single_index(bits, compressed, 0);
        compressed = Self::decompress_single_index(bits, compressed, p2_fixup);
        Self {
            uncompressed: compressed,
            bits,
            mask: Self::get_mask(bits),
        }
    }
    pub fn from_compressed_p3(bits: u8, mut compressed: u64, p2_fixup: u8, p3_fixup: u8) -> Self {
        debug_assert!(bits <= 4);
        debug_assert!(0 < p2_fixup && p2_fixup < p3_fixup);
        compressed = Self::decompress_single_index(bits, compressed, 0);
        compressed = Self::decompress_single_index(bits, compressed, p2_fixup);
        compressed = Self::decompress_single_index(bits, compressed, p3_fixup);
        Self {
            uncompressed: compressed,
            bits,
            mask: Self::get_mask(bits),
        }
    }
    fn decompress_single_index(bits: u8, mut compressed: u64, index: u8) -> u64 {
        let mask = Self::get_mask(bits);

        let keep_count = index * bits;
        let keep = compressed & ((1 << keep_count) - 1);
        compressed >>= keep_count;
        compressed <<= 1;
        let first = compressed & mask;
        compressed = (compressed & !mask) | (first >> 1);
        compressed <<= keep_count;
        compressed |= keep;
        compressed
    }

    pub fn get_index(&self, pixel_index: u8) -> u8 {
        ((self.uncompressed >> (pixel_index * self.bits)) & self.mask) as u8
    }
}
