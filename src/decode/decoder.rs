use std::io::{Read, Seek};
use std::mem::size_of;

use crate::ColorFormatSet;
use crate::{
    decode::read_write::for_each_pixel_rect_untyped, Channels, ColorFormat, DecodeError, Precision,
    Rect, Size,
};

use super::read_write::{for_each_pixel_untyped, PixelSize, ProcessPixelsFn};
use super::DecodeOptions;

pub(crate) type DecodeFn = fn(args: Args) -> Result<(), DecodeError>;
pub(crate) type DecodeRectFn = fn(args: RArgs) -> Result<(), DecodeError>;

pub(crate) struct DecodeContext {
    pub color: ColorFormat,
    pub size: Size,
    pub memory_limit: usize,
}
impl DecodeContext {
    pub fn reserve_bytes(&mut self, bytes: usize) -> Result<(), DecodeError> {
        if self.memory_limit < bytes {
            return Err(DecodeError::MemoryLimitExceeded);
        }

        self.memory_limit -= bytes;
        Ok(())
    }
    pub fn alloc<T: Default + Copy>(&mut self, len: usize) -> Result<Box<[T]>, DecodeError> {
        self.reserve_bytes(len * size_of::<T>())?;
        Ok(vec![T::default(); len].into_boxed_slice())
    }
    pub fn alloc_capacity<T: Default + Copy>(&mut self, len: usize) -> Result<Vec<T>, DecodeError> {
        self.reserve_bytes(len * size_of::<T>())?;
        Ok(Vec::with_capacity(len))
    }
}

pub(crate) trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}

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
impl<'a, 'b> Args<'a, 'b> {
    pub fn new(
        reader: &'a mut dyn Read,
        output: &'b mut [u8],
        context: DecodeContext,
    ) -> Result<Self, DecodeError> {
        let required_bytes = context
            .size
            .pixels()
            .saturating_mul(context.color.bytes_per_pixel() as u64)
            .try_into()
            .unwrap_or(usize::MAX);

        if output.len() != required_bytes {
            return Err(DecodeError::UnexpectedBufferSize {
                expected: required_bytes,
            });
        }

        Ok(Self(reader, output, context))
    }

    pub fn color(&self) -> ColorFormat {
        self.2.color
    }
}

pub(crate) struct RArgs<'a, 'b>(
    pub &'a mut dyn ReadSeek,
    pub &'b mut [u8],
    pub usize,
    pub Rect,
    pub DecodeContext,
);
impl<'a, 'b> RArgs<'a, 'b> {
    pub fn new(
        reader: &'a mut dyn ReadSeek,
        output: &'b mut [u8],
        row_pitch: usize,
        rect: Rect,
        context: DecodeContext,
    ) -> Result<Self, DecodeError> {
        // Check that the rect is within the bounds of the image.
        if !rect.is_within_bounds(context.size) {
            return Err(DecodeError::RectOutOfBounds);
        }

        // Check row pitch
        let min_row_pitch = usize::saturating_mul(
            rect.width as usize,
            context.color.bytes_per_pixel() as usize,
        );
        if row_pitch < min_row_pitch {
            return Err(DecodeError::RowPitchTooSmall {
                required_minimum: min_row_pitch,
            });
        }

        // Check that the buffer is long enough
        // saturate to usize::MAX on overflow
        let required_bytes = usize::saturating_mul(row_pitch, rect.height as usize);
        if output.len() < required_bytes {
            return Err(DecodeError::RectBufferTooSmall {
                required_minimum: required_bytes,
            });
        }

        Ok(Self(reader, output, row_pitch, rect, context))
    }

    pub fn color(&self) -> ColorFormat {
        self.4.color
    }
}

/// A macro to iterate over an array in a const context.
macro_rules! const_for {
    ($var:ident, $array:expr, $block:expr) => {{
        let mut i = 0;
        while i < $array.len() {
            let $var = &$array[i];
            {
                $block
            }
            i += 1;
        }
    }};
}

