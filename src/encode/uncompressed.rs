use std::io::Write;

use bitflags::bitflags;

use crate::{
    cast, ch, convert_channels_untyped, convert_to_rgba_f32, fp10, fp11, fp16, n1, n10, n16, n2,
    n4, n5, n6, n8, rgb9995f, s16, s8, util, xr10, yuv10, yuv16, yuv8, Channels, ColorFormat,
    ColorFormatSet, Precision,
};

use super::{Args, DecodedArgs, EncodeError, Encoder};

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(crate) struct Flags: u8 {
        /// Whether all U8 values will be encoded exactly, meaning no loss of
        /// precision.
        ///
        /// SNORM8 is considered exact.
        const EXACT_U8 = 0b001;
        /// Whether all U16 values will be encoded exactly, meaning no loss of
        /// precision.
        ///
        /// SNORM16 is considered exact.
        ///
        /// This flag implies `EXACT_U8`.
        const EXACT_U16 = 0b011;
        /// Whether all F32 values will be encoded exactly, meaning no loss of
        /// precision.
        ///
        /// This flag implies `EXACT_U16` and `EXACT_U8`.
        const EXACT_F32 = 0b111;
    }
}

impl Flags {
    const fn exact_for(precision: Precision) -> Self {
        match precision {
            Precision::U8 => Flags::EXACT_U8,
            Precision::U16 => Flags::EXACT_U16,
            Precision::F32 => Flags::EXACT_F32,
        }
    }
}

pub(crate) struct UncompressedEncoder {
    color_formats: ColorFormatSet,
    flags: Flags,
    encode: fn(Args) -> Result<(), EncodeError>,
}
impl UncompressedEncoder {
    const fn copy(color: ColorFormat) -> Self {
        let flags = Flags::exact_for(color.precision);

        Self {
            color_formats: ColorFormatSet::from_single(color),
            flags,
            encode: |args| copy_directly(args),
        }
    }

    const fn add_flags(mut self, flags: Flags) -> Self {
        self.flags = self.flags.union(flags);
        self
    }
}
impl Encoder for UncompressedEncoder {
    fn supported_color_formats(&self) -> ColorFormatSet {
        self.color_formats
    }

    fn encode(
        &self,
        data: &[u8],
        width: u32,
        color: ColorFormat,
        writer: &mut dyn Write,
    ) -> Result<(), EncodeError> {
        if !self.color_formats.contains(color) {
            return Err(EncodeError::UnsupportedColorFormat(color));
        }

        (self.encode)(Args(data, width, color, writer))
    }
}
impl Encoder for &[UncompressedEncoder] {
    fn supported_color_formats(&self) -> ColorFormatSet {
        let mut set = ColorFormatSet::EMPTY;
        for encoder in *self {
            set = set.union(encoder.supported_color_formats());
        }
        set
    }

    fn encode(
        &self,
        data: &[u8],
        width: u32,
        color: ColorFormat,
        writer: &mut dyn Write,
    ) -> Result<(), EncodeError> {
        for encoder in *self {
            if encoder.supported_color_formats().contains(color) {
                return encoder.encode(data, width, color, writer);
            }
        }
        Err(EncodeError::UnsupportedColorFormat(color))
    }
}

