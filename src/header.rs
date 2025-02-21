use crate::{
    cast,
    util::{self, read_u32_le_array},
    HeaderError, Options, PixelInfo, Size,
};
use bitflags::bitflags;
use std::{
    io::{Read, Write},
    num::NonZeroU32,
};

/// An unparsed DDS header without magic bytes.
///
/// See [`Header`] for a parsed version.
///
/// See https://learn.microsoft.com/en-us/windows/win32/direct3ddds/dds-header
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RawHeader {
    /// Size of structure. This member must be set to 124.
    pub size: u32,
    /// Flags to indicate which members contain valid data.
    pub flags: DdsFlags,
    pub height: u32,
    pub width: u32,
    /// The pitch or number of bytes per scan line in an uncompressed texture;
    /// the total number of bytes in the top level texture for a compressed texture.
    pub pitch_or_linear_size: u32,
    /// Depth of a volume texture (in pixels), otherwise unused.
    pub depth: u32,
    /// Number of mipmap levels, otherwise unused.
    pub mipmap_count: u32,
    pub reserved1: [u32; 11],
    pub pixel_format: RawPixelFormat,
    pub caps: DdsCaps,
    pub caps2: DdsCaps2,
    pub caps3: u32,
    pub caps4: u32,
    pub reserved2: u32,
    pub dx10: Option<RawDx10Header>,
}
/// An unparsed DDS pixel format.
///
/// See [`PixelFormat`] for a parsed version.
///
/// See https://learn.microsoft.com/en-us/windows/win32/direct3ddds/dds-pixelformat
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RawPixelFormat {
    /// Structure size; set to 32 (bytes).
    pub size: u32,
    /// Values which indicate what type of data is in the surface.
    pub flags: PixelFormatFlags,
    pub four_cc: FourCC,
    pub rgb_bit_count: u32,
    pub r_bit_mask: u32,
    pub g_bit_mask: u32,
    pub b_bit_mask: u32,
    pub a_bit_mask: u32,
}
/// An unparsed DDS DX10 header.
///
/// See [`Dx10Header`] for a parsed version.
///
/// See https://learn.microsoft.com/en-us/windows/win32/direct3ddds/dds-header-dxt10
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RawDx10Header {
    pub dxgi_format: u32,
    pub resource_dimension: u32,
    pub misc_flag: MiscFlags,
    pub array_size: u32,
    pub misc_flags2: u32,
}

impl RawHeader {
    /// Reads the raw header without magic bytes from a reader.
    pub fn read<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        let mut buffer: [u32; Header::INTS] = Default::default();
        read_u32_le_array(reader, &mut buffer)?;

        let mut header: Self = Self {
            size: buffer[0],
            flags: DdsFlags::from_bits_retain(buffer[1]),
            height: buffer[2],
            width: buffer[3],
            pitch_or_linear_size: buffer[4],
            depth: buffer[5],
            mipmap_count: buffer[6],
            reserved1: [
                buffer[7], buffer[8], buffer[9], buffer[10], buffer[11], buffer[12], buffer[13],
                buffer[14], buffer[15], buffer[16], buffer[17],
            ],
            pixel_format: RawPixelFormat {
                size: buffer[18],
                flags: PixelFormatFlags::from_bits_retain(buffer[19]),
                four_cc: FourCC(buffer[20]),
                rgb_bit_count: buffer[21],
                r_bit_mask: buffer[22],
                g_bit_mask: buffer[23],
                b_bit_mask: buffer[24],
                a_bit_mask: buffer[25],
            },
            caps: DdsCaps::from_bits_retain(buffer[26]),
            caps2: DdsCaps2::from_bits_retain(buffer[27]),
            caps3: buffer[28],
            caps4: buffer[29],
            reserved2: buffer[30],
            dx10: None,
        };

        if header.pixel_format.flags.contains(PixelFormatFlags::FOURCC)
            && header.pixel_format.four_cc == FourCC::DX10
        {
            let buffer = &mut buffer[..5];
            read_u32_le_array(reader, buffer)?;
            header.dx10 = Some(RawDx10Header {
                dxgi_format: buffer[0],
                resource_dimension: buffer[1],
                misc_flag: MiscFlags::from_bits_retain(buffer[2]),
                array_size: buffer[3],
                misc_flags2: buffer[4],
            });
        }

