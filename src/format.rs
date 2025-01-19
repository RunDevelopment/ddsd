use std::io::Read;

use crate::{
    cast, decode::Decoder, detect, DecodeError, DxgiFormat, FourCC, Header, TinyEnum, TinySet,
};

/// The number and semantics of the color channels in a surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Channels {
    /// The image only contains a single (color) channel.
    ///
    /// This (color) channels may be luminosity or one of the RGB channels (typically R).
    Grayscale,
    /// The image contains only alpha values.
    Alpha,
    /// The image contains RGB values.
    Rgb,
    /// The image contains RGBA values.
    Rgba,
}
impl Channels {
    /// Returns the number of channels.
    pub const fn count(&self) -> u8 {
        match self {
            Self::Grayscale | Self::Alpha => 1,
            Self::Rgb => 3,
            Self::Rgba => 4,
        }
    }
}
impl TinyEnum for Channels {
    const VARIANTS: &'static [Self] = &[Self::Grayscale, Self::Alpha, Self::Rgb, Self::Rgba];

    fn bit_mask(self) -> u8 {
        1 << self as u8
    }
}

/// The precision/bit depth of the values in a surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Precision {
    /// 8-bit unsigned integer.
    ///
    /// This represents normalized values in the range `[0, 255]`.
    U8,
    /// 16-bit unsigned integer.
    ///
    /// This represents normalized values in the range `[0, 65535]`.
    U16,
    /// 32-bit floating point.
    ///
    /// Values **might not** be normalized to the range `[0, 1]`.
    F32,
}
impl Precision {
    /// Returns the size of a single value of this precision in bytes.
    pub const fn size(&self) -> u8 {
        match self {
            Self::U8 => 1,
            Self::U16 => 2,
            Self::F32 => 4,
        }
    }
}
impl TinyEnum for Precision {
    const VARIANTS: &'static [Self] = &[Self::U8, Self::U16, Self::F32];

    fn bit_mask(self) -> u8 {
        1 << self as u8
    }
}

/// A color format with a specific number of channels and precision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ColorFormat {
    pub channels: Channels,
    pub precision: Precision,
}
impl ColorFormat {
    pub const fn new(channels: Channels, precision: Precision) -> Self {
        Self {
            channels,
            precision,
        }
    }

    /// The number of bytes per pixel in the decoded surface/output buffer.
    ///
    /// This is calculated as simply `channels.count() * precision.size()`.
    pub const fn bytes_per_pixel(&self) -> u8 {
        self.channels.count() * self.precision.size()
    }
}
impl core::fmt::Display for ColorFormat {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?} {:?}", self.channels, self.precision)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[allow(non_camel_case_types)]
pub enum SupportedFormat {
    // uncompressed formats
    R8G8B8_UNORM,
    B8G8R8_UNORM,
    R8G8B8A8_UNORM,
    R8G8B8A8_SNORM,
    B8G8R8A8_UNORM,
    B8G8R8X8_UNORM,
    B5G6R5_UNORM,
    B5G5R5A1_UNORM,
    B4G4R4A4_UNORM,
    R8_SNORM,
    R8_UNORM,
    R8G8_UNORM,
    R8G8_SNORM,
    A8_UNORM,
    R16_UNORM,
    R16_SNORM,
    R16G16_UNORM,
    R16G16_SNORM,
    R16G16B16A16_UNORM,
    R16G16B16A16_SNORM,
    R10G10B10A2_UNORM,
    R11G11B10_FLOAT,
    R9G9B9E5_SHAREDEXP,
    R16_FLOAT,
    R16G16_FLOAT,
    R16G16B16A16_FLOAT,
    R32_FLOAT,
    R32G32_FLOAT,
    R32G32B32_FLOAT,
    R32G32B32A32_FLOAT,
    R10G10B10_XR_BIAS_A2_UNORM,

    // sub-sampled formats
    R8G8_B8G8_UNORM,
    G8R8_G8B8_UNORM,