fn process_subsample<const BLOCK_WIDTH: usize, EncodedBlock, F>(
    data: &[[f32; 4]],
    out: &mut [EncodedBlock],
    f: F,
) where
    F: Fn(&[[f32; 4]; BLOCK_WIDTH]) -> EncodedBlock,
{
    let full_blocks_len = data.len() / BLOCK_WIDTH * BLOCK_WIDTH;
    let rest = data.len() - full_blocks_len;

    // process full blocks
    let full = cast::as_array_chunks(&data[..full_blocks_len]).unwrap();
    for (i, o) in full.iter().zip(out.iter_mut()) {
        *o = f(i);
    }

    // process the last partial block (if any)
    if rest > 0 {
        let mut last_block = [[0_f32; 4]; BLOCK_WIDTH];
        last_block[..rest].copy_from_slice(&data[full_blocks_len..]);
        last_block[rest..].fill(data[data.len() - 1]);

        out[full.len()] = f(&last_block);
    }
}
fn uncompressed_universal_subsample<EncodedBlock>(
    args: Args,
    block_width: usize,
    process: fn(&[[f32; 4]], &mut [EncodedBlock]),
) -> Result<(), EncodeError>
where
    EncodedBlock: Default + Copy + ToLe + cast::Castable,
{
    let DecodedArgs {
        data,
        color,
        writer,
        width,
        ..
    } = DecodedArgs::from(args)?;
    let bytes_per_pixel = color.bytes_per_pixel() as usize;

    assert!(block_width >= 2);

    const BUFFER_PIXELS: usize = 512;
    let mut intermediate_buffer = [[0_f32; 4]; BUFFER_PIXELS];
    let mut encoded_buffer = [EncodedBlock::default(); BUFFER_PIXELS / 2];

    for y_line in data.chunks(width * bytes_per_pixel) {
        debug_assert!(y_line.len() == width * bytes_per_pixel);

        let chunk_size = BUFFER_PIXELS * bytes_per_pixel;
        for chunk in y_line.chunks(chunk_size) {
            let pixels = chunk.len() / bytes_per_pixel;

            let intermediate = &mut intermediate_buffer[..pixels];
            let encoded = &mut encoded_buffer[..util::div_ceil(pixels, block_width)];

            convert_to_rgba_f32(color, chunk, intermediate);

            process(intermediate, encoded);

            ToLe::to_le(encoded);

            writer.write_all(cast::as_bytes(encoded))?;
        }
    }

    Ok(())
}

fn uncompressed_universal<EncodedPixel>(
    args: Args,
    process: fn(&[[f32; 4]], &mut [EncodedPixel]),
) -> Result<(), EncodeError>
where
    EncodedPixel: Default + Copy + ToLe + cast::Castable,
{
    let DecodedArgs {
        data,
        color,
        writer,
        ..
    } = DecodedArgs::from(args)?;
    let bytes_per_pixel = color.bytes_per_pixel() as usize;

    const BUFFER_PIXELS: usize = 512;
    let mut intermediate_buffer = [[0_f32; 4]; BUFFER_PIXELS];
    let mut encoded_buffer = [EncodedPixel::default(); BUFFER_PIXELS];

    let chunk_size = BUFFER_PIXELS * bytes_per_pixel;
    for line in data.chunks(chunk_size) {
        debug_assert!(line.len() % bytes_per_pixel == 0);
        let pixels = line.len() / bytes_per_pixel;

        let intermediate = &mut intermediate_buffer[..pixels];
        let encoded = &mut encoded_buffer[..pixels];

        convert_to_rgba_f32(color, line, intermediate);

        process(intermediate, encoded);

        ToLe::to_le(encoded);

        writer.write_all(cast::as_bytes(encoded))?;
    }

    Ok(())
}
fn uncompressed<EncodedPixel, F>(args: Args, f: F) -> Result<(), EncodeError>
where
    EncodedPixel: Default + Copy + ToLe + cast::Castable,
    F: Fn(&[u8], ColorFormat, &mut [EncodedPixel]),
{
    let DecodedArgs {
        data,
        color,
        writer,
        ..
    } = DecodedArgs::from(args)?;
    let bytes_per_pixel = color.bytes_per_pixel() as usize;

    const BUFFER_PIXELS: usize = 512;
    let mut encoded_buffer = [EncodedPixel::default(); BUFFER_PIXELS];

    let chuck_size = BUFFER_PIXELS * bytes_per_pixel;
    for line in data.chunks(chuck_size) {
        debug_assert!(line.len() % bytes_per_pixel == 0);
        let pixels = line.len() / bytes_per_pixel;
        let encoded = &mut encoded_buffer[..pixels];

        f(line, color, encoded);

        ToLe::to_le(encoded);

        writer.write_all(cast::as_bytes(encoded))?;
    }

    Ok(())
}
fn uncompressed_untyped(
    args: Args,
    bytes_per_encoded_pixel: usize,
    f: impl Fn(&[u8], ColorFormat, &mut [u8]),
) -> Result<(), EncodeError> {
    let DecodedArgs {
        data,
        color,
        writer,
        ..
    } = DecodedArgs::from(args)?;
    let bytes_per_pixel = color.bytes_per_pixel() as usize;

    let mut raw_buffer = [0_u32; 1024];
    let encoded_buffer = cast::as_bytes_mut(&mut raw_buffer);

    let chuck_size = encoded_buffer.len() / bytes_per_pixel * bytes_per_pixel;
    for line in data.chunks(chuck_size) {
        debug_assert!(line.len() % bytes_per_pixel == 0);
        let pixels = line.len() / bytes_per_pixel;
        let encoded = &mut encoded_buffer[..pixels * bytes_per_encoded_pixel];

        f(line, color, encoded);

        writer.write_all(encoded)?;
    }

    Ok(())
}