        Ok(header)
    }

    /// Write the raw header without magic bytes to a writer.
    pub fn write<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let mut buffer: [u32; 36] = [
            self.size,
            self.flags.bits(),
            self.height,
            self.width,
            self.pitch_or_linear_size,
            self.depth,
            self.mipmap_count,
            self.reserved1[0],
            self.reserved1[1],
            self.reserved1[2],
            self.reserved1[3],
            self.reserved1[4],
            self.reserved1[5],
            self.reserved1[6],
            self.reserved1[7],
            self.reserved1[8],
            self.reserved1[9],
            self.reserved1[10],
            self.pixel_format.size,
            self.pixel_format.flags.bits(),
            self.pixel_format.four_cc.0,
            self.pixel_format.rgb_bit_count,
            self.pixel_format.r_bit_mask,
            self.pixel_format.g_bit_mask,
            self.pixel_format.b_bit_mask,
            self.pixel_format.a_bit_mask,
            self.caps.bits(),
            self.caps2.bits(),
            self.caps3,
            self.caps4,
            self.reserved2,
            // fill in the DXT10 header later
            0,
            0,
            0,
            0,
            0,
        ];
        let selection = if let Some(dx10) = &self.dx10 {
            buffer[31] = dx10.dxgi_format;
            buffer[32] = dx10.resource_dimension;
            buffer[33] = dx10.misc_flag.bits();
            buffer[34] = dx10.array_size;
            buffer[35] = dx10.misc_flags2;
            &mut buffer[..]
        } else {
            &mut buffer[..31]
        };
        let bytes = cast::as_bytes_mut(selection);
        util::le_to_native_endian_32(bytes);
        writer.write_all(bytes)?;
        Ok(())
    }
}

impl RawPixelFormat {
    fn new_four_cc(four_cc: FourCC) -> RawPixelFormat {
        Self {
            size: 32,
            flags: PixelFormatFlags::FOURCC,
            four_cc,
            rgb_bit_count: 0,
            r_bit_mask: 0,
            g_bit_mask: 0,
            b_bit_mask: 0,
            a_bit_mask: 0,
        }
    }
}

/// The DDS header and the DX10 extension header if any.
///
/// This structure contains parsed data. Using by the decoder.
///
/// https://learn.microsoft.com/en-us/windows/win32/direct3ddds/dds-header
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Header {
    /// Surface height (in pixels).
    pub height: u32,
    /// Surface width (in pixels).
    pub width: u32,
    /// Depth of a volume texture (in pixels).
    pub depth: Option<u32>,
    /// Number of mipmap levels.
    pub mipmap_count: NonZeroU32,
    /// Additional detail about the surfaces stored.
    pub caps2: DdsCaps2,
    pub format: PixelFormat,
}

impl Header {
    pub const fn dx10(&self) -> Option<&Dx10Header> {
        match &self.format {
            PixelFormat::Dx10(dx10) => Some(dx10),
            _ => None,
        }
    }
    pub const fn dx10_mut(&mut self) -> Option<&mut Dx10Header> {
        match &mut self.format {
            PixelFormat::Dx10(dx10) => Some(dx10),
            _ => None,
        }
    }

    /// Returns the size of the header (including the DX10 header extension if
    /// any) in bytes. This does **not** include the magic bytes at the start
    /// of the file.
    ///
    /// This is useful for calculating the offset to the pixel data.
    ///
    /// The returned value will be 144 for DX10 DDS files and 124 for legacy
    /// files.
    pub const fn byte_len(&self) -> usize {
        let mut size = Header::SIZE;
        if matches!(self.format, PixelFormat::Dx10(_)) {
            size += Dx10Header::SIZE;
        }
        size
    }

