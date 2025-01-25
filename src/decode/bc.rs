use super::convert::{Norm, ToRgb, ToRgba};
use super::read_write::for_each_block_4x4;
use super::{Args, Decoder, DecoderSet, WithPrecision};

use crate::Channels::*;

// helpers

macro_rules! gray {
    ($out:ty, $f:expr) => {
        Decoder::new_without_rect_decode(
            Grayscale,
            <$out as WithPrecision>::PRECISION,
            |Args(r, out, context)| {
                let f = $f;
                for_each_block_4x4(r, out, context.size, |pixel| -> [[$out; 1]; 16] {
                    f(pixel)
                })
            },
        )
    };
}
macro_rules! rgb {
    ($out:ty, $f:expr) => {
        Decoder::new_without_rect_decode(
            Rgb,
            <$out as WithPrecision>::PRECISION,
            |Args(r, out, context)| {
                let f = $f;
                for_each_block_4x4(r, out, context.size, |pixel| -> [[$out; 3]; 16] {
                    f(pixel)
                })
            },
        )
    };
}
macro_rules! rgba {
    ($out:ty, $f:expr) => {
        Decoder::new_without_rect_decode(
            Rgba,
            <$out as WithPrecision>::PRECISION,
            |Args(r, out, context)| {
                let f = $f;
                for_each_block_4x4(r, out, context.size, |pixel| -> [[$out; 4]; 16] {
                    f(pixel)
                })
            },
        )
    };
}

fn gray_to_rgb<const N: usize, T: Copy>(
    f: impl Fn([u8; N]) -> [[T; 1]; 16],
) -> impl Fn([u8; N]) -> [[T; 3]; 16] {
    move |block_bytes| f(block_bytes).map(ToRgb::to_rgb)
}
fn gray_to_rgba<const N: usize, T: Norm>(
    f: impl Fn([u8; N]) -> [[T; 1]; 16],
) -> impl Fn([u8; N]) -> [[T; 4]; 16] {
    move |block_bytes| f(block_bytes).map(ToRgba::to_rgba)
}
fn rgb_to_rgba<const N: usize, T: Norm>(
    f: impl Fn([u8; N]) -> [[T; 3]; 16],
) -> impl Fn([u8; N]) -> [[T; 4]; 16] {
    move |block_bytes| f(block_bytes).map(ToRgba::to_rgba)
}
fn rgba_to_rgb<const N: usize, T>(
    f: impl Fn([u8; N]) -> [[T; 4]; 16],
) -> impl Fn([u8; N]) -> [[T; 3]; 16] {
    move |block_bytes| f(block_bytes).map(ToRgb::to_rgb)
}

// decoders

pub(crate) const BC1_UNORM: DecoderSet = DecoderSet::new(&[
    rgba!(u8, blocks::bc1_u8_rgba),
    rgb!(u8, rgba_to_rgb(blocks::bc1_u8_rgba)),
]);

pub(crate) const BC2_UNORM: DecoderSet =
    DecoderSet::new(&[rgba!(u8, blocks::bc2_u8_rgba), rgb!(u8, blocks::bc2_u8_rgb)]);

pub(crate) const BC3_UNORM: DecoderSet =
    DecoderSet::new(&[rgba!(u8, blocks::bc3_u8_rgba), rgb!(u8, blocks::bc3_u8_rgb)]);

pub(crate) const BC3_UNORM_RXGB: DecoderSet = DecoderSet::new(&[
    rgb!(u8, blocks::bc3_rxgb_u8_rgb),
    rgba!(u8, rgb_to_rgba(blocks::bc3_rxgb_u8_rgb)),
]);

pub(crate) const BC4_UNORM: DecoderSet = DecoderSet::new(&[
    gray!(u8, blocks::bc4u_gray),
    gray!(u16, blocks::bc4u_gray),
    gray!(f32, blocks::bc4u_gray),
    rgb!(u8, gray_to_rgb(blocks::bc4u_gray)),
    rgb!(u16, gray_to_rgb(blocks::bc4u_gray)),
    rgb!(f32, gray_to_rgb(blocks::bc4u_gray)),
    rgba!(u8, gray_to_rgba(blocks::bc4u_gray)),
    rgba!(u16, gray_to_rgba(blocks::bc4u_gray)),
    rgba!(f32, gray_to_rgba(blocks::bc4u_gray)),
]);