fn copy_directly(args: Args) -> Result<(), EncodeError> {
    let DecodedArgs {
        data,
        color,
        writer,
        ..
    } = DecodedArgs::from(args)?;

    // We can always just write everything directly on LE systems
    // and when the precision is U8
    if cfg!(target_endian = "little") || color.precision == Precision::U8 {
        writer.write_all(data)?;
        return Ok(());
    }

    // We need to convert to LE, so we need to allocate a buffer
    let mut buffer = [0_u8; 4096];
    let chuck_size = buffer.len();

    for chunk in data.chunks(chuck_size) {
        debug_assert!(chunk.len() % color.precision.size() as usize == 0);
        let chunk_buffer = &mut buffer[..chunk.len()];
        chunk_buffer.copy_from_slice(chunk);
        convert_to_le(color.precision, chunk_buffer);
        writer.write_all(chunk_buffer)?;
    }

    Ok(())
}

fn convert_to_le(precision: Precision, buffer: &mut [u8]) {
    match precision {
        Precision::U8 => {}
        Precision::U16 => util::le_to_native_endian_16(buffer),
        Precision::F32 => util::le_to_native_endian_32(buffer),
    }
}

fn simple_color_convert(
    target: ColorFormat,
    snorm: bool,
) -> impl Fn(&[u8], ColorFormat, &mut [u8]) {
    if snorm {
        assert!(matches!(target.precision, Precision::U8 | Precision::U16));
    }

    move |line, color, out| {
        assert!(color.precision == target.precision);

        let from = color.channels;
        let to = target.channels;
        match target.precision {
            Precision::U8 => convert_channels_untyped::<u8>(from, to, line, out),
            Precision::U16 => convert_channels_untyped::<u16>(from, to, line, out),
            Precision::F32 => convert_channels_untyped::<f32>(from, to, line, out),
        }

        if snorm {
            match target.precision {
                Precision::U8 => {
                    out.iter_mut().for_each(|o| *o = s8::from_n8(*o));
                }
                Precision::U16 => {
                    let chunked: &mut [[u8; 2]] =
                        cast::as_array_chunks_mut(out).expect("invalid buffer size");
                    chunked.iter_mut().for_each(|o| {
                        *o = s16::from_n16(u16::from_ne_bytes(*o)).to_ne_bytes();
                    });
                }
                Precision::F32 => unreachable!(),
            }
        }

        convert_to_le(target.precision, out);
    }
}

trait ToLe: Sized {
    fn to_le(buffer: &mut [Self]);
}
impl ToLe for u8 {
    fn to_le(_buffer: &mut [Self]) {}
}
impl ToLe for u16 {
    fn to_le(buffer: &mut [Self]) {
        util::le_to_native_endian_16(cast::as_bytes_mut(buffer));
    }
}
impl ToLe for u32 {
    fn to_le(buffer: &mut [Self]) {
        util::le_to_native_endian_32(cast::as_bytes_mut(buffer));
    }
}
impl ToLe for f32 {
    fn to_le(buffer: &mut [Self]) {
        util::le_to_native_endian_32(cast::as_bytes_mut(buffer));
    }
}
impl<const N: usize, T> ToLe for [T; N]
where
    T: ToLe + cast::Castable,
{
    fn to_le(buffer: &mut [Self]) {
        let flat = cast::as_flattened_mut(buffer);
        T::to_le(flat);
    }
}