    pub const fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }

    /// Creates a new header for DX10 texture 2D with the given dimensions and
    /// format.
    ///
    /// The mipmap count is set to 1 and the alpha mode is set to unknown.
    pub const fn new_image(width: u32, height: u32, format: DxgiFormat) -> Self {
        Self {
            height,
            width,
            depth: None,
            mipmap_count: NonZeroU32::new(1).unwrap(),
            format: PixelFormat::Dx10(Dx10Header {
                dxgi_format: format,
                resource_dimension: ResourceDimension::Texture2D,
                misc_flag: MiscFlags::empty(),
                array_size: 1,
                misc_flags2: MiscFlags2::ALPHA_MODE_UNKNOWN,
            }),
            caps2: DdsCaps2::empty(),
        }
    }
    /// Creates a new header for DX10 texture 3D with the given dimensions and
    /// format.
    ///
    /// The mipmap count is set to 1 and the alpha mode is set to unknown.
    pub const fn new_volume(width: u32, height: u32, depth: u32, format: DxgiFormat) -> Self {
        Self {
            height,
            width,
            depth: Some(depth),
            mipmap_count: NonZeroU32::new(1).unwrap(),
            format: PixelFormat::Dx10(Dx10Header {
                dxgi_format: format,
                resource_dimension: ResourceDimension::Texture3D,
                misc_flag: MiscFlags::empty(),
                array_size: 1,
                misc_flags2: MiscFlags2::ALPHA_MODE_UNKNOWN,
            }),
            caps2: DdsCaps2::VOLUME,
        }
    }
    /// Creates a new header for DX10 cube map with the given dimensions and
    /// format.
    ///
    /// The mipmap count is set to 1 and the alpha mode is set to unknown.
    pub const fn new_cube_map(width: u32, height: u32, format: DxgiFormat) -> Self {
        Self {
            height,
            width,
            depth: None,
            mipmap_count: NonZeroU32::new(1).unwrap(),
            format: PixelFormat::Dx10(Dx10Header {
                dxgi_format: format,
                resource_dimension: ResourceDimension::Texture2D,
                misc_flag: MiscFlags::TEXTURE_CUBE,
                array_size: 1,
                misc_flags2: MiscFlags2::ALPHA_MODE_UNKNOWN,
            }),
            caps2: DdsCaps2::CUBE_MAP.union(DdsCaps2::CUBE_MAP_ALL_FACES),
        }
    }

    pub(crate) const SIZE: usize = 124;
    pub(crate) const INTS: usize = Self::SIZE / 4;

    /// The magic bytes (`'DDS '`) at the start of every DDS file.
    pub const MAGIC: [u8; 4] = *b"DDS ";

    /// The magic bytes `'DDS '` are at the start of every DDS file. This
    /// function reads the magic bytes and returns `Ok` if they are correct.
    ///
    /// See [`Header::MAGIC`] for the expected magic bytes.
    pub fn read_magic<R: Read>(reader: &mut R) -> Result<(), HeaderError> {
        let mut buffer = [0; 4];
        reader.read_exact(&mut buffer)?;

        if buffer != Self::MAGIC {
            return Err(HeaderError::InvalidMagicBytes(buffer));
        }

        Ok(())
    }

    /// Reads the header without magic bytes from a reader.
    ///
    /// If the header is read successfully, the reader will be at the start of the pixel data.
    pub fn read<R: Read>(reader: &mut R, options: &Options) -> Result<Self, HeaderError> {
        if !options.skip_magic_bytes {
            Self::read_magic(reader)?;
        }

        let raw = RawHeader::read(reader)?;
        Self::from_raw(&raw, options)
    }

    pub fn from_raw(raw: &RawHeader, options: &Options) -> Result<Self, HeaderError> {
        // verify header size
        if raw.size != Self::SIZE as u32 {
            if options.permissive && raw.size == 24 {
                // Some DDS files from the game Stalker 2 have their header size
                // set to 24 instead of 124. This is likely a typo in the source
                // code from the DDS encoder they used.
                // https://github.com/microsoft/DirectXTex/issues/399
            } else {
                return Err(HeaderError::InvalidHeaderSize(raw.size));
            }
        }

        let flags = raw.flags;
        let height = raw.height;
        let width = raw.width;
        let depth = if flags.contains(DdsFlags::DEPTH) {
            Some(raw.depth)
        } else {
            None
        };

        let mipmap_count = if flags.contains(DdsFlags::MIPMAP_COUNT)
            || raw.caps.contains(DdsCaps::COMPLEX)
            || raw.caps.contains(DdsCaps::MIPMAP)
        {
            raw.mipmap_count
        } else {
            1
        };
        let mipmap_count = NonZeroU32::new(mipmap_count.max(1)).unwrap();

        let format = PixelFormat::from_raw(raw, options)?;

        Ok(Self {
            height,
            width,
            depth,
            mipmap_count,
            format,
            caps2: raw.caps2,
        })
    }

    pub fn to_raw(&self) -> RawHeader {
        let mut flags = DdsFlags::REQUIRED | DdsFlags::MIPMAP_COUNT;
        let mut caps = DdsCaps::REQUIRED;

        if self.mipmap_count.get() > 1 {
            caps |= DdsCaps::MIPMAP | DdsCaps::COMPLEX;
        }
        if self.depth.is_some() {
            flags |= DdsFlags::DEPTH;
        }

        // We can only calculate the pitch or linear size if we know the byte
        // size and layout of the pixel data.
        let mut pitch_or_linear_size = 0;
        if let Ok(pixel_info) = PixelInfo::from_header(self) {
            if let PixelInfo::Fixed { bytes_per_pixel } = pixel_info {
                let pitch = self.width.checked_mul(bytes_per_pixel as u32);
                if let Some(pitch) = pitch {
                    pitch_or_linear_size = pitch;
                    flags |= DdsFlags::PITCH;
                }
            } else {
                let linear_size: Option<u32> = pixel_info
                    .surface_bytes(Size::new(self.width, self.height))
                    .and_then(|size| size.try_into().ok());
                if let Some(linear_size) = linear_size {
                    pitch_or_linear_size = linear_size;
                    flags |= DdsFlags::LINEAR_SIZE;
                }
            }
        }

        let (pixel_format, dx10) = match &self.format {
            PixelFormat::FourCC(four_cc) => (RawPixelFormat::new_four_cc(*four_cc), None),
            PixelFormat::Mask(mask_pixel_format) => (
                RawPixelFormat {
                    size: 32,
                    flags: mask_pixel_format.flags,
                    four_cc: FourCC::NONE,
                    rgb_bit_count: mask_pixel_format.rgb_bit_count,
                    r_bit_mask: mask_pixel_format.r_bit_mask,
                    g_bit_mask: mask_pixel_format.g_bit_mask,
                    b_bit_mask: mask_pixel_format.b_bit_mask,
                    a_bit_mask: mask_pixel_format.a_bit_mask,
                },
                None,
            ),
            PixelFormat::Dx10(dx10_header) => (
                RawPixelFormat::new_four_cc(FourCC::DX10),
                Some(RawDx10Header {
                    dxgi_format: dx10_header.dxgi_format.into(),
                    resource_dimension: dx10_header.resource_dimension.into(),
                    misc_flag: dx10_header.misc_flag,
                    array_size: dx10_header.array_size,
                    misc_flags2: dx10_header.misc_flags2.bits(),
                }),
            ),
        };

        RawHeader {
            size: 124,
            flags,
            height: self.height,
            width: self.width,
            pitch_or_linear_size,
            depth: self.depth.unwrap_or(1),
            mipmap_count: self.mipmap_count.get(),
            reserved1: [0; 11],
            pixel_format,
            caps,
            caps2: self.caps2,
            caps3: 0,
            caps4: 0,
            reserved2: 0,
            dx10,
        }
    }
}

