//! Internal module for detecting supported formats from DXGI, FourCC, and
//! DDS pixel formats.

use crate::{DecodeFormat, DxgiFormat, FourCC, MaskPixelFormat, PixelFormatFlags};

pub(crate) const fn dxgi_format_to_supported(dxgi_format: DxgiFormat) -> Option<DecodeFormat> {
    match dxgi_format {
        // uncompressed formats
        DxgiFormat::R8G8B8A8_TYPELESS
        | DxgiFormat::R8G8B8A8_UNORM
        | DxgiFormat::R8G8B8A8_UNORM_SRGB => Some(DecodeFormat::R8G8B8A8_UNORM),
        DxgiFormat::R8G8B8A8_SNORM => Some(DecodeFormat::R8G8B8A8_SNORM),
        DxgiFormat::B8G8R8A8_TYPELESS
        | DxgiFormat::B8G8R8A8_UNORM
        | DxgiFormat::B8G8R8A8_UNORM_SRGB => Some(DecodeFormat::B8G8R8A8_UNORM),
        DxgiFormat::B8G8R8X8_TYPELESS
        | DxgiFormat::B8G8R8X8_UNORM
        | DxgiFormat::B8G8R8X8_UNORM_SRGB => Some(DecodeFormat::B8G8R8X8_UNORM),
        DxgiFormat::B5G6R5_UNORM => Some(DecodeFormat::B5G6R5_UNORM),
        DxgiFormat::B5G5R5A1_UNORM => Some(DecodeFormat::B5G5R5A1_UNORM),
        DxgiFormat::B4G4R4A4_UNORM => Some(DecodeFormat::B4G4R4A4_UNORM),
        DxgiFormat::A4B4G4R4_UNORM => Some(DecodeFormat::A4B4G4R4_UNORM),
        DxgiFormat::R8_TYPELESS | DxgiFormat::R8_UNORM => Some(DecodeFormat::R8_UNORM),
        DxgiFormat::R8_SNORM => Some(DecodeFormat::R8_SNORM),
        DxgiFormat::R8G8_UNORM => Some(DecodeFormat::R8G8_UNORM),
        DxgiFormat::R8G8_SNORM => Some(DecodeFormat::R8G8_SNORM),
        DxgiFormat::A8_UNORM => Some(DecodeFormat::A8_UNORM),
        DxgiFormat::R16_TYPELESS | DxgiFormat::R16_UNORM => Some(DecodeFormat::R16_UNORM),
        DxgiFormat::R16_SNORM => Some(DecodeFormat::R16_SNORM),
        DxgiFormat::R16_FLOAT => Some(DecodeFormat::R16_FLOAT),
        DxgiFormat::R16G16_TYPELESS | DxgiFormat::R16G16_UNORM => Some(DecodeFormat::R16G16_UNORM),
        DxgiFormat::R16G16_SNORM => Some(DecodeFormat::R16G16_SNORM),
        DxgiFormat::R16G16_FLOAT => Some(DecodeFormat::R16G16_FLOAT),
        DxgiFormat::R16G16B16A16_TYPELESS | DxgiFormat::R16G16B16A16_UNORM => {
            Some(DecodeFormat::R16G16B16A16_UNORM)
        }
        DxgiFormat::R16G16B16A16_SNORM => Some(DecodeFormat::R16G16B16A16_SNORM),
        DxgiFormat::R16G16B16A16_FLOAT => Some(DecodeFormat::R16G16B16A16_FLOAT),
        DxgiFormat::R10G10B10A2_TYPELESS | DxgiFormat::R10G10B10A2_UNORM => {
            Some(DecodeFormat::R10G10B10A2_UNORM)
        }
        DxgiFormat::R11G11B10_FLOAT => Some(DecodeFormat::R11G11B10_FLOAT),
        DxgiFormat::R9G9B9E5_SHAREDEXP => Some(DecodeFormat::R9G9B9E5_SHAREDEXP),
        DxgiFormat::R32_TYPELESS | DxgiFormat::R32_FLOAT => Some(DecodeFormat::R32_FLOAT),
        DxgiFormat::R32G32_TYPELESS | DxgiFormat::R32G32_FLOAT => Some(DecodeFormat::R32G32_FLOAT),
        DxgiFormat::R32G32B32_TYPELESS | DxgiFormat::R32G32B32_FLOAT => {
            Some(DecodeFormat::R32G32B32_FLOAT)
        }
        DxgiFormat::R32G32B32A32_TYPELESS | DxgiFormat::R32G32B32A32_FLOAT => {
            Some(DecodeFormat::R32G32B32A32_FLOAT)
        }
        DxgiFormat::R10G10B10_XR_BIAS_A2_UNORM => Some(DecodeFormat::R10G10B10_XR_BIAS_A2_UNORM),
        DxgiFormat::AYUV => Some(DecodeFormat::AYUV),
        DxgiFormat::Y410 => Some(DecodeFormat::Y410),
        DxgiFormat::Y416 => Some(DecodeFormat::Y416),

        // sub-sampled formats
        DxgiFormat::R8G8_B8G8_UNORM => Some(DecodeFormat::R8G8_B8G8_UNORM),
        DxgiFormat::G8R8_G8B8_UNORM => Some(DecodeFormat::G8R8_G8B8_UNORM),
        DxgiFormat::YUY2 => Some(DecodeFormat::YUY2),
        DxgiFormat::Y210 => Some(DecodeFormat::Y210),
        DxgiFormat::Y216 => Some(DecodeFormat::Y216),
        DxgiFormat::R1_UNORM => Some(DecodeFormat::R1_UNORM),

        // block compression formats
        DxgiFormat::BC1_TYPELESS | DxgiFormat::BC1_UNORM | DxgiFormat::BC1_UNORM_SRGB => {
            Some(DecodeFormat::BC1_UNORM)
        }
        DxgiFormat::BC2_TYPELESS | DxgiFormat::BC2_UNORM | DxgiFormat::BC2_UNORM_SRGB => {
            Some(DecodeFormat::BC2_UNORM)
        }
        DxgiFormat::BC3_TYPELESS | DxgiFormat::BC3_UNORM | DxgiFormat::BC3_UNORM_SRGB => {
            Some(DecodeFormat::BC3_UNORM)
        }
        DxgiFormat::BC4_TYPELESS | DxgiFormat::BC4_UNORM => Some(DecodeFormat::BC4_UNORM),
        DxgiFormat::BC4_SNORM => Some(DecodeFormat::BC4_SNORM),
        DxgiFormat::BC5_TYPELESS | DxgiFormat::BC5_UNORM => Some(DecodeFormat::BC5_UNORM),
        DxgiFormat::BC5_SNORM => Some(DecodeFormat::BC5_SNORM),
        DxgiFormat::BC6H_TYPELESS | DxgiFormat::BC6H_UF16 => Some(DecodeFormat::BC6H_UF16),
        DxgiFormat::BC6H_SF16 => Some(DecodeFormat::BC6H_SF16),
        DxgiFormat::BC7_TYPELESS | DxgiFormat::BC7_UNORM | DxgiFormat::BC7_UNORM_SRGB => {
            Some(DecodeFormat::BC7_UNORM)
        }
        _ => None,
    }
}