    // block compression formats
    BC1_UNORM,
    BC2_UNORM,
    BC3_UNORM,
    BC4_UNORM,
    BC4_SNORM,
    BC5_UNORM,
    BC5_SNORM,
    BC6H_UF16,
    BC6H_SF16,
    BC7_UNORM,
}
impl SupportedFormat {
    /// Returns the format of the surfaces from a DDS header.
    pub fn from_header(header: &Header) -> Result<SupportedFormat, DecodeError> {
        if let Some(dx10_header) = &header.dxt10 {
            // decide based on DXGI format
            detect::dxgi_format_to_supported(dx10_header.dxgi_format)
                .ok_or(DecodeError::UnsupportedDxgiFormat(dx10_header.dxgi_format))
        } else if let Some(four_cc) = header.pixel_format.four_cc {
            // decide based on FourCC
            detect::four_cc_to_supported(four_cc).ok_or(DecodeError::UnsupportedFourCC(four_cc))
        } else {
            // decide based on PixelFormat
            detect::pixel_format_to_supported(&header.pixel_format)
                .ok_or(DecodeError::UnsupportedPixelFormat)
        }
    }
    /// Returns the format of a surface from a DXGI format.
    ///
    /// `None` if the DXGI format is not supported for decoding.
    pub const fn from_dxgi(dxgi: DxgiFormat) -> Option<SupportedFormat> {
        detect::dxgi_format_to_supported(dxgi)
    }
    /// Returns the format of a surface from a FourCC code.
    ///
    /// `None` if the FourCC code is not supported for decoding.
    pub const fn from_four_cc(four_cc: FourCC) -> Option<SupportedFormat> {
        detect::four_cc_to_supported(four_cc)
    }

    /// The number and type of (color) channels in the surface.
    ///
    /// If the channels of a format cannot be accurately described by
    /// [`Channels`], the next larger type is used. For example, a format with
    /// only R and G channels will be described as [`Channels::Rgb`].
    pub const fn channels(&self) -> Channels {
        decoders::get_decoders(*self).main().channels
    }
    /// The precision/bit depth closest to the values in the surface.
    ///
    /// DDS supports formats with various precisions and ranges, and not all of
    /// them can be represented *exactly* by the `Precision` enum. The closest
    /// precision is chosen based on the format's range and encoded bit depth.
    /// It is typically larger than the encoded bit depth.
    ///
    /// E.g. the format `B5G6R5_UNORM` is a 5/6-bit per channel format and the
    /// closest precision is `U8`. While `U8` can closely approximate all
    /// `B5G6R5_UNORM` values, it is not exact. E.g. a 5-bit UNORM value of 11
    /// is 90.48 as an 8-bit UNORM value exactly but will be rounded to 90.
    pub const fn precision(&self) -> Precision {
        decoders::get_decoders(*self).main().precision
    }

    pub const fn color_format(&self) -> ColorFormat {
        ColorFormat::new(self.channels(), self.precision())
    }

    /// A set of all channels this formats supports for decoding.
    ///
    /// This list is guaranteed to be without duplicates and to contain
    /// `self.channels()`.
    pub fn supported_channels(&self) -> TinySet<Channels> {
        decoders::get_decoders(*self).supported_channels
    }
    /// A set of all precisions this formats supports for decoding.
    ///
    /// This list is guaranteed to be without duplicates and to contain
    /// `self.precision()`.
    pub fn supported_precisions(&self) -> TinySet<Precision> {
        decoders::get_decoders(*self).supported_precisions
    }

    /// Returns `true` if this format supports decoding as the given color
    /// format.
    ///
    /// ## Channel and precision combinations
    ///
    /// All color formats that consist of a supported channels type and
    /// supported precision are supported. This means that all combinations
    /// of channel and precisions from `supported_channels()` and
    /// `supported_precisions()` respectively are supported color formats.
    pub fn supports(&self, color: ColorFormat) -> bool {
        self.supported_channels().contains(color.channels)
            && self.supported_precisions().contains(color.precision)
    }

    fn get_decoder(&self, color: ColorFormat) -> Result<&'static Decoder, DecodeError> {
        let decoders = decoders::get_decoders(*self).decoders;
        let found = decoders
            .iter()
            .find(|d| d.channels == color.channels && d.precision == color.precision);

