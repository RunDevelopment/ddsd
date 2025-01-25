pub(crate) fn decode_bc7_block(block: [u8; 16]) -> [[u8; 4]; 16] {
    let mut stream = BitStream::new(block);
    // initialize the output to all 0s, aka transparent black
    let mut output = [[0_u8; 4]; 16];

    let mode = extract_mode(&mut stream);

    if mode >= 8 {
        // To quote the spec: Mode 8 (LSB 0x00) is reserved and should not
        // be used by the encoder. If this mode is given to the hardware,
        // an all 0 block will be returned.
        return output;
    }

    //decode partition data from explicit partition bits
    let mut subset_index = PARTITION_SET_1;
    let mut num_subsets = 1;

    if matches!(mode, 0 | 1 | 2 | 3 | 7) {
        num_subsets = get_num_subsets(mode);
        let partition_set_id = extract_partition_set_id(mode, &mut stream);
        subset_index = get_partition_index(num_subsets, partition_set_id);
    }

    // extract rotation bits for modes that have them
    let mut rotation = 0;
    if matches!(mode, 4 | 5) {
        rotation = stream.consume_bits(2);
    }

    // index mode for mode 4
    let mut index_mode = false;
    if mode == 4 {
        index_mode = stream.consume_bit();
    }

    // get fully decoded endpoints
    let endpoints = get_end_points(mode, &mut stream);

    // mode 4 and 5 store alpha indexes separately... fun.
    if matches!(mode, 4 | 5) {
        let color_index_bits = 2;
        let alpha_index_bits = if mode == 4 { 3 } else { 2 };

        debug_assert!(num_subsets == 1);
        debug_assert!(subset_index == PARTITION_SET_1);

        // pass 1: decode the color indexes
        // TODO: This can be heavily optimized, because mode 4&5 only have one
        // subset. This means that we statically know that only the first index
        // is the fix up index. This in turns allows us to do
        // `stream.consume_bits_u32(31) as u32` to "decompress" all color
        // indexes at once. The resulting u32 is the simply a list of the 16
        // 2-bit indexes.
        let mut color_indexes = [0_u8; 16];
        for pixel_index in 0..16 {
            let is_fix_up = subset_index.is_fix_up(pixel_index);
            let index = stream.consume_bits(color_index_bits - is_fix_up as u8);
            color_indexes[pixel_index as usize] = index;
        }

        // pass 2: decode the alpha indexes and interpolate
        for pixel_index in 0..16 {
            let (subset_index, is_fix_up) = subset_index.get_index(pixel_index);
            debug_assert!(subset_index == 0);

            let mut color_index = color_indexes[pixel_index as usize];
            let mut alpha_index = stream.consume_bits(alpha_index_bits - is_fix_up as u8);

            let mut color_bits = color_index_bits;
            let mut alpha_bits = alpha_index_bits;

            if mode == 4 && index_mode {
                std::mem::swap(&mut color_index, &mut alpha_index);
                std::mem::swap(&mut color_bits, &mut alpha_bits);
            }

            // endpoints are now complete.
            let endpoint_start = endpoints[0];
            let endpoint_end = endpoints[1];

            let r = interpolate(endpoint_start[0], endpoint_end[0], color_index, color_bits);
            let g = interpolate(endpoint_start[1], endpoint_end[1], color_index, color_bits);
            let b = interpolate(endpoint_start[2], endpoint_end[2], color_index, color_bits);
            let a = interpolate(endpoint_start[3], endpoint_end[3], alpha_index, alpha_bits);

            output[pixel_index as usize] = [r, g, b, a];
        }
    } else {
        let index_bits = match mode {
            0 => 3,
            1 => 3,
            2 => 2,
            3 => 2,
            6 => 4,
            7 => 2,
            _ => unreachable!(),
        };

        for pixel_index in 0..16 {
            let (subset_index, is_fix_up) = subset_index.get_index(pixel_index);
            debug_assert!(subset_index < num_subsets);

            let index = stream.consume_bits(index_bits - is_fix_up as u8);

            // endpoints are now complete.
            let endpoint_start = endpoints[2 * subset_index as usize];
            let endpoint_end = endpoints[2 * subset_index as usize + 1];

            let r = interpolate(endpoint_start[0], endpoint_end[0], index, index_bits);
            let g = interpolate(endpoint_start[1], endpoint_end[1], index, index_bits);
            let b = interpolate(endpoint_start[2], endpoint_end[2], index, index_bits);
            let a = interpolate(endpoint_start[3], endpoint_end[3], index, index_bits);

            output[pixel_index as usize] = [r, g, b, a];
        }
    }

    if matches!(mode, 4 | 5) {
        // Decode the 2 color rotation bits as follows:
        // 00 - Block format is Scalar(A) Vector(RGB) - no swapping
        // 01 - Block format is Scalar(R) Vector(AGB) - swap A and R
        // 10 - Block format is Scalar(G) Vector(RAB) - swap A and G
        // 11 - Block format is Scalar(B) Vector(RGA) - swap A and B
        swap_channels(&mut output, rotation);
    }

    stream.validate();

    output
}
fn extract_mode(stream: &mut BitStream) -> u8 {
    // instead of doing it in a loopty loop, just count trailing zeros
    let mode = stream.low_u8().trailing_zeros() as u8;
    stream.skip(mode + 1);
    mode
}
fn get_num_subsets(mode: u8) -> u8 {
    match mode {
        0 => 3,
        1 => 2,
        2 => 3,
        3 => 2,
        4 => 1,
        5 => 1,
        6 => 1,
        7 => 2,
        _ => unreachable!(),
    }
}
fn extract_partition_set_id(mode: u8, stream: &mut BitStream) -> u8 {
    match mode {
        0 => stream.consume_bits(4),
        1 | 2 | 3 | 7 => stream.consume_bits(6),
        _ => unreachable!(),
    }
}
fn get_partition_index(num_subsets: u8, partition_set_id: u8) -> SubsetIndexMap {
    match num_subsets {
        1 => PARTITION_SET_1,
        2 => PARTITION_SET_2[partition_set_id as usize],
        3 => PARTITION_SET_3[partition_set_id as usize],
        _ => unreachable!(),
    }
}
fn get_end_points(mode: u8, stream: &mut BitStream) -> [[u8; 4]; 6] {
    #[inline]
    fn promote(mut number: u8, number_bits: u8) -> u8 {
        debug_assert!((4..8).contains(&number_bits));
        number <<= 8 - number_bits;
        number |= number >> number_bits;
        number
    }

    let mut output = [Default::default(); 6];

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
        2 => {
            let r = [0_u8; 6].map(|_| stream.consume_bits(5));
            let g = [0_u8; 6].map(|_| stream.consume_bits(5));
            let b = [0_u8; 6].map(|_| stream.consume_bits(5));

            for i in 0..6 {
                output[i] = [promote(r[i], 5), promote(g[i], 5), promote(b[i], 5), 255];
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
fn swap_channels(pixels: &mut [[u8; 4]; 16], rotation: u8) {
    match rotation {
        1 => pixels.iter_mut().for_each(|p| p.swap(0, 3)),
        2 => pixels.iter_mut().for_each(|p| p.swap(1, 3)),
        3 => pixels.iter_mut().for_each(|p| p.swap(2, 3)),
        _ => {}
    };
}

const WEIGHTS_2: [u16; 4] = [0, 21, 43, 64];
const WEIGHTS_3: [u16; 8] = [0, 9, 18, 27, 37, 46, 55, 64];
const WEIGHTS_4: [u16; 16] = [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64];
fn interpolate(e0: u8, e1: u8, index: u8, index_bits: u8) -> u8 {
    let weight = match index_bits {
        2 => WEIGHTS_2[index as usize],
        3 => WEIGHTS_3[index as usize],
        4 => WEIGHTS_4[index as usize],
        _ => unreachable!(),
    };
    (((64 - weight) * e0 as u16 + weight * e1 as u16 + 32) >> 6) as u8
    // if(indexprecision == 2)
    //     return (UINT8) (((64 - aWeights2[index])*UINT16(e0) + aWeights2[index]*UINT16(e1) + 32) >> 6);
    // else if(indexprecision == 3)
    //     return (UINT8) (((64 - aWeights3[index])*UINT16(e0) + aWeights3[index]*UINT16(e1) + 32) >> 6);
    // else // indexprecision == 4
    //     return (UINT8) (((64 - aWeights4[index])*UINT16(e0) + aWeights4[index]*UINT16(e1) + 32) >> 6);
}

/// The subset indexes for all 4x4 pixels encoded as a single u32.
#[derive(Clone, Copy, PartialEq, Eq)]
struct SubsetIndexMap(u32, u16);
impl SubsetIndexMap {
    const fn new_1() -> Self {
        Self(0, 1)
    }
    const fn new_2(data: [u8; 17]) -> Self {
        let mut output: u32 = 0;
        let mut fix_up_mask: u16 = 1;

        let mut pixel_index = 0;
        let mut data_index = 0;
        while data_index < data.len() {
            let d = data[data_index];
            data_index += 1;

            if d == b'-' {
                fix_up_mask |= 1 << pixel_index;
            } else {
                let d = (d - b'0') as u32;
                assert!(d <= 1);
                output |= d << (pixel_index * 2);
                pixel_index += 1;
            }
        }
        assert!(pixel_index == 16);
        assert!(fix_up_mask.count_ones() == 2);

        let result = Self(output, fix_up_mask);

        // the first subset index is always 0
        assert!(result.get_index(0).0 == 0);
        // the second fix-up index is always 1
        let one_fix_up_index = (fix_up_mask & !1).trailing_zeros();
        assert!(result.get_index(one_fix_up_index as u8).0 == 1);

        result
    }
    const fn new_3(data: [u8; 18]) -> Self {
        let mut output: u32 = 0;
        let mut fix_up_mask: u16 = 1;

        let mut pixel_index = 0;
        let mut data_index = 0;
        while data_index < data.len() {
            let d = data[data_index];
            data_index += 1;

            if d == b'-' {
                fix_up_mask |= 1 << pixel_index;
            } else {
                let d = (d - b'0') as u32;
                assert!(d <= 2);
                output |= d << (pixel_index * 2);
                pixel_index += 1;
            }
        }
        assert!(pixel_index == 16);
        assert!(fix_up_mask.count_ones() == 3);

        let result = Self(output, fix_up_mask);

        // the first subset index is always 0
        assert!(result.get_index(0).0 == 0);
        // the second and third fix-up indexes are always 1 or 2 respectively
        let one_fix_up_index = (fix_up_mask & !1).trailing_zeros();
        let two_fix_up_index = 15 - fix_up_mask.leading_zeros();
        let one_index = result.get_index(one_fix_up_index as u8).0;
        let two_index = result.get_index(two_fix_up_index as u8).0;
        assert!(one_index == 1 && two_index == 2 || one_index == 2 && two_index == 1);

        result
    }

    const fn get_index(self, pixel_index: u8) -> (u8, bool) {
        let subset = (self.0.wrapping_shr(pixel_index as u32 * 2) & 0b11) as u8;
        let fix_up = self.is_fix_up(pixel_index);
        (subset, fix_up)
    }
    const fn is_fix_up(self, pixel_index: u8) -> bool {
        (1_u16 << pixel_index & self.1) != 0
    }
}
const PARTITION_SET_1: SubsetIndexMap = SubsetIndexMap::new_1();
const PARTITION_SET_2: [SubsetIndexMap; 64] = [
    // 0
    SubsetIndexMap::new_2(*b"001100110011001-1"),
    SubsetIndexMap::new_2(*b"000100010001000-1"),
    SubsetIndexMap::new_2(*b"011101110111011-1"),
    SubsetIndexMap::new_2(*b"000100110011011-1"),
    SubsetIndexMap::new_2(*b"000000010001001-1"),
    SubsetIndexMap::new_2(*b"001101110111111-1"),
    SubsetIndexMap::new_2(*b"000100110111111-1"),
    SubsetIndexMap::new_2(*b"000000010011011-1"),
    SubsetIndexMap::new_2(*b"000000000001001-1"),
    SubsetIndexMap::new_2(*b"001101111111111-1"),
    SubsetIndexMap::new_2(*b"000000010111111-1"),
    SubsetIndexMap::new_2(*b"000000000001011-1"),
    SubsetIndexMap::new_2(*b"000101111111111-1"),
    SubsetIndexMap::new_2(*b"000000001111111-1"),
    SubsetIndexMap::new_2(*b"000011111111111-1"),
    SubsetIndexMap::new_2(*b"000000000000111-1"),
    // 16
    SubsetIndexMap::new_2(*b"000010001110111-1"),
    SubsetIndexMap::new_2(*b"01-11000100000000"),
    SubsetIndexMap::new_2(*b"00000000-10001110"),
    SubsetIndexMap::new_2(*b"01-11001100010000"),
    SubsetIndexMap::new_2(*b"00-11000100000000"),
    SubsetIndexMap::new_2(*b"00001000-11001110"),
    SubsetIndexMap::new_2(*b"00000000-10001100"),
    SubsetIndexMap::new_2(*b"011100110011000-1"),
    SubsetIndexMap::new_2(*b"00-11000100010000"),
    SubsetIndexMap::new_2(*b"00001000-10001100"),
    SubsetIndexMap::new_2(*b"01-10011001100110"),
    SubsetIndexMap::new_2(*b"00-11011001101100"),
    SubsetIndexMap::new_2(*b"00010111-11101000"),
    SubsetIndexMap::new_2(*b"00001111-11110000"),
    SubsetIndexMap::new_2(*b"01-11000110001110"),
    SubsetIndexMap::new_2(*b"00-11100110011100"),
    // 32
    SubsetIndexMap::new_2(*b"010101010101010-1"),
    SubsetIndexMap::new_2(*b"000011110000111-1"),
    SubsetIndexMap::new_2(*b"010110-1001011010"),
    SubsetIndexMap::new_2(*b"00110011-11001100"),
    SubsetIndexMap::new_2(*b"00-11110000111100"),
    SubsetIndexMap::new_2(*b"01010101-10101010"),
    SubsetIndexMap::new_2(*b"011010010110100-1"),
    SubsetIndexMap::new_2(*b"010110101010010-1"),
    SubsetIndexMap::new_2(*b"01-11001111001110"),
    SubsetIndexMap::new_2(*b"00010011-11001000"),
    SubsetIndexMap::new_2(*b"00-11001001001100"),
    SubsetIndexMap::new_2(*b"00-11101111011100"),
    SubsetIndexMap::new_2(*b"01-10100110010110"),
    SubsetIndexMap::new_2(*b"001111001100001-1"),
    SubsetIndexMap::new_2(*b"011001101001100-1"),
    SubsetIndexMap::new_2(*b"000001-1001100000"),
    // 48
    SubsetIndexMap::new_2(*b"010011-1001000000"),
    SubsetIndexMap::new_2(*b"00-10011100100000"),
    SubsetIndexMap::new_2(*b"000000-1001110010"),
    SubsetIndexMap::new_2(*b"00000100-11100100"),
    SubsetIndexMap::new_2(*b"011011001001001-1"),
    SubsetIndexMap::new_2(*b"001101101100100-1"),
    SubsetIndexMap::new_2(*b"01-10001110011100"),
    SubsetIndexMap::new_2(*b"00-11100111000110"),
    SubsetIndexMap::new_2(*b"011011001100100-1"),
    SubsetIndexMap::new_2(*b"011000110011100-1"),
    SubsetIndexMap::new_2(*b"011111101000000-1"),
    SubsetIndexMap::new_2(*b"000110001110011-1"),
    SubsetIndexMap::new_2(*b"000011110011001-1"),
    SubsetIndexMap::new_2(*b"00-11001111110000"),
    SubsetIndexMap::new_2(*b"00-10001011101110"),
    SubsetIndexMap::new_2(*b"010001000111011-1"),
];
const PARTITION_SET_3: [SubsetIndexMap; 64] = [
    // 0
    SubsetIndexMap::new_3(*b"001-100110221222-2"),
    SubsetIndexMap::new_3(*b"000-10011-22112221"),
    SubsetIndexMap::new_3(*b"00002001-2211221-1"),
    SubsetIndexMap::new_3(*b"022-200220011011-1"),
    SubsetIndexMap::new_3(*b"00000000-1122112-2"),
    SubsetIndexMap::new_3(*b"001-100110022002-2"),
    SubsetIndexMap::new_3(*b"002-200221111111-1"),
    SubsetIndexMap::new_3(*b"00110011-2211221-1"),
    SubsetIndexMap::new_3(*b"00000000-1111222-2"),
    SubsetIndexMap::new_3(*b"00001111-1111222-2"),
    SubsetIndexMap::new_3(*b"000011-112222222-2"),
    SubsetIndexMap::new_3(*b"001200-120012001-2"),
    SubsetIndexMap::new_3(*b"011201-120112011-2"),
    SubsetIndexMap::new_3(*b"01220-1220122012-2"),
    SubsetIndexMap::new_3(*b"001-101121122122-2"),
    SubsetIndexMap::new_3(*b"001-12001-22002220"),
    // 16
    SubsetIndexMap::new_3(*b"000-100110112112-2"),
    SubsetIndexMap::new_3(*b"011-10011-20012200"),
    SubsetIndexMap::new_3(*b"00001122-1122112-2"),
    SubsetIndexMap::new_3(*b"002-200220022111-1"),
    SubsetIndexMap::new_3(*b"011-101110222022-2"),
    SubsetIndexMap::new_3(*b"000-10001-22212221"),
    SubsetIndexMap::new_3(*b"000000-110122012-2"),
    SubsetIndexMap::new_3(*b"00001100-22-102210"),
    SubsetIndexMap::new_3(*b"012-20-12200110000"),
    SubsetIndexMap::new_3(*b"00120012-1122222-2"),
    SubsetIndexMap::new_3(*b"011012-21-12210110"),
    SubsetIndexMap::new_3(*b"000001-1012-211221"),
    SubsetIndexMap::new_3(*b"00221102-1102002-2"),
    SubsetIndexMap::new_3(*b"01100-1102002222-2"),
    SubsetIndexMap::new_3(*b"0011012201-22001-1"),
    SubsetIndexMap::new_3(*b"00002000-2211222-1"),
    // 32
    SubsetIndexMap::new_3(*b"00000002-1122122-2"),
    SubsetIndexMap::new_3(*b"022-200220012001-1"),
    SubsetIndexMap::new_3(*b"001-100120022022-2"),
    SubsetIndexMap::new_3(*b"01200-12001-200120"),
    SubsetIndexMap::new_3(*b"000011-1122-220000"),
    SubsetIndexMap::new_3(*b"01201201-20-120120"),
    SubsetIndexMap::new_3(*b"01202012-1-2010120"),
    SubsetIndexMap::new_3(*b"0011220011-22001-1"),
    SubsetIndexMap::new_3(*b"001111-222200001-1"),
    SubsetIndexMap::new_3(*b"010-101012222222-2"),
    SubsetIndexMap::new_3(*b"00000000-2121212-1"),
    SubsetIndexMap::new_3(*b"00221-1220022112-2"),
    SubsetIndexMap::new_3(*b"002-200110022001-1"),
    SubsetIndexMap::new_3(*b"022012-210220122-1"),
    SubsetIndexMap::new_3(*b"010122-222222010-1"),
    SubsetIndexMap::new_3(*b"00002121-2121212-1"),
    // 48
    SubsetIndexMap::new_3(*b"010-101010101222-2"),
    SubsetIndexMap::new_3(*b"022-201110222011-1"),
    SubsetIndexMap::new_3(*b"00021-1120002111-2"),
    SubsetIndexMap::new_3(*b"00002-1122112211-2"),
    SubsetIndexMap::new_3(*b"02220-1110111022-2"),
    SubsetIndexMap::new_3(*b"00021112-1112000-2"),
    SubsetIndexMap::new_3(*b"01100-1100110222-2"),
    SubsetIndexMap::new_3(*b"0000000021-12211-2"),
    SubsetIndexMap::new_3(*b"01100-1102222222-2"),
    SubsetIndexMap::new_3(*b"0022001100-11002-2"),
    SubsetIndexMap::new_3(*b"00221122-1122002-2"),
    SubsetIndexMap::new_3(*b"0000000000002-11-2"),
    SubsetIndexMap::new_3(*b"000-200010002000-1"),
    SubsetIndexMap::new_3(*b"022212220222-122-2"),
    SubsetIndexMap::new_3(*b"010-122222222222-2"),
    SubsetIndexMap::new_3(*b"011-12011-22012220"),
];

struct BitStream {
    state: u128,
    consumed_bits: u8,
}
impl BitStream {
    pub fn new(block: [u8; 16]) -> Self {
        Self {
            state: u128::from_le_bytes(block),
            consumed_bits: 0,
        }
    }

    pub fn low_u8(&self) -> u8 {
        self.state as u8
    }

    #[inline(always)]
    pub fn skip(&mut self, n: u8) {
        self.state >>= n;
        self.consumed_bits += n;
    }

    #[inline(always)]
    pub fn consume_bit(&mut self) -> bool {
        let bit = self.state as u8 & 1 != 0;
        self.skip(1);
        bit
    }

    #[inline(always)]
    pub fn consume_bits(&mut self, count: u8) -> u8 {
        debug_assert!(0 < count && count <= 8);
        let mask = (1_u16 << count).wrapping_sub(1) as u8;
        let bits = self.state as u8 & mask;
        self.skip(count);
        bits
    }
    #[inline(always)]
    pub fn consume_bits_16(&mut self, count: u8) -> u16 {
        debug_assert!(0 < count && count <= 16);
        let mut bits = self.state as u16;
        if count < 16 {
            bits &= (1_u16.wrapping_shl(count as u32)).wrapping_sub(1);
        }
        self.skip(count);
        bits
    }
    #[inline(always)]
    pub fn consume_bits_32(&mut self, count: u8) -> u32 {
        debug_assert!(0 < count && count <= 32);
        let mut bits = self.state as u32;
        if count < 32 {
            bits &= (1_u32.wrapping_shl(count as u32)).wrapping_sub(1);
        }
        self.skip(count);
        bits
    }
    #[inline(always)]
    pub fn consume_bits_64(&mut self, count: u8) -> u64 {
        debug_assert!(0 < count && count <= 64);
        let mut bits = self.state as u64;
        if count < 64 {
            bits &= (1_u64.wrapping_shl(count as u32)).wrapping_sub(1);
        }
        self.skip(count);
        bits
    }

    pub fn validate(self) {
        debug_assert_eq!(self.consumed_bits, 128);
    }
}