pub(crate) const BC4_SNORM: DecoderSet = DecoderSet::new(&[
    gray!(u8, blocks::bc4s_u8_gray),
    rgb!(u8, gray_to_rgb(blocks::bc4s_u8_gray)),
    rgba!(u8, gray_to_rgba(blocks::bc4s_u8_gray)),
]);

pub(crate) const BC5_UNORM: DecoderSet = DecoderSet::new(&[
    rgb!(u8, blocks::bc5u_rgb),
    rgb!(u16, blocks::bc5u_rgb),
    rgb!(f32, blocks::bc5u_rgb),
    rgba!(u8, rgb_to_rgba(blocks::bc5u_rgb)),
    rgba!(u16, rgb_to_rgba(blocks::bc5u_rgb)),
    rgba!(f32, rgb_to_rgba(blocks::bc5u_rgb)),
]);

pub(crate) const BC5_SNORM: DecoderSet = DecoderSet::new(&[
    rgb!(u8, blocks::bc5s_u8_rgb),
    rgba!(u8, rgb_to_rgba(blocks::bc5s_u8_rgb)),
]);

fn dummy_bc6(_block: [u8; 16]) -> [[f32; 3]; 16] {
    todo!("BC6H is not supported yet")
}
pub(crate) const BC6H_UF16: DecoderSet =
    DecoderSet::new(&[rgb!(f32, dummy_bc6), rgba!(f32, rgb_to_rgba(dummy_bc6))]);
pub(crate) const BC6H_SF16: DecoderSet =
    DecoderSet::new(&[rgb!(f32, dummy_bc6), rgba!(f32, rgb_to_rgba(dummy_bc6))]);

pub(crate) const BC7_UNORM: DecoderSet = DecoderSet::new(&[
    rgba!(u8, blocks::bc7_u8_rgba),
    rgb!(u8, rgba_to_rgb(blocks::bc7_u8_rgba)),
]);

/// Internal module for the underlying logic of decoding BC1-7 blocks.
mod blocks {
    use crate::decode::convert::{n4, n8, s8, Norm, ToRgba, B5G6R5};

    /// Decodes a BC1 block into 16 RGBA pixels.
    pub(crate) fn bc1_u8_rgba(block_bytes: [u8; 8]) -> [[u8; 4]; 16] {
        // https://learn.microsoft.com/en-us/windows/win32/direct3d10/d3d10-graphics-programming-guide-resources-block-compression#bc1
        let color0_u16 = u16::from_le_bytes([block_bytes[0], block_bytes[1]]);
        let color1_u16 = u16::from_le_bytes([block_bytes[2], block_bytes[3]]);

        let c0_bgr = B5G6R5::from_u16(color0_u16);
        let c1_bgr = B5G6R5::from_u16(color1_u16);

        let c0 = c0_bgr.to_n8().to_rgba();
        let c1 = c1_bgr.to_n8().to_rgba();

        let mut pixels: [[u8; 4]; 16] = Default::default();

        let (c2, c3) = if color0_u16 > color1_u16 {
            (
                c0_bgr.one_third_color_rgb8(c1_bgr).to_rgba(),
                c0_bgr.two_third_color_rgb8(c1_bgr).to_rgba(),
            )
        } else {
            (
                c0_bgr.mid_color_rgb8(c1_bgr).to_rgba(),
                [0, 0, 0, 0], // transparent
            )
        };

        let lut = [c0, c1, c2, c3];
        let indexes = u32::from_le_bytes([
            block_bytes[4],
            block_bytes[5],
            block_bytes[6],
            block_bytes[7],
        ]);
        for (i, pixel) in pixels.iter_mut().enumerate() {
            let index = (indexes >> (i * 2)) & 0b11;
            *pixel = lut[index as usize];
        }

        pixels
    }