/// Contains decode functions directly. These functions can be used as is.
#[derive(Clone)]
pub(crate) struct DirectDecoder {
    native_color: ColorFormat,
    supported_colors: ColorFormatSet,
    decode_fn: DecodeFn,
    decode_rect_fn: DecodeRectFn,
}
impl DirectDecoder {
    pub const fn new_with_all_channels(
        color: ColorFormat,
        decode_fn: DecodeFn,
        decode_rect_fn: DecodeRectFn,
    ) -> Self {
        Self {
            native_color: color,
            supported_colors: ColorFormatSet::from_precision(color.precision),
            decode_fn,
            decode_rect_fn,
        }
    }
}

/// A decoder for uncompressed pixel formats. This contains only a single
/// [`ProcessPixelsFn`] that can be used for both full images and rects.
#[derive(Clone, Copy)]
pub(crate) struct UncompressedDecoder {
    process_fn: ProcessPixelsFn,
    native_color: ColorFormat,
    pixel_size: PixelSize,
}
impl UncompressedDecoder {
    pub const fn new<InPixel, OutPixel>(color: ColorFormat, process_fn: ProcessPixelsFn) -> Self {
        assert!(size_of::<OutPixel>() == color.bytes_per_pixel() as usize);

        Self {
            process_fn,
            native_color: color,
            pixel_size: PixelSize {
                encoded_size: size_of::<InPixel>() as u8,
                decoded_size: size_of::<OutPixel>() as u8,
            },
        }
    }
}

struct DirectDecoderSet {
    decoders: &'static [DirectDecoder],
}
impl DirectDecoderSet {
    const fn new(decoders: &'static [DirectDecoder]) -> Self {
        assert!(!decoders.is_empty());

        let mut supported_colors = ColorFormatSet::EMPTY;
        const_for!(decoder, decoders, {
            supported_colors = supported_colors.union(decoder.supported_colors);
        });
        debug_assert!(supported_colors.is_all(), "All colors must be supported");

        Self { decoders }
    }

    const fn native_color(&self) -> ColorFormat {
        self.decoders[0].native_color
    }

    fn get_decoder(&self, color: ColorFormat) -> &DirectDecoder {
        // try to find an exact match
        if let Some(decoder) = self.decoders.iter().find(|d| d.native_color == color) {
            return decoder;
        }

        // get any decoders
        self.decoders
            .iter()
            .find(|d| d.supported_colors.contains(color))
            .expect("All color formats should be supported")
    }

    fn decode(&self, args: Args) -> Result<(), DecodeError> {
        let decoder = self.get_decoder(args.color());
        (decoder.decode_fn)(args)
    }
    fn decode_rect(&self, args: RArgs) -> Result<(), DecodeError> {
        let decoder = self.get_decoder(args.color());
        (decoder.decode_rect_fn)(args)
    }
}

struct UncompressedDecoderSet {
    decoders: &'static [UncompressedDecoder],
}
impl UncompressedDecoderSet {
    const fn new(decoders: &'static [UncompressedDecoder]) -> Self {
        #[cfg(debug_assertions)]
        Self::verify(decoders);

        Self { decoders }
    }
    #[cfg(debug_assertions)]
    const fn verify(decoders: &'static [UncompressedDecoder]) {
        // 1. The list must be non-empty.
        assert!(!decoders.is_empty());

        // 2. There should be exactly 3 decoders, one for each precision.
        {
            let mut bitset: u32 = 0;
            const_for!(decoder, decoders, {
                let bit_mask = 1 << decoder.native_color.key();
                if bitset & bit_mask != 0 {
                    panic!("Repeated color channel-precision combination");
                }
                bitset |= bit_mask;
            });
        }

        // 3. All precisions must be present
        {
            let mut precision_bitset: u32 = 0;
            const_for!(decoder, decoders, {
                precision_bitset |= 1 << decoder.native_color.precision as u32;
            });

            let precision_count = precision_bitset.count_ones();
            if precision_count != Precision::COUNT as u32 {
                panic!("Missing color channel-precision combination");
            }
        }
    }