macro_rules! color_convert {
    ($target:expr) => {
        UncompressedEncoder {
            color_formats: ColorFormatSet::from_precision($target.precision),
            flags: Flags::exact_for($target.precision),
            encode: |args| {
                uncompressed_untyped(
                    args,
                    $target.bytes_per_pixel() as usize,
                    simple_color_convert($target, false),
                )
            },
        }
    };
    ($target:expr, SNORM) => {
        UncompressedEncoder {
            color_formats: ColorFormatSet::from_precision($target.precision),
            flags: Flags::exact_for($target.precision),
            encode: |args| {
                uncompressed_untyped(
                    args,
                    $target.bytes_per_pixel() as usize,
                    simple_color_convert($target, true),
                )
            },
        }
    };
}

macro_rules! universal {
    ($out:ty, $f:expr) => {{
        fn process_line(line: &[[f32; 4]], out: &mut [$out]) {
            assert!(line.len() == out.len());
            let f = util::closure_types::<[f32; 4], $out, _>($f);
            for (i, o) in line.iter().zip(out.iter_mut()) {
                *o = f(*i);
            }
        }
        UncompressedEncoder {
            color_formats: ColorFormatSet::ALL,
            flags: Flags::empty(),
            encode: |args| uncompressed_universal(args, process_line),
        }
    }};
}

macro_rules! universal_subsample {
    ($block_width:literal, $out:ty, $f:expr) => {{
        fn process_blocks(block: &[[f32; 4]], out: &mut [$out]) {
            process_subsample::<$block_width, $out, _>(block, out, $f);
        }
        UncompressedEncoder {
            color_formats: ColorFormatSet::ALL,
            flags: Flags::empty(),
            encode: |args| uncompressed_universal_subsample(args, $block_width, process_blocks),
        }
    }};
}

pub const R8G8B8_UNORM: &[UncompressedEncoder] = &[
    UncompressedEncoder::copy(ColorFormat::RGB_U8),
    color_convert!(ColorFormat::RGB_U8),
    universal!([u8; 3], |[r, g, b, _]| [r, g, b].map(n8::from_f32)),
];

pub const B8G8R8_UNORM: &[UncompressedEncoder] = &[
    UncompressedEncoder {
        color_formats: ColorFormatSet::U8,
        flags: Flags::EXACT_U8,
        encode: |args| {
            fn process_line(line: &[u8], color: ColorFormat, out: &mut [u8]) {
                assert!(color.precision == Precision::U8);
                convert_channels_untyped::<u8>(color.channels, Channels::Rgb, line, out);

                // swap R and B
                let chunked: &mut [[u8; 3]] =
                    cast::as_array_chunks_mut(out).expect("invalid buffer size");
                chunked.iter_mut().for_each(|p| p.swap(0, 2));
            }

            uncompressed_untyped(args, 3, process_line)
        },
    },
    universal!([u8; 3], |[r, g, b, _]| [b, g, r].map(n8::from_f32)),
];

pub const R8G8B8A8_UNORM: &[UncompressedEncoder] = &[
    UncompressedEncoder::copy(ColorFormat::RGBA_U8),
    color_convert!(ColorFormat::RGBA_U8),
    universal!([u8; 4], |rgba| rgba.map(n8::from_f32)),
];

pub const R8G8B8A8_SNORM: &[UncompressedEncoder] = &[
    color_convert!(ColorFormat::RGBA_U8, SNORM),
    universal!([u8; 4], |rgba| rgba.map(s8::from_uf32)),
];

pub const B8G8R8A8_UNORM: &[UncompressedEncoder] = &[
    UncompressedEncoder {
        color_formats: ColorFormatSet::U8,
        flags: Flags::EXACT_U8,
        encode: |args| {
            fn process_line(line: &[u8], color: ColorFormat, out: &mut [u8]) {
                assert!(color.precision == Precision::U8);
                convert_channels_untyped::<u8>(color.channels, Channels::Rgba, line, out);

                // swap R and B
                let chunked: &mut [[u8; 4]] =
                    cast::as_array_chunks_mut(out).expect("invalid buffer size");
                chunked.iter_mut().for_each(|p| p.swap(0, 2));
            }

            uncompressed_untyped(args, 4, process_line)
        },
    },
    universal!([u8; 4], |[r, g, b, a]| [b, g, r, a].map(n8::from_f32)),
];