const fn four_cc_to_dxgi(four_cc: FourCC) -> Option<DxgiFormat> {
    match four_cc {
        FourCC::DXT1 => Some(DxgiFormat::BC1_UNORM),
        FourCC::DXT3 => Some(DxgiFormat::BC2_UNORM),
        FourCC::DXT5 => Some(DxgiFormat::BC3_UNORM),

        FourCC::ATI1 => Some(DxgiFormat::BC4_UNORM),
        FourCC::BC4U => Some(DxgiFormat::BC4_UNORM),
        FourCC::BC4S => Some(DxgiFormat::BC4_SNORM),

        FourCC::ATI2 => Some(DxgiFormat::BC5_UNORM),
        FourCC::BC5U => Some(DxgiFormat::BC5_UNORM),
        FourCC::BC5S => Some(DxgiFormat::BC5_SNORM),

        FourCC::RGBG => Some(DxgiFormat::R8G8_B8G8_UNORM),
        FourCC::GRGB => Some(DxgiFormat::G8R8_G8B8_UNORM),

        FourCC::YUY2 => Some(DxgiFormat::YUY2),

        // Some old encoders use the FOURCC field to store D3DFORMAT constants:
        // https://learn.microsoft.com/en-us/windows/win32/direct3d9/d3dformat
        //
        // We can theoretically support most of them. However, testing them
        // is hard because there aren't many programs that produce them
        // (AFAIK). Texconv from the DirectX SDK is one of them, but it only
        // produces the following formats.
        FourCC(36) => Some(DxgiFormat::R16G16B16A16_UNORM),
        FourCC(110) => Some(DxgiFormat::R16G16B16A16_SNORM),
        FourCC(111) => Some(DxgiFormat::R16_FLOAT),
        FourCC(112) => Some(DxgiFormat::R16G16_FLOAT),
        FourCC(113) => Some(DxgiFormat::R16G16B16A16_FLOAT),
        FourCC(114) => Some(DxgiFormat::R32_FLOAT),
        FourCC(115) => Some(DxgiFormat::R32G32_FLOAT),
        FourCC(116) => Some(DxgiFormat::R32G32B32A32_FLOAT),

        _ => None,
    }
}

