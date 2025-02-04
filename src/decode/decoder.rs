use std::io::{Read, Seek};
use std::mem::size_of;

use crate::{
    decode::read_write::for_each_pixel_rect_untyped, Channels, ColorFormat, DecodeError, Precision,
    Rect, Size, TinyEnum, TinySet,
};

use super::{
    adapt::{adapt, UncompressedAdapter},
    read_write::{for_each_pixel_untyped, InOutSize, ProcessPixelsFn},
};

pub(crate) struct DecodeContext {
    pub size: Size,
}

/// This is a silly hack to make [DecodeFn] `const`-compatible on MSRV.
///
/// The issue is that `const fn`s not not allow mutable references. On older
/// Rust versions, this also included multiple references in function pointers.
/// Of course, functions pointers can't be called in `const`, so them having
/// mutable references doesn't matter, but the compiler wasn't smart enough
/// back then. It only looked at types, saw an `&mut` and rejected the code.
///
/// The "fix" is to wrap all mutable references in a struct so that compiler
/// can't see them in the type signature of the function pointer anymore. Truly
/// silly, and thankfully not necessary on never compiler versions.
pub(crate) struct Args<'a, 'b>(pub &'a mut dyn Read, pub &'b mut [u8], pub DecodeContext);

pub(crate) type DecodeFn = fn(args: Args) -> Result<(), DecodeError>;

pub(crate) trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}

pub(crate) struct RArgs<'a, 'b>(
    pub &'a mut dyn ReadSeek,
    pub &'b mut [u8],
    pub usize,
    pub Rect,
    pub DecodeContext,
);

pub(crate) type DecodeRectFn = fn(args: RArgs) -> Result<(), DecodeError>;

#[derive(Debug, Clone)]
pub(crate) struct Decoder {
    pub channels: Channels,
    pub precision: Precision,
    decode_fn: DecodeFn,
    decode_rect_fn: DecodeRectFn,
}
impl Decoder {
    pub const fn new(
        channels: Channels,
        precision: Precision,
        decode_fn: DecodeFn,
        decode_rect_fn: DecodeRectFn,
    ) -> Self {
        Self {
            channels,
            precision,
            decode_fn,
            decode_rect_fn,
        }
    }
}

/// Verifies that the buffer is exactly as long as expected.
fn check_buffer_len(
    size: Size,
    channels: Channels,
    precision: Precision,
    buf: &[u8],
) -> Result<(), DecodeError> {
    // overflow isn't possible here
    let bytes_per_pixel = channels.count() as usize * precision.size() as usize;
    // saturate to usize::MAX on overflow
    let required_bytes = usize::saturating_mul(size.width as usize, size.height as usize)
        .saturating_mul(bytes_per_pixel);

    if buf.len() != required_bytes {
        Err(DecodeError::UnexpectedBufferSize {
            expected: required_bytes,
            actual: buf.len(),
        })
    } else {
        Ok(())
    }
}

/// Verifies that the rect, buffer, and row pitch are all valid.
fn check_rect_buffer_len(
    size: Size,
    rect: Rect,
    channels: Channels,
    precision: Precision,
    buf: &[u8],
    row_pitch: usize,
) -> Result<(), DecodeError> {
    // Check that the rect is within the bounds of the image.
    if !rect.is_within_bounds(size) {
        return Err(DecodeError::RectOutOfBounds);
    }

    // overflow isn't possible here
    let bytes_per_pixel = channels.count() as usize * precision.size() as usize;

    // Check row pitch
    let min_row_pitch = bytes_per_pixel.saturating_mul(rect.width as usize);
    if row_pitch < min_row_pitch {
        return Err(DecodeError::RowPitchTooSmall {
            required_minimum: min_row_pitch,
            actual: row_pitch,
        });
    }

    // Check that the buffer is long enough
    // saturate to usize::MAX on overflow
    let required_bytes = usize::saturating_mul(row_pitch, rect.height as usize);
    if buf.len() < required_bytes {
        return Err(DecodeError::RectBufferTooSmall {
            required_minimum: required_bytes,
            actual: buf.len(),
        });
    }

    Ok(())
}

pub(crate) struct SimpleDecoderList {
    decoders: &'static [Decoder],
    pub native_channels: Channels,
    pub native_precision: Precision,
    pub supported_channels: TinySet<Channels>,
    pub supported_precisions: TinySet<Precision>,
}
impl SimpleDecoderList {
    pub const fn new(decoders: &'static [Decoder]) -> Self {
        assert!(!decoders.is_empty());

        let channels = TinySet::from_raw_unchecked({
            let mut set: u8 = 0;

            let mut i = 0;
            while i < decoders.len() {
                let decoder = &decoders[i];
                set |= 1 << decoder.channels as u8;
                i += 1;
            }
            set
        });
        let precisions = TinySet::from_raw_unchecked({
            let mut set: u8 = 0;

            let mut i = 0;
            while i < decoders.len() {
                let decoder = &decoders[i];
                set |= 1 << decoder.precision as u8;
                i += 1;
            }
            set
        });

        let value = Self {
            decoders,
            native_channels: decoders[0].channels,
            native_precision: decoders[0].precision,
            supported_channels: channels,
            supported_precisions: precisions,
        };
        value.verify();
        value
    }