pub const B8G8R8X8_UNORM: &[UncompressedEncoder] = &[
    UncompressedEncoder {
        color_formats: ColorFormatSet::U8,
        flags: Flags::EXACT_U8,
        encode: |args| {
            fn process_line(line: &[u8], color: ColorFormat, out: &mut [u8]) {
                assert!(color.precision == Precision::U8);
                convert_channels_untyped::<u8>(color.channels, Channels::Rgba, line, out);

                // swap R and B and set X to 0xFF
                let chunked: &mut [[u8; 4]] =
                    cast::as_array_chunks_mut(out).expect("invalid buffer size");
                chunked.iter_mut().for_each(|p| {
                    p.swap(0, 2);
                    p[3] = 0xFF;
                });
            }

            uncompressed_untyped(args, 4, process_line)
        },
    },
    universal!([u8; 4], |[r, g, b, _]| [
        n8::from_f32(b),
        n8::from_f32(g),
        n8::from_f32(r),
        0xFF
    ]),
];

pub const B5G6R5_UNORM: &[UncompressedEncoder] = &[universal!(u16, |[r, g, b, _]| {
    let r = n5::from_f32(r) as u16;
    let g = n6::from_f32(g) as u16;
    let b = n5::from_f32(b) as u16;
    b | (g << 5) | (r << 11)
})];

pub const B5G5R5A1_UNORM: &[UncompressedEncoder] = &[universal!(u16, |[r, g, b, a]| {
    let r = n5::from_f32(r) as u16;
    let g = n5::from_f32(g) as u16;
    let b = n5::from_f32(b) as u16;
    let a = n1::from_f32(a) as u16;
    b | (g << 5) | (r << 10) | (a << 15)
})];

pub const B4G4R4A4_UNORM: &[UncompressedEncoder] = &[universal!(u16, |[r, g, b, a]| {
    let r = n4::from_f32(r) as u16;
    let g = n4::from_f32(g) as u16;
    let b = n4::from_f32(b) as u16;
    let a = n4::from_f32(a) as u16;
    b | (g << 4) | (r << 8) | (a << 12)
})];

pub const A4B4G4R4_UNORM: &[UncompressedEncoder] = &[universal!(u16, |[r, g, b, a]| {
    let r = n4::from_f32(r) as u16;
    let g = n4::from_f32(g) as u16;
    let b = n4::from_f32(b) as u16;
    let a = n4::from_f32(a) as u16;
    a | (b << 4) | (g << 8) | (r << 12)
})];

pub const R8_UNORM: &[UncompressedEncoder] = &[
    UncompressedEncoder::copy(ColorFormat::GRAYSCALE_U8),
    color_convert!(ColorFormat::GRAYSCALE_U8),
    universal!(u8, |[r, _, _, _]| n8::from_f32(r)),
];

pub const R8_SNORM: &[UncompressedEncoder] = &[
    color_convert!(ColorFormat::GRAYSCALE_U8, SNORM),
    universal!(u8, |[r, _, _, _]| s8::from_uf32(r)),
];

pub const R8G8_UNORM: &[UncompressedEncoder] =
    &[universal!([u8; 2], |[r, g, _, _]| [r, g].map(n8::from_f32)).add_flags(Flags::EXACT_U8)];

pub const R8G8_SNORM: &[UncompressedEncoder] =
    &[universal!([u8; 2], |[r, g, _, _]| [r, g].map(s8::from_uf32)).add_flags(Flags::EXACT_U8)];

pub const A8_UNORM: &[UncompressedEncoder] = &[
    UncompressedEncoder::copy(ColorFormat::ALPHA_U8),
    color_convert!(ColorFormat::ALPHA_U8),
    universal!(u8, |[_, _, _, a]| n8::from_f32(a)),
];

pub const R16_UNORM: &[UncompressedEncoder] = &[
    UncompressedEncoder::copy(ColorFormat::GRAYSCALE_U16),
    color_convert!(ColorFormat::GRAYSCALE_U16),
    universal!(u16, |[r, _, _, _]| n16::from_f32(r)),
];

pub const R16_SNORM: &[UncompressedEncoder] = &[
    color_convert!(ColorFormat::GRAYSCALE_U16, SNORM),
    universal!(u16, |[r, _, _, _]| s16::from_uf32(r)),
];