    fn bc1_no_default_u8_rgba(block_bytes: [u8; 8]) -> [[u8; 4]; 16] {
        // https://learn.microsoft.com/en-us/windows/win32/direct3d10/d3d10-graphics-programming-guide-resources-block-compression#bc1
        let color0_u16 = u16::from_le_bytes([block_bytes[0], block_bytes[1]]);
        let color1_u16 = u16::from_le_bytes([block_bytes[2], block_bytes[3]]);

        let c0_bgr = B5G6R5::from_u16(color0_u16);
        let c1_bgr = B5G6R5::from_u16(color1_u16);

        let c0 = c0_bgr.to_n8().to_rgba();
        let c1 = c1_bgr.to_n8().to_rgba();
        let c2 = c0_bgr.one_third_color_rgb8(c1_bgr).to_rgba();
        let c3 = c0_bgr.two_third_color_rgb8(c1_bgr).to_rgba();

        let mut pixels: [[u8; 4]; 16] = Default::default();

        let lut = [c0, c1, c2, c3];
        let indexes = u32::from_le_bytes([
            block_bytes[4],
            block_bytes[5],
            block_bytes[6],
            block_bytes[7],
        ]);
        for (i, pixel) in pixels.iter_mut().enumerate() {
            let index = (indexes >> (i * 2)) & 0b11;
            *pixel = lut[index as usize];
        }

        pixels
    }

    fn split_16(x: [u8; 16]) -> ([u8; 8], [u8; 8]) {
        let lower = [x[0], x[1], x[2], x[3], x[4], x[5], x[6], x[7]];
        let upper = [x[8], x[9], x[10], x[11], x[12], x[13], x[14], x[15]];
        (lower, upper)
    }

    /// Decodes a BC2 block into 16 RGBA pixels.
    pub(crate) fn bc2_u8_rgba(block_bytes: [u8; 16]) -> [[u8; 4]; 16] {
        // https://learn.microsoft.com/en-us/windows/win32/direct3d10/d3d10-graphics-programming-guide-resources-block-compression#bc2
        let (alpha_bytes, bc1_bytes) = split_16(block_bytes);
        let mut pixels = bc1_no_default_u8_rgba(bc1_bytes);

        for i in 0..4 {
            let alpha_byte_high = alpha_bytes[i * 2];
            let alpha_byte_low = alpha_bytes[i * 2 + 1];
            let alpha = [
                alpha_byte_high & 0xF,
                alpha_byte_high >> 4,
                alpha_byte_low & 0xF,
                alpha_byte_low >> 4,
            ]
            .map(n4::n8);

            for (j, &alpha) in alpha.iter().enumerate() {
                pixels[i * 4 + j][3] = alpha;
            }
        }

        pixels
    }
    pub(crate) fn bc2_u8_rgb(block_bytes: [u8; 16]) -> [[u8; 3]; 16] {
        // https://learn.microsoft.com/en-us/windows/win32/direct3d10/d3d10-graphics-programming-guide-resources-block-compression#bc2
        let (_, bc1_bytes) = split_16(block_bytes);
        let pixels = bc1_no_default_u8_rgba(bc1_bytes);
        pixels.map(|[r, g, b, _]| [r, g, b])
    }

    /// Decodes a BC3 block into 16 RGBA pixels.
    pub(crate) fn bc3_u8_rgba(block_bytes: [u8; 16]) -> [[u8; 4]; 16] {
        // https://learn.microsoft.com/en-us/windows/win32/direct3d10/d3d10-graphics-programming-guide-resources-block-compression#bc3
        let (alpha_bytes, bc1_bytes) = split_16(block_bytes);

        let mut pixels = bc1_u8_rgba(bc1_bytes);
        let alpha = bc4u_gray(alpha_bytes);

        for i in 0..4 {
            for j in 0..4 {
                pixels[i * 4 + j][3] = alpha[i * 4 + j][0];
            }
        }

        pixels
    }
    pub(crate) fn bc3_u8_rgb(block_bytes: [u8; 16]) -> [[u8; 3]; 16] {
        // https://learn.microsoft.com/en-us/windows/win32/direct3d10/d3d10-graphics-programming-guide-resources-block-compression#bc3
        let (_, bc1_bytes) = split_16(block_bytes);
        let pixels = bc1_u8_rgba(bc1_bytes);
        pixels.map(|[r, g, b, _]| [r, g, b])
    }
    pub(crate) fn bc3_rxgb_u8_rgb(block_bytes: [u8; 16]) -> [[u8; 3]; 16] {
        bc3_u8_rgba(block_bytes).map(|[_, g, b, r]| [r, g, b])
    }