        if let Some(decoder) = found {
            if !decoder.disabled {
                return Ok(decoder);
            }
        }

        Err(DecodeError::UnsupportedColorFormat {
            format: *self,
            color,
            missing_feature: found.is_some(),
        })
    }

    /// Decodes the image data of a surface from the given reader and writes it
    /// to the given output buffer.
    ///
    /// If this format does not support the given channels and precision, an
    /// error is returned. Support can be checked ahead of time with
    /// [`Self::supported_channels`] and [`Self::supported_precisions`].
    ///
    /// It is highly recommended for the output buffer to be aligned to the
    /// given precision to improve performance. E.g. if the precision is `U16`,
    /// the output buffer should be aligned to 2 bytes. As such, using the
    /// `decode_<precision>` methods is recommended.
    ///
    /// ## Output buffer
    ///
    /// The output buffer must be exactly the right size to hold the decoded
    /// image data.
    ///
    /// The size in bytes of the output buffer can be calculated as
    /// `size.pixels() * color.bytes_per_pixel()`. If you are using one of the
    /// `decode_<precision>` methods, the length of the types output buffer is
    /// `size.pixels() * channels.count()`
    ///
    /// ## State of the reader
    ///
    /// The reader is expected to be positioned at the start of the encoded
    /// image data of the current surface.
    ///
    /// If the operation completes successfully, the reader will be positioned
    /// at the end of the encoded image data, meaning that the next byte read
    /// will be the first byte of either the next encoded surface or EOF.
    ///
    /// If the operation fails and returns an error, the position of the reader
    /// remains unchanged.
    ///
    /// ## Panics
    ///
    /// This method will only panic in the given reader panics while reading.
    pub fn decode(
        &self,
        reader: &mut dyn Read,
        size: Size,
        color: ColorFormat,
        output: &mut [u8],
    ) -> Result<(), DecodeError> {
        self.get_decoder(color)?.decode(reader, size, output)
    }

    /// A convenience method to decode with [`Precision::U8`].
    ///
    /// See [`Self::decode`] for more details.
    pub fn decode_u8(
        &self,
        reader: &mut dyn Read,
        size: Size,
        channels: Channels,
        output: &mut [u8],
    ) -> Result<(), DecodeError> {
        self.decode(
            reader,
            size,
            ColorFormat::new(channels, Precision::U8),
            output,
        )
    }
    /// A convenience method to decode with [`Precision::U16`].
    ///
    /// See [`Self::decode`] for more details.
    pub fn decode_u16(
        &self,
        reader: &mut dyn Read,
        size: Size,
        channels: Channels,
        output: &mut [u16],
    ) -> Result<(), DecodeError> {
        self.decode(
            reader,
            size,
            ColorFormat::new(channels, Precision::U16),
            cast::as_bytes_mut(output),
        )
    }
    /// A convenience method to decode with [`Precision::F32`].
    ///
    /// See [`Self::decode`] for more details.
    pub fn decode_f32(
        &self,
        reader: &mut dyn Read,
        size: Size,
        channels: Channels,
        output: &mut [f32],
    ) -> Result<(), DecodeError> {
        self.decode(
            reader,
            size,
            ColorFormat::new(channels, Precision::F32),
            cast::as_bytes_mut(output),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}
impl Size {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }
    pub const fn pixels(&self) -> u64 {
        self.width as u64 * self.height as u64
    }
}
impl From<(u32, u32)> for Size {
    fn from((width, height): (u32, u32)) -> Self {
        Self { width, height }
    }
}

mod decoders {

    use crate::decode::{self, DecodeFn, Decoder, DecoderSet};

    use super::{Channels, Precision, SupportedFormat};

    const noop_decode: DecodeFn = |_| Ok(());

