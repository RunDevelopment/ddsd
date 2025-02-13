#![forbid(unsafe_code)]

mod cast;
mod decode;
mod detect;
mod error;
mod format;
mod header;
mod layout;
mod pixel;
mod util;

use std::io::Read;

pub use error::*;
pub use format::*;
pub use header::*;
pub use layout::*;
pub use pixel::*;

/// Additional options for the DDS decoder specifying how to read and interpret
/// the header.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Options {
    /// Whether magic bytes should be skipped when reading the header.
    ///
    /// DDS files typically start with the magic bytes `"DDS "`. By default, the
    /// decoder will check for these bytes and error if they are not present.
    ///
    /// If this is set to `true`, the decoder assume that the magic bytes are
    /// not present and immediately start reading the header. This can be used
    /// to read DDS files without magic bytes.
    ///
    /// Defaults to `false`.
    pub skip_magic_bytes: bool,

    /// The maximum allowed value of the `array_size` field in the header.
    ///
    /// DDS files support texture arrays and the `array_size` field denotes the
    /// number of textures in the array. The only exception for this are cube
    /// maps where `array_size` denotes the number of cube maps instead, meaning
    /// that the DDS file will contain `array_size * 6` textures (6 faces per
    /// cube map).
    ///
    /// Since `array_size` is defined by the file, it is possible for a
    /// malicious or corrupted file to contain a very large value. For security
    /// reasons, this option can be used to limit the maximum allowed value.
    ///
    /// To disable this limit, set this to `u32::MAX`.
    ///
    /// Defaults to `4096`.
    pub max_array_size: u32,

    /// Whether to allow certain invalid DDS files to be read.
    ///
    /// Certain older software may generate DDS files that do not strictly
    /// adhere to the DDS specification and may contain invalid values in the
    /// header. By default, the decoder will reject such files.
    ///
    /// If this option is set to `true`, the decoder will (1) ignore invalid
    /// header values that would otherwise cause the decoder to reject the file
    /// and (2) attempt to fix the header to read the file correctly. To fix the
    /// header, [`Options::file_len`] must be provided.
    ///
    /// Defaults to `false`.
    pub permissive: bool,

    /// The length of the file in bytes.
    ///
    /// This length includes the magic bytes, header, and data section. Even if
    /// [`Options::skip_magic_bytes`] is set to `true`, the length must include
    /// the magic bytes.
    ///
    /// The purpose of this option is to provide more information, which enables
    /// the decoder to read certain invalid DDS files if [`Options::permissive`]
    /// is set to `true`. If [`Options::permissive`] is set to `false`, this
    /// option will be ignored.
    ///
    /// If this option is set incorrectly (i.e. the length is not equal to the
    /// actual length of the file), the decoder may fail to read certain
    /// invalid DDS files, either rejecting them or reading their contents
    /// incorrectly. Valid DDS files are not affected by this option.
    ///
    /// Defaults to `None`.
    pub file_len: Option<u64>,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            skip_magic_bytes: false,
            max_array_size: 4096,
            permissive: false,
            file_len: None,
        }
    }
}

pub struct DdsDecoder {
    header: Header,
    format: SupportedFormat,
    layout: DataLayout,
}

impl DdsDecoder {
    /// Creates a new decoder by reading the header from the given reader.
    ///
    /// This is equivalent to calling `Decoder::new_with(r, Options::default())`.
    /// See [`Self::new_with`] for more details.
    pub fn new<R: Read>(r: &mut R) -> Result<Self, DecodeError> {
        Self::new_with(r, &Options::default())
    }
    /// Creates a new decoder with the given options by reading the header from the given reader.
    ///
    /// If this operations succeeds, the given reader will be positioned at the start of the data
    /// section. All offsets in [`DataLayout`] are relative to this position.
    pub fn new_with<R: Read>(r: &mut R, options: &Options) -> Result<Self, DecodeError> {
        Self::from_header_with(Header::read(r, options)?, options)
    }

    pub fn from_header(header: Header) -> Result<Self, DecodeError> {
        Self::from_header_with(header, &Options::default())
    }
    pub fn from_header_with(mut header: Header, options: &Options) -> Result<Self, DecodeError> {
        // enforce `array_size` limit
        if let Some(dxt10) = &header.dxt10 {
            if dxt10.array_size > options.max_array_size {
                return Err(DecodeError::ArraySizeTooBig(dxt10.array_size));
            }
        }

        // detect format
        let format = SupportedFormat::from_header(&header)?;

        // data layout
        let pixel_info = format.into();
        let mut layout = DataLayout::from_header_with(&header, pixel_info)?;

        // try to fix invalid headers
        if options.permissive {
            if let Some(expected_data_len) = get_expected_data_len(&header, options) {
                fix(&mut header, &mut layout, pixel_info, expected_data_len);
            }
        }

        Ok(Self {
            header,
            format,
            layout,
        })
    }

    pub fn header(&self) -> &Header {
        &self.header
    }
    pub fn format(&self) -> SupportedFormat {
        self.format
    }
    pub fn layout(&self) -> &DataLayout {
        &self.layout
    }

    /// Whether the texture is in sRGB color space.
    ///
    /// This can only be `true` for DX10+ DDS files. Legacy (DX9) formats cannot
    /// specify the color space and are assumed to be linear.
    pub fn is_srgb(&self) -> bool {
        if let Some(dx10) = &self.header.dxt10 {
            dx10.dxgi_format.is_srgb()
        } else {
            false
        }
    }
}

fn get_expected_data_len(header: &Header, options: &Options) -> Option<u64> {
    let non_data = Header::MAGIC.len()
        + Header::SIZE
        + if header.dxt10.is_some() {
            HeaderDxt10::SIZE
        } else {
            0
        };

    options.file_len?.checked_sub(non_data as u64)
}

fn fix(
    header: &mut Header,
    layout: &mut DataLayout,
    pixel_info: PixelInfo,
    expected_data_len: u64,
) {
    if layout.data_len() == expected_data_len {
        // the data layout is already correct
        return;
    }

    if let Some(dx10) = &header.dxt10 {
        // Some DDS files containing a single cube map have array_size set to 6.
        // This is incorrect and likely stems from an incorrect MS DDS docs example.
        // https://github.com/MicrosoftDocs/win32/pull/1970
        if dx10.array_size == 6
            && dx10.resource_dimension == ResourceDimension::Texture2D
            && layout.data_len() / 6 == expected_data_len
            && layout.texture_array().map_or(false, |array| {
                array.kind() == TextureArrayKind::CubeMaps && array.len() == 36
            })
        {
            let mut new_header = header.clone();
            new_header.dxt10.as_mut().unwrap().array_size = 1;

            if let Ok(new_layout) = DataLayout::from_header_with(&new_header, pixel_info) {
                if new_layout.data_len() == expected_data_len {
                    *header = new_header;
                    *layout = new_layout;
                    return;
                }
            }
        }

        // Some DX10 writers set array_size=0 for "arrays" with one element.
        // https://github.com/microsoft/DirectXTex/pull/490
        if dx10.array_size == 0 {
            let mut new_header = header.clone();
            new_header.dxt10.as_mut().unwrap().array_size = 1;

            if let Ok(new_layout) = DataLayout::from_header_with(&new_header, pixel_info) {
                if new_layout.data_len() == expected_data_len {
                    *header = new_header;
                    *layout = new_layout;
                    return;
                }
            }
        }
    }

    // sadly, we couldn't fix it
}
