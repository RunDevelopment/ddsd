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
    /// The number of different precisions.
    pub(crate) const COUNT: usize = 3;

    /// Returns the size of a single value of this precision in bytes.
    pub const fn size(&self) -> u8 {
        match self {
            Self::U8 => 1,
            Self::U16 => 2,
            Self::F32 => 4,
        }
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

    /// Returns a unique key for this color format.
    ///
    /// The key is guaranteed to be less than 32.
    pub(crate) const fn key(&self) -> u8 {
        self.channels as u8 * Precision::COUNT as u8 + self.precision as u8
    }
}
impl core::fmt::Display for ColorFormat {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?} {:?}", self.channels, self.precision)
    }
}