    pub(crate) const fn get_decoders(format: SupportedFormat) -> DecoderSet {
        use Channels::*;
        use Precision::*;

        /// A helper macro to make it easier to define a const array of decoders.
        macro_rules! decoders {
            ($c:ident, $p:ident, $d:expr) => {{
                const DECODER: Decoder = Decoder::new($c, $p, $d);
                const INFO: DecoderSet = DecoderSet::new(&[DECODER]);
                INFO
            }};
        }

        match format {
            // uncompressed formats
            SupportedFormat::R8G8B8_UNORM => decode::R8G8B8_UNORM,
            SupportedFormat::B8G8R8_UNORM => decode::B8G8R8_UNORM,
            SupportedFormat::R8G8B8A8_UNORM => decode::R8G8B8A8_UNORM,
            SupportedFormat::R8G8B8A8_SNORM => decode::R8G8B8A8_SNORM,
            SupportedFormat::B8G8R8A8_UNORM => decode::B8G8R8A8_UNORM,
            SupportedFormat::B8G8R8X8_UNORM => decode::B8G8R8X8_UNORM,
            SupportedFormat::B5G6R5_UNORM => decode::B5G6R5_UNORM,
            SupportedFormat::B5G5R5A1_UNORM => decode::B5G5R5A1_UNORM,
            SupportedFormat::B4G4R4A4_UNORM => decode::B4G4R4A4_UNORM,
            SupportedFormat::R8_SNORM => decode::R8_SNORM,
            SupportedFormat::R8_UNORM => decode::R8_UNORM,
            SupportedFormat::R8G8_UNORM => decode::R8G8_UNORM,
            SupportedFormat::R8G8_SNORM => decode::R8G8_SNORM,
            SupportedFormat::A8_UNORM => decode::A8_UNORM,
            SupportedFormat::R16_UNORM => decode::R16_UNORM,
            SupportedFormat::R16_SNORM => decode::R16_SNORM,
            SupportedFormat::R16G16_UNORM => decode::R16G16_UNORM,
            SupportedFormat::R16G16_SNORM => decode::R16G16_SNORM,
            SupportedFormat::R16G16B16A16_UNORM => decode::R16G16B16A16_UNORM,
            SupportedFormat::R16G16B16A16_SNORM => decode::R16G16B16A16_SNORM,
            SupportedFormat::R10G10B10A2_UNORM => decode::R10G10B10A2_UNORM,
            SupportedFormat::R11G11B10_FLOAT => decode::R11G11B10_FLOAT,
            SupportedFormat::R9G9B9E5_SHAREDEXP => decode::R9G9B9E5_SHAREDEXP,
            SupportedFormat::R16_FLOAT => decode::R16_FLOAT,
            SupportedFormat::R16G16_FLOAT => decode::R16G16_FLOAT,
            SupportedFormat::R16G16B16A16_FLOAT => decode::R16G16B16A16_FLOAT,
            SupportedFormat::R32_FLOAT => decode::R32_FLOAT,
            SupportedFormat::R32G32_FLOAT => decode::R32G32_FLOAT,
            SupportedFormat::R32G32B32_FLOAT => decode::R32G32B32_FLOAT,
            SupportedFormat::R32G32B32A32_FLOAT => decode::R32G32B32A32_FLOAT,
            SupportedFormat::R10G10B10_XR_BIAS_A2_UNORM => decode::R10G10B10_XR_BIAS_A2_UNORM,

            // sub-sampled formats
            SupportedFormat::R8G8_B8G8_UNORM => decode::R8G8_B8G8_UNORM,
            SupportedFormat::G8R8_G8B8_UNORM => decode::G8R8_G8B8_UNORM,

            // block compression formats
            SupportedFormat::BC1_UNORM => decode::BC1_UNORM,
            SupportedFormat::BC2_UNORM => decode::BC2_UNORM,
            SupportedFormat::BC3_UNORM => decode::BC3_UNORM,
            SupportedFormat::BC4_UNORM => decode::BC4_UNORM,
            SupportedFormat::BC4_SNORM => decode::BC4_SNORM,
            SupportedFormat::BC5_UNORM => decode::BC5_UNORM,
            SupportedFormat::BC5_SNORM => decode::BC5_SNORM,
            SupportedFormat::BC6H_UF16 => decoders!(Rgb, F32, noop_decode),
            SupportedFormat::BC6H_SF16 => decoders!(Rgb, F32, noop_decode),
            SupportedFormat::BC7_UNORM => decoders!(Rgb, U8, noop_decode),
        }
    }
}