    pub fn get_decoder(&self, color: ColorFormat) -> Option<Decoder> {
        self.decoders
            .iter()
            .find(|d| d.channels == color.channels && d.precision == color.precision)
            .cloned()
    }

    const fn verify(&self) {
        // 1. The list must be non-empty.
        assert!(!self.decoders.is_empty());

        // 2. No color channel-precision combination may be repeated.
        {
            let mut bitset: u32 = 0;
            let mut i = 0;
            while i < self.decoders.len() {
                let decoder = &self.decoders[i];

                let key = decoder.channels as u32 * Precision::VARIANTS.len() as u32
                    + decoder.precision as u32;
                assert!(key < 32);

                let bit_mask = 1 << key;
                if bitset & bit_mask != 0 {
                    panic!("Repeated color channel-precision combination");
                }
                bitset |= bit_mask;

                i += 1;
            }
        }

        // 3. Color channel-precision combination must be exhaustive.
        let mut channels_bitset: u32 = 0;
        let mut precision_bitset: u32 = 0;
        {
            let mut i = 0;
            while i < self.decoders.len() {
                let decoder = &self.decoders[i];

                channels_bitset |= 1 << decoder.channels as u32;
                precision_bitset |= 1 << decoder.precision as u32;

                i += 1;
            }

            let channels_count = channels_bitset.count_ones();
            let precision_count = precision_bitset.count_ones();
            // the expected number of decoders IF all combinations are present
            let expected = channels_count * precision_count;
            if self.decoders.len() != expected as usize {
                panic!("Missing color channel-precision combination");
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct UncompressedProcessFn {
    pub process_fn: ProcessPixelsFn,
    pub color: ColorFormat,
    in_size: u8,
}
impl UncompressedProcessFn {
    pub const fn new<InPixel, OutPixel>(
        channels: Channels,
        precision: Precision,
        process_fn: ProcessPixelsFn,
    ) -> Self {
        let color = ColorFormat::new(channels, precision);
        assert!(size_of::<OutPixel>() == color.bytes_per_pixel() as usize);

        Self {
            process_fn,
            color,
            in_size: size_of::<InPixel>() as u8,
        }
    }
    fn fn_for(&self, color: ColorFormat) -> (impl Fn(&[u8], &mut [u8]), InOutSize) {
        debug_assert!(self.color.precision == color.precision);

        let from_channels = self.color.channels;
        let to_channels = color.channels;
        let precision = color.precision;
        let process_fn = self.process_fn;

        (
            move |encoded, decoded| {
                adapt(
                    UncompressedAdapter {
                        encoded,
                        decoded,
                        process_fn,
                    },
                    from_channels,
                    to_channels,
                    precision,
                );
            },
            InOutSize {
                in_size: self.in_size,
                out_size: color.bytes_per_pixel(),
            },
        )
    }
}
pub(crate) struct UncompressedDecoders {
    decoders: &'static [UncompressedProcessFn],
}
impl UncompressedDecoders {
    pub const fn new(decoders: &'static [UncompressedProcessFn]) -> Self {
        // TODO: verify
        Self { decoders }
    }

    pub const fn native_channels(&self) -> Channels {
        self.decoders[0].color.channels
    }
    pub const fn native_precision(&self) -> Precision {
        self.decoders[0].color.precision
    }

    pub fn get_process_fn(&self, color: ColorFormat) -> Option<UncompressedProcessFn> {
        self.decoders.iter().find(|d| d.color == color).cloned()
    }
    pub fn get_closest_process_fn(&self, color: ColorFormat) -> Option<UncompressedProcessFn> {
        if let Some(perfect_match) = self.get_process_fn(color) {
            return Some(perfect_match);
        }

        let precision = color.precision;

        // try to find another color format that is efficient and retains a
        // much color information as possible
        let channel_preference: &[Channels] = match color.channels {
            Channels::Grayscale => &[Channels::Rgb, Channels::Rgba],
            Channels::Alpha => &[Channels::Rgba],
            Channels::Rgb => &[Channels::Rgba, Channels::Grayscale],
            Channels::Rgba => &[Channels::Rgb, Channels::Grayscale, Channels::Alpha],
        };
        for &channels in channel_preference {
            let color = ColorFormat::new(channels, precision);
            if let Some(process_fn) = self.get_process_fn(color) {
                return Some(process_fn);
            }
        }

        // we give up. Find any with the same precision
        self.decoders
            .iter()
            .find(|d| d.color.precision == precision)
            .cloned()
    }
}

enum Inner {
    List(SimpleDecoderList),
    Uncompressed(UncompressedDecoders),
}

struct SpecializedDecodeFn {
    decode_fn: DecodeFn,
    color: ColorFormat,
}

pub(crate) struct DecoderSet {
    decoders: Inner,
    optimized: Option<SpecializedDecodeFn>,
}
impl DecoderSet {
    pub const fn new(decoders: &'static [Decoder]) -> Self {
        Self {
            decoders: Inner::List(SimpleDecoderList::new(decoders)),
            optimized: None,
        }
    }
    pub const fn new_uncompressed(decoders: &'static [UncompressedProcessFn]) -> Self {
        Self {
            decoders: Inner::Uncompressed(UncompressedDecoders::new(decoders)),
            optimized: None,
        }
    }
    pub const fn add_specialized(
        self,
        channels: Channels,
        precision: Precision,
        decode_fn: DecodeFn,
    ) -> Self {
        assert!(self.optimized.is_none());
        Self {
            decoders: self.decoders,
            optimized: Some(SpecializedDecodeFn {
                decode_fn,
                color: ColorFormat::new(channels, precision),
            }),
        }
    }

    pub const fn native_channels(&self) -> Channels {
        match &self.decoders {
            Inner::List(list) => list.native_channels,
            Inner::Uncompressed(list) => list.native_channels(),
        }
    }
    pub const fn native_precision(&self) -> Precision {
        match &self.decoders {
            Inner::List(list) => list.native_precision,
            Inner::Uncompressed(list) => list.native_precision(),
        }
    }

    pub const fn supported_channels(&self) -> TinySet<Channels> {
        let all = TinySet::from_raw_unchecked(0b1111);
        match &self.decoders {
            Inner::List(list) => list.supported_channels,
            Inner::Uncompressed(_) => all,
        }
    }
    pub const fn supported_precisions(&self) -> TinySet<Precision> {
        let all = TinySet::from_raw_unchecked(0b111);
        match &self.decoders {
            Inner::List(list) => list.supported_precisions,
            Inner::Uncompressed(_) => all,
        }
    }

    pub fn decode(
        &self,
        color: ColorFormat,
        reader: &mut dyn Read,
        size: Size,
        output: &mut [u8],
    ) -> Result<(), DecodeError> {
        check_buffer_len(size, color.channels, color.precision, output)?;

        // never decode empty images
        if size.is_empty() {
            return Ok(());
        }

        let args = Args(reader, output, DecodeContext { size });
        let unsupported = DecodeError::UnsupportedColorFormat {
            format: crate::SupportedFormat::A8_UNORM, // FIXME:
            color,
        };

        if let Some(optimized) = &self.optimized {
            if optimized.color == color {
                // some decoder sets have specially optimized full-image decoders
                return (optimized.decode_fn)(args);
            }
        }

        match &self.decoders {
            Inner::List(list) => {
                let decoder = list.get_decoder(color).ok_or(unsupported)?;
                (decoder.decode_fn)(args)
            }
            Inner::Uncompressed(list) => {
                let decoder = list.get_closest_process_fn(color).ok_or(unsupported)?;
                let (process_fn, pixel_size) = decoder.fn_for(color);
                for_each_pixel_untyped(reader, output, pixel_size, process_fn)
            }
        }
    }

    pub fn decode_rect(
        &self,
        color: ColorFormat,
        reader: &mut dyn ReadSeek,
        size: Size,
        rect: Rect,
        output: &mut [u8],
        row_pitch: usize,
    ) -> Result<(), DecodeError> {
        check_rect_buffer_len(
            size,
            rect,
            color.channels,
            color.precision,
            output,
            row_pitch,
        )?;

        // never decode empty rects
        if rect.size().is_empty() {
            return Ok(());
        }

        let args = RArgs(reader, output, row_pitch, rect, DecodeContext { size });
        let unsupported = DecodeError::UnsupportedColorFormat {
            format: crate::SupportedFormat::A8_UNORM, // FIXME:
            color,
        };

        match &self.decoders {
            Inner::List(list) => {
                let decoder = list.get_decoder(color).ok_or(unsupported)?;
                (decoder.decode_rect_fn)(args)
            }
            Inner::Uncompressed(list) => {
                let decoder = list.get_closest_process_fn(color).ok_or(unsupported)?;
                let (process_fn, pixel_size) = decoder.fn_for(color);
                for_each_pixel_rect_untyped(
                    reader, output, row_pitch, size, rect, pixel_size, process_fn,
                )
            }
        }
    }
}