    pub(crate) trait BC4uOperations: Norm {
        /// Given a UNORM 8 endpoint, convert to Self.
        fn from_byte(byte: u8) -> Self;
        /// Given a UNORM in the range `0..=255*7`, convert to Self.
        fn from_interpolation_6(interpolation: u16) -> Self;
        /// Given a UNORM in the range `0..=255*5`, convert to Self.
        fn from_interpolation_4(interpolation: u16) -> Self;
    }
    impl BC4uOperations for u8 {
        fn from_byte(byte: u8) -> Self {
            byte
        }
        fn from_interpolation_6(interpolation: u16) -> Self {
            debug_assert!(interpolation <= 1785);
            ((interpolation as u32 * 9360 + 32160) >> 16) as u8
        }
        fn from_interpolation_4(interpolation: u16) -> Self {
            debug_assert!(interpolation <= 1275);
            ((interpolation as u32 * 13104 + 30288) >> 16) as u8
        }
    }
    impl BC4uOperations for u16 {
        fn from_byte(byte: u8) -> Self {
            n8::n16(byte)
        }
        fn from_interpolation_6(interpolation: u16) -> Self {
            debug_assert!(interpolation <= 1785);
            ((interpolation as u32 * 2406112 + 28064) >> 16) as u16
        }
        fn from_interpolation_4(interpolation: u16) -> Self {
            debug_assert!(interpolation <= 1275);
            ((interpolation as u32 * 3368544 + 34368) >> 16) as u16
        }
    }
    impl BC4uOperations for f32 {
        fn from_byte(byte: u8) -> Self {
            n8::f32(byte)
        }
        fn from_interpolation_6(interpolation: u16) -> Self {
            debug_assert!(interpolation <= 1785);
            const F: f32 = 1.0 / 1785.0;
            interpolation as f32 * F
        }
        fn from_interpolation_4(interpolation: u16) -> Self {
            debug_assert!(interpolation <= 1275);
            const F: f32 = 1.0 / 1275.0;
            interpolation as f32 * F
        }
    }
    pub(crate) fn bc4u_gray<T: BC4uOperations>(block_bytes: [u8; 8]) -> [[T; 1]; 16] {
        // https://learn.microsoft.com/en-us/windows/win32/direct3d10/d3d10-graphics-programming-guide-resources-block-compression#bc4
        let c0_u8 = block_bytes[0];
        let c1_u8 = block_bytes[1];
        let c0_u16 = c0_u8 as u16;
        let c1_u16 = c1_u8 as u16;

        let c0 = T::from_byte(c0_u8);
        let c1 = T::from_byte(c1_u8);

        let (c2, c3, c4, c5, c6, c7) = if c0_u8 > c1_u8 {
            // 6 interpolated colors
            (
                T::from_interpolation_6(c0_u16 * 6 + c1_u16),
                T::from_interpolation_6(c0_u16 * 5 + c1_u16 * 2),
                T::from_interpolation_6(c0_u16 * 4 + c1_u16 * 3),
                T::from_interpolation_6(c0_u16 * 3 + c1_u16 * 4),
                T::from_interpolation_6(c0_u16 * 2 + c1_u16 * 5),
                T::from_interpolation_6(c0_u16 + c1_u16 * 6),
            )
        } else {
            // 4 interpolated colors
            (
                T::from_interpolation_4(c0_u16 * 4 + c1_u16),
                T::from_interpolation_4(c0_u16 * 3 + c1_u16 * 2),
                T::from_interpolation_4(c0_u16 * 2 + c1_u16 * 3),
                T::from_interpolation_4(c0_u16 + c1_u16 * 4),
                T::NORM_ZERO,
                T::NORM_ONE,
            )
        };

        let mut pixels: [[T; 1]; 16] = Default::default();

        let lut = [c0, c1, c2, c3, c4, c5, c6, c7];
        let indexes0 = u32::from_le_bytes([block_bytes[2], block_bytes[3], block_bytes[4], 0]);
        let indexes1 = u32::from_le_bytes([block_bytes[5], block_bytes[6], block_bytes[7], 0]);
        for (i, indexes) in [indexes0, indexes1].into_iter().enumerate() {
            for j in 0..8 {
                let index = (indexes >> (j * 3)) & 0b111;
                pixels[i * 8 + j][0] = lut[index as usize];
            }
        }

        pixels
    }