    const fn native_color(&self) -> ColorFormat {
        self.decoders[0].native_color
    }

    fn get_closest_process_fn(&self, color: ColorFormat) -> UncompressedDecoder {
        // Try to find one that matches the native color exactly
        if let Some(process_fn) = self.decoders.iter().find(|d| d.native_color == color) {
            return *process_fn;
        }

        // Find any with the same precision
        if let Some(process_fn) = self
            .decoders
            .iter()
            .find(|d| d.native_color.precision == color.precision)
        {
            return *process_fn;
        }

        unreachable!("This object is invalid, because it should have at least one process function of every precision");
    }

    fn decode(&self, args: Args) -> Result<(), DecodeError> {
        let decoder = self.get_closest_process_fn(args.color());
        debug_assert!(decoder.native_color.precision == args.color().precision);

        for_each_pixel_untyped(
            args.0,
            args.1,
            args.2,
            decoder.native_color,
            decoder.pixel_size,
            decoder.process_fn,
        )
    }
    fn decode_rect(&self, args: RArgs) -> Result<(), DecodeError> {
        let decoder = self.get_closest_process_fn(args.color());
        debug_assert!(decoder.native_color.precision == args.color().precision);

        for_each_pixel_rect_untyped(
            args.0,
            args.1,
            args.2,
            args.4,
            args.3,
            decoder.native_color,
            decoder.pixel_size,
            decoder.process_fn,
        )
    }
}

enum Inner {
    List(DirectDecoderSet),
    Uncompressed(UncompressedDecoderSet),
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
    pub const fn new(decoders: &'static [DirectDecoder]) -> Self {
        Self {
            decoders: Inner::List(DirectDecoderSet::new(decoders)),
            optimized: None,
        }
    }
    pub const fn new_uncompressed(decoders: &'static [UncompressedDecoder]) -> Self {
        Self {
            decoders: Inner::Uncompressed(UncompressedDecoderSet::new(decoders)),
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

    pub const fn native_color(&self) -> ColorFormat {
        match &self.decoders {
            Inner::List(list) => list.native_color(),
            Inner::Uncompressed(list) => list.native_color(),
        }
    }

    pub fn decode(
        &self,
        color: ColorFormat,
        reader: &mut dyn Read,
        size: Size,
        output: &mut [u8],
        options: &DecodeOptions,
    ) -> Result<(), DecodeError> {
        let args = Args::new(
            reader,
            output,
            DecodeContext {
                color,
                size,
                memory_limit: options.memory_limit,
            },
        )?;

        // never decode empty images
        if size.is_empty() {
            return Ok(());
        }

        if let Some(optimized) = &self.optimized {
            if optimized.color == color {
                // some decoder sets have specially optimized full-image decoders
                return (optimized.decode_fn)(args);
            }
        }

        match &self.decoders {
            Inner::List(list) => list.decode(args),
            Inner::Uncompressed(list) => list.decode(args),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn decode_rect(
        &self,
        color: ColorFormat,
        reader: &mut dyn ReadSeek,
        size: Size,
        rect: Rect,
        output: &mut [u8],
        row_pitch: usize,
        options: &DecodeOptions,
    ) -> Result<(), DecodeError> {
        let args = RArgs::new(
            reader,
            output,
            row_pitch,
            rect,
            DecodeContext {
                color,
                size,
                memory_limit: options.memory_limit,
            },
        )?;

        // never decode empty rects
        if rect.size().is_empty() {
            return Ok(());
        }

        match &self.decoders {
            Inner::List(list) => list.decode_rect(args),
            Inner::Uncompressed(list) => list.decode_rect(args),
        }
    }
}