pub const R16G16_UNORM: &[UncompressedEncoder] =
    &[universal!([u16; 2], |[r, g, _, _]| [r, g].map(n16::from_f32)).add_flags(Flags::EXACT_U16)];

pub const R16G16_SNORM: &[UncompressedEncoder] =
    &[universal!([u16; 2], |[r, g, _, _]| [r, g].map(s16::from_uf32)).add_flags(Flags::EXACT_U16)];

pub const R16G16B16A16_UNORM: &[UncompressedEncoder] = &[
    UncompressedEncoder::copy(ColorFormat::RGBA_U16),
    color_convert!(ColorFormat::RGBA_U16),
    universal!([u16; 4], |rgba| rgba.map(n16::from_f32)),
];

pub const R16G16B16A16_SNORM: &[UncompressedEncoder] = &[
    color_convert!(ColorFormat::RGBA_U16, SNORM),
    universal!([u16; 4], |rgba| rgba.map(s16::from_uf32)),
];

pub const R10G10B10A2_UNORM: &[UncompressedEncoder] = &[universal!(u32, |[r, g, b, a]| {
    let r = n10::from_f32(r) as u32;
    let g = n10::from_f32(g) as u32;
    let b = n10::from_f32(b) as u32;
    let a = n2::from_f32(a) as u32;
    (a << 30) | (b << 20) | (g << 10) | r
})];

pub const R11G11B10_FLOAT: &[UncompressedEncoder] = &[universal!(u32, |[r, g, b, _]| {
    let r11 = fp11::from_f32(r) as u32;
    let g11 = fp11::from_f32(g) as u32;
    let b10 = fp10::from_f32(b) as u32;
    (b10 << 22) | (g11 << 11) | r11
})];

pub const R9G9B9E5_SHAREDEXP: &[UncompressedEncoder] =
    &[
        universal!(u32, |[r, g, b, _]| { rgb9995f::from_f32([r, g, b]) })
            .add_flags(Flags::EXACT_U8),
    ];

pub const R16_FLOAT: &[UncompressedEncoder] =
    &[universal!(u16, |[r, _, _, _]| fp16::from_f32(r)).add_flags(Flags::EXACT_U8)];

pub const R16G16_FLOAT: &[UncompressedEncoder] =
    &[universal!([u16; 2], |[r, g, _, _]| [r, g].map(fp16::from_f32)).add_flags(Flags::EXACT_U8)];

pub const R16G16B16A16_FLOAT: &[UncompressedEncoder] =
    &[universal!([u16; 4], |rgba| rgba.map(fp16::from_f32)).add_flags(Flags::EXACT_U8)];

pub const R32_FLOAT: &[UncompressedEncoder] = &[
    UncompressedEncoder::copy(ColorFormat::GRAYSCALE_F32),
    color_convert!(ColorFormat::GRAYSCALE_F32),
    universal!(f32, |[r, _, _, _]| r),
];

pub const R32G32_FLOAT: &[UncompressedEncoder] =
    &[universal!([f32; 2], |[r, g, _, _]| [r, g]).add_flags(Flags::EXACT_F32)];

pub const R32G32B32_FLOAT: &[UncompressedEncoder] = &[
    UncompressedEncoder::copy(ColorFormat::RGB_F32),
    color_convert!(ColorFormat::RGB_F32),
    universal!([f32; 3], |[r, g, b, _]| [r, g, b]),
];

pub const R32G32B32A32_FLOAT: &[UncompressedEncoder] = &[
    UncompressedEncoder::copy(ColorFormat::RGBA_F32),
    color_convert!(ColorFormat::RGBA_F32),
    universal!([f32; 4], |[r, g, b, a]| [r, g, b, a]),
];

pub const R10G10B10_XR_BIAS_A2_UNORM: &[UncompressedEncoder] = &[universal!(u32, |[r, g, b, a]| {
    let r = xr10::from_f32(r) as u32;
    let g = xr10::from_f32(g) as u32;
    let b = xr10::from_f32(b) as u32;
    let a = n2::from_f32(a) as u32;
    (a << 30) | (b << 20) | (g << 10) | r
})];