/// A combined pixel format and DX10 header.
///
/// DDS files define their pixel format either with a (legacy) `DDS_PIXELFORMAT`
/// structure or with a `DXGI_FORMAT` from the Direct3D 10 and later APIs. This
/// enum represents all cases in a single type.
///
/// https://learn.microsoft.com/en-us/windows/win32/direct3ddds/dds-pixelformat
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PixelFormat {
    FourCC(FourCC),
    Mask(MaskPixelFormat),
    Dx10(Dx10Header),
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MaskPixelFormat {
    /// Values which indicate what type of data is in the surface.
    pub flags: PixelFormatFlags,
    /// Number of bits in an RGB (possibly including alpha) format. Valid when dwFlags includes DDPF_RGB, DDPF_LUMINANCE, or DDPF_YUV.
    pub rgb_bit_count: u32,
    /// Red (or luminance or Y) mask for reading color data. For instance, given the A8R8G8B8 format, the red mask would be 0x00ff0000.
    pub r_bit_mask: u32,
    /// Green (or U) mask for reading color data. For instance, given the A8R8G8B8 format, the green mask would be 0x0000ff00.
    pub g_bit_mask: u32,
    /// Blue (or V) mask for reading color data. For instance, given the A8R8G8B8 format, the blue mask would be 0x000000ff.
    pub b_bit_mask: u32,
    /// Alpha mask for reading alpha data. dwFlags must include DDPF_ALPHAPIXELS or DDPF_ALPHA. For instance, given the A8R8G8B8 format, the alpha mask would be 0xff000000.
    pub a_bit_mask: u32,
}
impl PixelFormat {
    const SIZE: u32 = 32;

    fn from_raw(raw: &RawHeader, options: &Options) -> Result<Self, HeaderError> {
        let size = raw.pixel_format.size;
        if size != Self::SIZE {
            if options.permissive && size == 0 {
                // Some DDS files have their pixel format size set to 0.
                // https://github.com/microsoft/DirectXTex/issues/392
            } else if options.permissive && size == 24 {
                // Some DDS files from the game Flat Out 2 have their pixel
                // format size set to 24 instead of 32. This is likely a bug in
                // the program that created the DDS files.
                // https://github.com/microsoft/DirectXTex/issues/392
            } else {
                return Err(HeaderError::InvalidPixelFormatSize(size));
            }
        }

        if let Some(dx10) = &raw.dx10 {
            let dx10 = Dx10Header::from_raw(dx10, options)?;
            return Ok(Self::Dx10(dx10));
        }

        let mut flags = raw.pixel_format.flags;
        let four_cc = raw.pixel_format.four_cc;
        let rgb_bit_count = raw.pixel_format.rgb_bit_count;

        if options.permissive
            && rgb_bit_count == 0
            && four_cc != FourCC::NONE
            && !flags.contains(PixelFormatFlags::FOURCC)
        {
            // Some old DDS files from Unreal Tournament 2004 have no flags set,
            // an rgb bit count of 0, and use four CC. These files are invalid
            // and format detection will fail for them, so we need to fix the
            // header here. Since those files do use four CC, we just set the
            // missing flag.
            // https://github.com/microsoft/DirectXTex/pull/371
            flags |= PixelFormatFlags::FOURCC;
        }

        if flags.contains(PixelFormatFlags::FOURCC) {
            return Ok(Self::FourCC(four_cc));
        };

        Ok(Self::Mask(MaskPixelFormat {
            flags,
            rgb_bit_count,
            r_bit_mask: raw.pixel_format.r_bit_mask,
            g_bit_mask: raw.pixel_format.g_bit_mask,
            b_bit_mask: raw.pixel_format.b_bit_mask,
            a_bit_mask: raw.pixel_format.a_bit_mask,
        }))
    }
}

/// DDS header extension to handle resource arrays, DXGI pixel formats that don't map to the legacy Microsoft DirectDraw pixel format structures, and additional metadata.
///
/// https://learn.microsoft.com/en-us/windows/win32/direct3ddds/dds-header-dxt10
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Dx10Header {
    /// The surface pixel format.
    pub dxgi_format: DxgiFormat,
    /// Identifies the type of resource.
    pub resource_dimension: ResourceDimension,
    /// Identifies other, less common options for resources.
    ///
    /// The following value for this member is a subset of the values in the D3D10_RESOURCE_MISC_FLAG or D3D11_RESOURCE_MISC_FLAG enumeration.
    pub misc_flag: MiscFlags,
    /// The number of elements in the array.
    ///
    /// For a 2D texture that is also a cube-map texture, this number represents the number of cubes. This number is the same as the number in the NumCubes member of D3D10_TEXCUBE_ARRAY_SRV1 or D3D11_TEXCUBE_ARRAY_SRV). In this case, the DDS file contains arraySize*6 2D textures. For more information about this case, see the miscFlag description.
    ///
    /// For a 3D texture, you must set this number to 1.
    pub array_size: u32,
    /// Contains additional metadata (formerly was reserved). The lower 3 bits indicate the alpha mode of the associated resource. The upper 29 bits are reserved and are typically 0.
    pub misc_flags2: MiscFlags2,
}
impl Dx10Header {
    pub(crate) const SIZE: usize = 20;

    fn from_raw(raw: &RawDx10Header, options: &Options) -> Result<Self, HeaderError> {
        let dxgi_format =
            DxgiFormat::try_from(raw.dxgi_format).map_err(HeaderError::InvalidDxgiFormat)?;
        let resource_dimension = ResourceDimension::try_from(raw.resource_dimension)
            .map_err(HeaderError::InvalidResourceDimension)?;

        let misc_flag = raw.misc_flag;
        let misc_flags2 = MiscFlags2::from_bits_retain(raw.misc_flags2);

        let mut array_size = raw.array_size;
        if resource_dimension == ResourceDimension::Texture3D && array_size != 1 {
            if options.permissive {
                array_size = 1;
            } else {
                return Err(HeaderError::InvalidArraySizeForTexture3D(array_size));
            }
        }

        Ok(Self {
            dxgi_format,
            resource_dimension,
            misc_flag,
            array_size,
            misc_flags2,
        })
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct DdsFlags: u32 {
        /// Required in every .dds file.
        const CAPS = 0x1;
        /// Required in every .dds file.
        const HEIGHT = 0x2;
        /// Required in every .dds file.
        const WIDTH = 0x4;
        /// Required when pitch is provided for an uncompressed texture.
        const PITCH = 0x8;
        /// Required in every .dds file.
        const PIXEL_FORMAT = 0x1000;
        /// Required in a mipmapped texture.
        const MIPMAP_COUNT = 0x20000;
        /// Required when pitch is provided for a compressed texture.
        const LINEAR_SIZE = 0x80000;
        /// Required in a depth texture.
        const DEPTH = 0x800000;

        /// Required in every .dds file.
        const REQUIRED = Self::CAPS.bits()
            | Self::HEIGHT.bits()
            | Self::WIDTH.bits()
            | Self::PIXEL_FORMAT.bits();
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct DdsCaps: u32 {
        /// Optional; must be used on any file that contains more than one surface (a mipmap, a cubic environment map, or mipmapped volume texture).
        const COMPLEX = 0x8;
        /// Optional; should be used for a mipmap.
        const MIPMAP = 0x400000;
        /// Required
        const TEXTURE = 0x1000;

        /// Required for all.
        const REQUIRED = Self::TEXTURE.bits();
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct DdsCaps2: u32 {
        /// Required for a cube map.
        const CUBE_MAP = 0x200;
        /// Required when these surfaces are stored in a cube map.
        const CUBE_MAP_POSITIVE_X = 0x400;
        /// Required when these surfaces are stored in a cube map.
        const CUBE_MAP_NEGATIVE_X = 0x800;
        /// Required when these surfaces are stored in a cube map.
        const CUBE_MAP_POSITIVE_Y = 0x1000;
        /// Required when these surfaces are stored in a cube map.
        const CUBE_MAP_NEGATIVE_Y = 0x2000;
        /// Required when these surfaces are stored in a cube map.
        const CUBE_MAP_POSITIVE_Z = 0x4000;
        /// Required when these surfaces are stored in a cube map.
        const CUBE_MAP_NEGATIVE_Z = 0x8000;
        /// Required for a volume texture.
        const VOLUME = 0x200000;

        /// Although Direct3D 9 supports partial cube-maps, Direct3D 10, 10.1, and 11 require that you define all six cube-map faces (that is, you must set DDS_CUBEMAP_ALLFACES).
        const CUBE_MAP_ALL_FACES = Self::CUBE_MAP_POSITIVE_X.bits()
            | Self::CUBE_MAP_NEGATIVE_X.bits()
            | Self::CUBE_MAP_POSITIVE_Y.bits()
            | Self::CUBE_MAP_NEGATIVE_Y.bits()
            | Self::CUBE_MAP_POSITIVE_Z.bits()
            | Self::CUBE_MAP_NEGATIVE_Z.bits();
    }

    /// Values which indicate what type of data is in the surface.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct PixelFormatFlags: u32 {
        // Official docs are outdated. The following constants are from (1) the
        // official docs and (2) the source code of DirectXTex:
        // https://github.com/microsoft/DirectXTex/blob/af1c8b3cb4cae9354a7aade2f999ebf97d46e4fb/DirectXTex/DDS.h#L42

        /// Texture contains alpha data; dwRGBAlphaBitMask contains valid data.
        const ALPHAPIXELS = 0x1;
        /// Used in some older DDS files for alpha channel only uncompressed data (dwRGBBitCount contains the alpha channel bitcount; dwABitMask contains valid data)
        const ALPHA = 0x2;
        /// Texture contains compressed RGB data; dwFourCC contains valid data.
        const FOURCC = 0x4;
        const PAL8 = 0x20;
        /// Texture contains uncompressed RGB data; dwRGBBitCount and the RGB masks (dwRBitMask, dwGBitMask, dwBBitMask) contain valid data.
        const RGB = 0x40;
        const RGBA = Self::RGB.bits() | Self::ALPHAPIXELS.bits();
        /// Used in some older DDS files for YUV uncompressed data (dwRGBBitCount contains the YUV bit count; dwRBitMask contains the Y mask, dwGBitMask contains the U mask, dwBBitMask contains the V mask)
        const YUV = 0x200;
        /// Used in some older DDS files for single channel color uncompressed data (dwRGBBitCount contains the luminance channel bit count; dwRBitMask contains the channel mask). Can be combined with DDPF_ALPHAPIXELS for a two channel DDS file.
        const LUMINANCE = 0x20000;
        const LUMINANCE_ALPHA = Self::LUMINANCE.bits() | Self::ALPHAPIXELS.bits();
        const BUMP_LUMINANCE = 0x40000;
        /// While DirectXTex calls this flag `BUMPDUDV` (bumpmap dUdV), this just says that the texture contains SNORM data. Which channels the texture contains depends on which bit masks are non-zero. All dw*BitMask fields contain valid data.
        const BUMP_DUDV = 0x80000;
    }

    /// Identifies other, less common options for resources.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct MiscFlags: u32 {
        /// Sets a resource to be a cube texture created from a Texture2DArray that contains 6 textures.
        const TEXTURE_CUBE = 0x4;
    }

    /// Additional metadata.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct MiscFlags2: u32 {
        /// Alpha channel content is unknown. This is the value for legacy files, which typically is assumed to be 'straight' alpha.
        const ALPHA_MODE_UNKNOWN = 0x0;
        /// Any alpha channel content is presumed to use straight alpha.
        const ALPHA_MODE_STRAIGHT = 0x1;
        /// Any alpha channel content is using premultiplied alpha. The only legacy file formats that indicate this information are 'DX2' and 'DX4'.
        const ALPHA_MODE_PREMULTIPLIED = 0x2;
        /// Any alpha channel content is all set to fully opaque.
        const ALPHA_MODE_OPAQUE = 0x3;
        /// Any alpha channel content is being used as a 4th channel and is not intended to represent transparency (straight or premultiplied).
        const ALPHA_MODE_CUSTOM = 0x4;
    }
}

/// Identifies the type of resource being used.
///
/// https://learn.microsoft.com/en-us/windows/win32/api/d3d10/ne-d3d10-d3d10_resource_dimension
/// https://learn.microsoft.com/en-us/windows/win32/api/d3d11/ne-d3d11-d3d11_resource_dimension
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResourceDimension {
    // Unknown = 0,
    // Buffer = 1,
    Texture1D = 2,
    Texture2D = 3,
    Texture3D = 4,
}
impl TryFrom<u32> for ResourceDimension {
    type Error = u32;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            2 => Ok(ResourceDimension::Texture1D),
            3 => Ok(ResourceDimension::Texture2D),
            4 => Ok(ResourceDimension::Texture3D),
            _ => Err(value),
        }
    }
}
impl From<ResourceDimension> for u32 {
    fn from(value: ResourceDimension) -> Self {
        value as u32
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FourCC(pub u32);

impl FourCC {
    pub const NONE: Self = FourCC(0);

    pub const DXT1: Self = FourCC(u32::from_le_bytes(*b"DXT1"));
    pub const DXT2: Self = FourCC(u32::from_le_bytes(*b"DXT2"));
    pub const DXT3: Self = FourCC(u32::from_le_bytes(*b"DXT3"));
    pub const DXT4: Self = FourCC(u32::from_le_bytes(*b"DXT4"));
    pub const DXT5: Self = FourCC(u32::from_le_bytes(*b"DXT5"));
    pub const RXGB: Self = FourCC(u32::from_le_bytes(*b"RXGB"));

    pub const DX10: Self = FourCC(u32::from_le_bytes(*b"DX10"));

    pub const ATI1: Self = FourCC(u32::from_le_bytes(*b"ATI1"));
    pub const BC4U: Self = FourCC(u32::from_le_bytes(*b"BC4U"));
    pub const BC4S: Self = FourCC(u32::from_le_bytes(*b"BC4S"));

    pub const ATI2: Self = FourCC(u32::from_le_bytes(*b"ATI2"));
    pub const BC5U: Self = FourCC(u32::from_le_bytes(*b"BC5U"));
    pub const BC5S: Self = FourCC(u32::from_le_bytes(*b"BC5S"));

    pub const RGBG: Self = FourCC(u32::from_le_bytes(*b"RGBG"));
    pub const GRGB: Self = FourCC(u32::from_le_bytes(*b"GRGB"));

    pub const YUY2: Self = FourCC(u32::from_le_bytes(*b"YUY2"));
    pub const UYVY: Self = FourCC(u32::from_le_bytes(*b"UYVY"));
}

impl From<u32> for FourCC {
    fn from(value: u32) -> Self {
        FourCC(value)
    }
}
impl From<FourCC> for u32 {
    fn from(value: FourCC) -> Self {
        value.0
    }
}

impl std::fmt::Debug for FourCC {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bytes = self.0.to_le_bytes();
        if bytes.iter().all(|&b| b.is_ascii_alphanumeric()) {
            write!(
                f,
                "FourCC({}{}{}{})",
                bytes[0] as char, bytes[1] as char, bytes[2] as char, bytes[3] as char
            )
        } else {
            write!(f, "FourCC(0x{:x})", self.0)
        }
    }
}

/// Resource data formats, including fully-typed and typeless formats. A list
/// of modifiers at the bottom of the page more fully describes each format
/// type.
///
/// https://learn.microsoft.com/en-us/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct DxgiFormat(u8);
impl DxgiFormat {
    pub fn is_srgb(&self) -> bool {
        matches!(
            *self,
            DxgiFormat::BC1_UNORM_SRGB
                | DxgiFormat::BC2_UNORM_SRGB
                | DxgiFormat::BC3_UNORM_SRGB
                | DxgiFormat::BC7_UNORM_SRGB
                | DxgiFormat::R8G8B8A8_UNORM_SRGB
                | DxgiFormat::B8G8R8A8_UNORM_SRGB
                | DxgiFormat::B8G8R8X8_UNORM_SRGB
        )
    }
}
impl TryFrom<u32> for DxgiFormat {
    type Error = u32;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        // NOTE: This implementation is NOT generated by the marco for
        // performance and code size reasons. On virtually any optimization
        // level, the below code translates to around 6 instructions, while a
        // generated match arm (0 | 1 | 2 | ... | 115 | 130 | 131 | 132 => ...)
        // translates to a LUT on -O3 and a jump table with 133 entries on
        // <= -O2, -Os, and -Oz. It's slower and takes up vastly more binary
        // size.
        match value {
            0..=115 | 130..=132 | 191 => Ok(DxgiFormat(value as u8)),
            _ => Err(value),
        }
    }
}
impl From<DxgiFormat> for u32 {
    fn from(value: DxgiFormat) -> Self {
        value.0 as u32
    }
}

