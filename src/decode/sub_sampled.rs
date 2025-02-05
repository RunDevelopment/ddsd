use crate::util::closure_types;
use crate::{Channels::*, ColorFormat};

use super::convert::{n8, ToRgba, WithPrecision};
use super::read_write::{
    for_each_block_rect_untyped, for_each_block_untyped, process_2x1_blocks_helper, PixelRange,
};
use super::{Args, DecoderSet, DirectDecoder, RArgs};

// helpers

macro_rules! underlying {
    ($channels:expr, $out:ty, $f:expr) => {{
        const BYTES_PER_BLOCK: usize = 4;
        const CHANNELS: usize = $channels.count() as usize;
        type OutPixel = [$out; CHANNELS];

        fn process_blocks(
            encoded_blocks: &[u8],
            decoded: &mut [u8],
            _stride: usize,
            range: PixelRange,
        ) {
            let f = closure_types::<[u8; BYTES_PER_BLOCK], [OutPixel; 2], _>($f);
            process_2x1_blocks_helper(encoded_blocks, decoded, range, f)
        }

        DirectDecoder::new(
            ColorFormat::new($channels, <$out as WithPrecision>::PRECISION),
            |Args(r, out, context)| {
                for_each_block_untyped::<2, 1, BYTES_PER_BLOCK, OutPixel>(
                    r,
                    out,
                    context.size,
                    process_blocks,
                )
            },
            |RArgs(r, out, row_pitch, rect, context)| {
                for_each_block_rect_untyped::<2, 1, BYTES_PER_BLOCK>(
                    r,
                    out,
                    row_pitch,
                    context.size,
                    rect,
                    process_blocks,
                )
            },
        )
    }};
}

macro_rules! rgb {
    ($out:ty, $f:expr) => {
        underlying!(Rgb, $out, $f)
    };
}
macro_rules! rgba {
    ($out:ty, $f:expr) => {
        underlying!(Rgba, $out, $f)
    };
}

// decoders

#[inline]
fn decode_rg_bg<T: Copy>([r, g1, b, g2]: [T; 4]) -> [[T; 3]; 2] {
    [[r, g1, b], [r, g2, b]]
}
pub(crate) const R8G8_B8G8_UNORM: DecoderSet = DecoderSet::new(&[
    rgb!(u8, decode_rg_bg),
    rgb!(u16, |pair| decode_rg_bg(pair.map(n8::n16))),
    rgb!(f32, |pair| decode_rg_bg(pair.map(n8::f32))),
    rgba!(u8, |pair| decode_rg_bg(pair).map(ToRgba::to_rgba)),
    rgba!(u16, |pair| decode_rg_bg(pair.map(n8::n16))
        .map(ToRgba::to_rgba)),
    rgba!(f32, |pair| decode_rg_bg(pair.map(n8::f32))
        .map(ToRgba::to_rgba)),
]);

#[inline]
fn decode_gr_bg<T: Copy>([g1, r, g2, b]: [T; 4]) -> [[T; 3]; 2] {
    [[r, g1, b], [r, g2, b]]
}
pub(crate) const G8R8_G8B8_UNORM: DecoderSet = DecoderSet::new(&[
    rgb!(u8, decode_gr_bg),
    rgb!(u16, |pair| decode_gr_bg(pair.map(n8::n16))),
    rgb!(f32, |pair| decode_gr_bg(pair.map(n8::f32))),
    rgba!(u8, |pair| decode_gr_bg(pair).map(ToRgba::to_rgba)),
    rgba!(u16, |pair| decode_gr_bg(pair.map(n8::n16))
        .map(ToRgba::to_rgba)),
    rgba!(f32, |pair| decode_gr_bg(pair.map(n8::f32))
        .map(ToRgba::to_rgba)),
]);