pub const AYUV: &[UncompressedEncoder] = &[universal!([u8; 4], |[r, g, b, a]| {
    let [y, u, v] = yuv8::from_rgb_f32([r, g, b]);
    let a = n8::from_f32(a);
    [v, u, y, a]
})];

pub const Y410: &[UncompressedEncoder] = &[universal!(u32, |[r, g, b, a]| {
    let [y, u, v] = yuv10::from_rgb_f32([r, g, b]);
    let a = n2::from_f32(a) as u32;
    (a << 30) | ((v as u32) << 20) | ((y as u32) << 10) | (u as u32)
})];

pub const Y416: &[UncompressedEncoder] = &[universal!([u16; 4], |[r, g, b, a]| {
    let [y, u, v] = yuv16::from_rgb_f32([r, g, b]);
    let a = n16::from_f32(a);
    [u, y, v, a]
})
.add_flags(Flags::EXACT_U8)];

fn to_rgbg([p0, p1]: &[[f32; 4]; 2]) -> [u8; 4] {
    let g0 = n8::from_f32(p0[1]);
    let g1 = n8::from_f32(p1[1]);
    let r = n8::from_f32((p0[0] + p1[0]) * 0.5);
    let b = n8::from_f32((p0[2] + p1[2]) * 0.5);
    [r, g0, b, g1]
}

pub const R8G8_B8G8_UNORM: &[UncompressedEncoder] =
    &[universal_subsample!(2, [u8; 4], to_rgbg).add_flags(Flags::EXACT_U8)];

pub const G8R8_G8B8_UNORM: &[UncompressedEncoder] = &[universal_subsample!(2, [u8; 4], |pair| {
    let [r, g0, b, g1] = to_rgbg(pair);
    [g0, r, g1, b]
})
.add_flags(Flags::EXACT_U8)];

fn to_yuy2([p0, p1]: &[[f32; 4]; 2]) -> [u8; 4] {
    let yuv1 = yuv8::from_rgb_f32([p0[0], p0[1], p0[2]]);
    let yuv2 = yuv8::from_rgb_f32([p1[0], p1[1], p1[2]]);
    let y0 = yuv1[0];
    let y1 = yuv2[0];
    fn pick_mid(a: u8, b: u8) -> u8 {
        let a = a as u16;
        let b = b as u16;
        ((a + b) / 2) as u8
    }
    let u = pick_mid(yuv1[1], yuv2[1]);
    let v = pick_mid(yuv1[2], yuv2[2]);
    [y0, u, y1, v]
}

pub const YUY2: &[UncompressedEncoder] = &[universal_subsample!(2, [u8; 4], to_yuy2)];

pub const UYVY: &[UncompressedEncoder] = &[universal_subsample!(2, [u8; 4], |pair| {
    let [y0, u, y1, v] = to_yuy2(pair);
    [u, y0, v, y1]
})];

fn to_y216([p0, p1]: &[[f32; 4]; 2]) -> [u16; 4] {
    let yuv1 = yuv16::from_rgb_f32([p0[0], p0[1], p0[2]]);
    let yuv2 = yuv16::from_rgb_f32([p1[0], p1[1], p1[2]]);
    let y0 = yuv1[0];
    let y1 = yuv2[0];
    fn pick_mid(a: u16, b: u16) -> u16 {
        let a = a as u32;
        let b = b as u32;
        ((a + b) / 2) as u16
    }
    let u = pick_mid(yuv1[1], yuv2[1]);
    let v = pick_mid(yuv1[2], yuv2[2]);
    [y0, u, y1, v]
}

pub const Y210: &[UncompressedEncoder] = &[universal_subsample!(2, [u16; 4], |pair| to_y216(pair)
    .map(|c| c & 0xFFC0))
.add_flags(Flags::EXACT_U8)];

pub const Y216: &[UncompressedEncoder] =
    &[universal_subsample!(2, [u16; 4], to_y216).add_flags(Flags::EXACT_U8)];

pub const R1_UNORM: &[UncompressedEncoder] = &[universal_subsample!(8, u8, |block| {
    let mut out = 0_u8;
    for (i, &p) in block.iter().enumerate() {
        out |= n1::from_f32(ch::rgba_to_grayscale(p)[0]) << (7 - i);
    }
    out
})];