    /// Decodes a BC5 UNORM block into 16 RGB pixels.
    pub(crate) fn bc5u_rgb<T: BC4uOperations>(block_bytes: [u8; 16]) -> [[T; 3]; 16] {
        let red = bc4u_gray(block_bytes[0..8].try_into().unwrap());
        let green = bc4u_gray(block_bytes[8..16].try_into().unwrap());

        let mut pixels: [[T; 3]; 16] = Default::default();
        for (i, pixel) in pixels.iter_mut().enumerate() {
            pixel[0] = red[i][0];
            pixel[1] = green[i][0];
            pixel[2] = T::NORM_ZERO;
        }

        pixels
    }

    pub(crate) fn bc4s_u8_gray(block_bytes: [u8; 8]) -> [[u8; 1]; 16] {
        // https://learn.microsoft.com/en-us/windows/win32/direct3d10/d3d10-graphics-programming-guide-resources-block-compression#bc4
        let red0 = block_bytes[0];
        let red1 = block_bytes[1];

        let c0 = s8::n8(red0);
        let c1 = s8::n8(red1);

        // exact f32 values of c0 and c1
        const CONVERSION_FACTOR: f32 = 255.0 / 254.0;
        let c0_f = red0.wrapping_add(128).saturating_sub(1) as f32 * CONVERSION_FACTOR;
        let c1_f = red1.wrapping_add(128).saturating_sub(1) as f32 * CONVERSION_FACTOR;

        fn interpolate(red0: f32, red1: f32, blend: f32) -> u8 {
            (red0 * (1.0 - blend) + red1 * blend + 0.5) as u8
        }
        let (c2, c3, c4, c5, c6, c7) = if c0 > c1 {
            // 6 interpolated colors
            (
                interpolate(c0_f, c1_f, 1.0 / 7.0),
                interpolate(c0_f, c1_f, 2.0 / 7.0),
                interpolate(c0_f, c1_f, 3.0 / 7.0),
                interpolate(c0_f, c1_f, 4.0 / 7.0),
                interpolate(c0_f, c1_f, 5.0 / 7.0),
                interpolate(c0_f, c1_f, 6.0 / 7.0),
            )
        } else {
            // 4 interpolated colors
            (
                interpolate(c0_f, c1_f, 1.0 / 5.0),
                interpolate(c0_f, c1_f, 2.0 / 5.0),
                interpolate(c0_f, c1_f, 3.0 / 5.0),
                interpolate(c0_f, c1_f, 4.0 / 5.0),
                0,
                255,
            )
        };

        let mut pixels: [[u8; 1]; 16] = Default::default();

        let lut = [c0, c1, c2, c3, c4, c5, c6, c7];
        let indexes0 = u32::from_le_bytes([block_bytes[2], block_bytes[3], block_bytes[4], 0]);
        let indexes1 = u32::from_le_bytes([block_bytes[5], block_bytes[6], block_bytes[7], 0]);
        for (i, indexes) in [indexes0, indexes1].into_iter().enumerate() {
            for j in 0..8 {
                let index = (indexes >> (j * 3)) & 0b111;
                pixels[i * 8 + j][0] = lut[index as usize];
            }
        }

        pixels
    }

    /// Decodes a BC5 UNORM block into 16 RGB pixels.
    pub(crate) fn bc5s_u8_rgb(block_bytes: [u8; 16]) -> [[u8; 3]; 16] {
        let red = bc4s_u8_gray(block_bytes[0..8].try_into().unwrap());
        let green = bc4s_u8_gray(block_bytes[8..16].try_into().unwrap());

        let mut pixels: [[u8; 3]; 16] = Default::default();
        for (i, pixel) in pixels.iter_mut().enumerate() {
            pixel[0] = red[i][0];
            pixel[1] = green[i][0];
            pixel[2] = 128;
        }

        pixels
    }

    /// Decodes a BC7 UNORM block into 16 RGBA pixels.
    pub(crate) fn bc7_u8_rgba(block_bytes: [u8; 16]) -> [[u8; 4]; 16] {
        super::super::bc7::decode_bc7_block(block_bytes)
    }
}