macro_rules! define_dxgi_formats {
    ($($name:ident = $n:literal),+) => {
        impl DxgiFormat {
            $(pub const $name: DxgiFormat = DxgiFormat($n);)+
        }

        impl std::fmt::Debug for DxgiFormat {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let name = match *self {
                    $(Self::$name => stringify!($name),)+
                    _ => {
                        return write!(f, "DxgiFormat({})", self.0);
                    }
                };
                write!(f, "{} ({})", name, self.0)
            }
        }
    };
}
define_dxgi_formats!(
    UNKNOWN = 0,
    R32G32B32A32_TYPELESS = 1,
    R32G32B32A32_FLOAT = 2,
    R32G32B32A32_UINT = 3,
    R32G32B32A32_SINT = 4,
    R32G32B32_TYPELESS = 5,
    R32G32B32_FLOAT = 6,
    R32G32B32_UINT = 7,
    R32G32B32_SINT = 8,
    R16G16B16A16_TYPELESS = 9,
    R16G16B16A16_FLOAT = 10,
    R16G16B16A16_UNORM = 11,
    R16G16B16A16_UINT = 12,
    R16G16B16A16_SNORM = 13,
    R16G16B16A16_SINT = 14,
    R32G32_TYPELESS = 15,
    R32G32_FLOAT = 16,
    R32G32_UINT = 17,
    R32G32_SINT = 18,
    R32G8X24_TYPELESS = 19,
    D32_FLOAT_S8X24_UINT = 20,
    R32_FLOAT_X8X24_TYPELESS = 21,
    X32_TYPELESS_G8X24_UINT = 22,
    R10G10B10A2_TYPELESS = 23,
    R10G10B10A2_UNORM = 24,
    R10G10B10A2_UINT = 25,
    R11G11B10_FLOAT = 26,
    R8G8B8A8_TYPELESS = 27,
    R8G8B8A8_UNORM = 28,
    R8G8B8A8_UNORM_SRGB = 29,
    R8G8B8A8_UINT = 30,
    R8G8B8A8_SNORM = 31,
    R8G8B8A8_SINT = 32,
    R16G16_TYPELESS = 33,
    R16G16_FLOAT = 34,
    R16G16_UNORM = 35,
    R16G16_UINT = 36,
    R16G16_SNORM = 37,
    R16G16_SINT = 38,
    R32_TYPELESS = 39,
    D32_FLOAT = 40,
    R32_FLOAT = 41,
    R32_UINT = 42,
    R32_SINT = 43,
    R24G8_TYPELESS = 44,
    D24_UNORM_S8_UINT = 45,
    R24_UNORM_X8_TYPELESS = 46,
    X24_TYPELESS_G8_UINT = 47,
    R8G8_TYPELESS = 48,
    R8G8_UNORM = 49,
    R8G8_UINT = 50,
    R8G8_SNORM = 51,
    R8G8_SINT = 52,
    R16_TYPELESS = 53,
    R16_FLOAT = 54,
    D16_UNORM = 55,
    R16_UNORM = 56,
    R16_UINT = 57,
    R16_SNORM = 58,
    R16_SINT = 59,
    R8_TYPELESS = 60,
    R8_UNORM = 61,
    R8_UINT = 62,
    R8_SNORM = 63,
    R8_SINT = 64,
    A8_UNORM = 65,
    R1_UNORM = 66,
    R9G9B9E5_SHAREDEXP = 67,
    R8G8_B8G8_UNORM = 68,
    G8R8_G8B8_UNORM = 69,
    BC1_TYPELESS = 70,
    BC1_UNORM = 71,
    BC1_UNORM_SRGB = 72,
    BC2_TYPELESS = 73,
    BC2_UNORM = 74,
    BC2_UNORM_SRGB = 75,
    BC3_TYPELESS = 76,
    BC3_UNORM = 77,
    BC3_UNORM_SRGB = 78,
    BC4_TYPELESS = 79,
    BC4_UNORM = 80,
    BC4_SNORM = 81,
    BC5_TYPELESS = 82,
    BC5_UNORM = 83,
    BC5_SNORM = 84,
    B5G6R5_UNORM = 85,
    B5G5R5A1_UNORM = 86,
    B8G8R8A8_UNORM = 87,
    B8G8R8X8_UNORM = 88,
    R10G10B10_XR_BIAS_A2_UNORM = 89,
    B8G8R8A8_TYPELESS = 90,
    B8G8R8A8_UNORM_SRGB = 91,
    B8G8R8X8_TYPELESS = 92,
    B8G8R8X8_UNORM_SRGB = 93,
    BC6H_TYPELESS = 94,
    BC6H_UF16 = 95,
    BC6H_SF16 = 96,
    BC7_TYPELESS = 97,
    BC7_UNORM = 98,
    BC7_UNORM_SRGB = 99,
    AYUV = 100,
    Y410 = 101,
    Y416 = 102,
    NV12 = 103,
    P010 = 104,
    P016 = 105,
    OPAQUE_420 = 106,
    YUY2 = 107,
    Y210 = 108,
    Y216 = 109,
    NV11 = 110,
    AI44 = 111,
    IA44 = 112,
    P8 = 113,
    A8P8 = 114,
    B4G4R4A4_UNORM = 115,
    P208 = 130,
    V208 = 131,
    V408 = 132,
    A4B4G4R4_UNORM = 191
);