pub(crate) const fn four_cc_to_supported(four_cc: FourCC) -> Option<DecodeFormat> {
    // quick and easy, convert to DXGI first
    if let Some(dxgi_format) = four_cc_to_dxgi(four_cc) {
        return dxgi_format_to_supported(dxgi_format);
    }

    // now everything that doesn't have a DXGI format equivalent
    match four_cc {
        FourCC::DXT2 => Some(DecodeFormat::BC2_UNORM_PREMULTIPLIED_ALPHA),
        FourCC::DXT4 => Some(DecodeFormat::BC3_UNORM_PREMULTIPLIED_ALPHA),

        FourCC::RXGB => Some(DecodeFormat::BC3_UNORM_RXGB),

        FourCC::UYVY => Some(DecodeFormat::UYVY),

        _ => None,
    }
}

pub(crate) fn pixel_format_to_supported(pf: &MaskPixelFormat) -> Option<DecodeFormat> {
    // known patterns
    for (pattern, format) in KNOWN_PIXEL_FORMATS {
        if pattern.matches(pf) {
            return Some(*format);
        }
    }

    None
}

struct PFPattern {
    flags: PixelFormatFlags,
    rgb_bit_count: u32,
    r_bit_mask: u32,
    g_bit_mask: u32,
    b_bit_mask: u32,
    a_bit_mask: u32,
}
impl PFPattern {
    fn matches(&self, pf: &MaskPixelFormat) -> bool {
        pf.flags == self.flags
            && pf.rgb_bit_count == self.rgb_bit_count
            && pf.r_bit_mask == self.r_bit_mask
            && pf.g_bit_mask == self.g_bit_mask
            && pf.b_bit_mask == self.b_bit_mask
            && pf.a_bit_mask == self.a_bit_mask
    }
    const fn with_flags(mut self, flags: PixelFormatFlags) -> Self {
        self.flags = flags;
        self
    }
}
const KNOWN_PIXEL_FORMATS: &[(PFPattern, DecodeFormat)] = {
    const fn alpha_only(bit_count: u32, a_mask: u32) -> PFPattern {
        PFPattern {
            flags: PixelFormatFlags::ALPHA,
            rgb_bit_count: bit_count,
            r_bit_mask: 0,
            g_bit_mask: 0,
            b_bit_mask: 0,
            a_bit_mask: a_mask,
        }
    }
    const fn grayscale(bit_count: u32, r_mask: u32) -> PFPattern {
        PFPattern {
            flags: PixelFormatFlags::LUMINANCE,
            rgb_bit_count: bit_count,
            r_bit_mask: r_mask,
            g_bit_mask: 0,
            b_bit_mask: 0,
            a_bit_mask: 0,
        }
    }
    const fn rgb(bit_count: u32, r_mask: u32, g_mask: u32, b_mask: u32) -> PFPattern {
        PFPattern {
            flags: PixelFormatFlags::RGB,
            rgb_bit_count: bit_count,
            r_bit_mask: r_mask,
            g_bit_mask: g_mask,
            b_bit_mask: b_mask,
            a_bit_mask: 0,
        }
    }
    const fn rgba(bit_count: u32, r_mask: u32, g_mask: u32, b_mask: u32, a_mask: u32) -> PFPattern {
        PFPattern {
            flags: PixelFormatFlags::RGBA,
            rgb_bit_count: bit_count,
            r_bit_mask: r_mask,
            g_bit_mask: g_mask,
            b_bit_mask: b_mask,
            a_bit_mask: a_mask,
        }
    }
    const fn snorm(
        bit_count: u32,
        r_mask: u32,
        g_mask: u32,
        b_mask: u32,
        a_mask: u32,
    ) -> PFPattern {
        PFPattern {
            flags: PixelFormatFlags::BUMP_DUDV,
            rgb_bit_count: bit_count,
            r_bit_mask: r_mask,
            g_bit_mask: g_mask,
            b_bit_mask: b_mask,
            a_bit_mask: a_mask,
        }
    }

    let rgb_luminance = PixelFormatFlags::RGB.union(PixelFormatFlags::LUMINANCE);

    use DecodeFormat::*;

    &[
        // alpha
        (alpha_only(8, 0xFF), A8_UNORM),
        // grayscale
        (grayscale(8, 0xFF), R8_UNORM),
        (grayscale(8, 0xFF).with_flags(rgb_luminance), R8_UNORM),
        (grayscale(16, 0xFFFF), R16_UNORM),
        // rgb
        (rgb(16, 0xF800, 0x07E0, 0x001F), B5G6R5_UNORM),
        (rgb(32, 0xFF0000, 0xFF00, 0xFF), B8G8R8X8_UNORM),
        (rgb(32, 0xFFFF, 0xFFFF0000, 0), R16G16_UNORM),
        (rgb(16, 0xFF, 0xFF00, 0), R8G8_UNORM),
        (rgb(24, 0xFF0000, 0xFF00, 0xFF), B8G8R8_UNORM),
        (rgb(24, 0xFF, 0xFF00, 0xFF0000), R8G8B8_UNORM),
        // rgba
        (rgba(16, 0xF00, 0xF0, 0xF, 0xF000), B4G4R4A4_UNORM),
        (rgba(16, 0x7C00, 0x3E0, 0x1F, 0x8000), B5G5R5A1_UNORM),
        (rgba(32, 0xFF0000, 0xFF00, 0xFF, 0xFF000000), B8G8R8A8_UNORM),
        (rgba(32, 0xFF, 0xFF00, 0xFF0000, 0xFF000000), R8G8B8A8_UNORM),
        (
            rgba(32, 0x3FF00000, 0xFFC00, 0x3FF, 0xC0000000),
            R10G10B10A2_UNORM,
        ),
        // snorm
        (
            snorm(32, 0xFF, 0xFF00, 0xFF0000, 0xFF000000),
            R8G8B8A8_SNORM,
        ),
        (snorm(16, 0xFF, 0xFF00, 0, 0), R8G8_SNORM),
        (snorm(32, 0xFFFF, 0xFFFF0000, 0, 0), R16G16_SNORM),
        // special
        (
            // I have no idea why, but LUMINANCE + ALPHAPIXELS is used for R8G8_UNORM
            PFPattern {
                flags: PixelFormatFlags::LUMINANCE_ALPHA,
                rgb_bit_count: 16,
                r_bit_mask: 0xFF,
                g_bit_mask: 0,
                b_bit_mask: 0,
                a_bit_mask: 0xFF00,
            },
            R8G8_UNORM,
        ),
    ]
};
