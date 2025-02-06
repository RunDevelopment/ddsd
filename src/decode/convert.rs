//! Internal module for converting between different number formats.
//!
//! Most magic constants for the U/SNorm conversion are from:
//! https://rundevelopment.github.io/projects/multiply-add-constants-finder

use crate::Precision;

#[derive(Debug, Clone, Copy)]
pub(crate) struct B5G6R5 {
    pub r5: u16,
    pub g6: u16,
    pub b5: u16,
}
impl B5G6R5 {
    #[inline(always)]
    pub fn from_le_bytes(bytes: [u8; 2]) -> Self {
        Self::from_u16(u16::from_le_bytes(bytes))
    }
    #[inline(always)]
    pub fn from_u16(u: u16) -> Self {
        Self {
            b5: u & 0x1F,
            g6: (u >> 5) & 0x3F,
            r5: (u >> 11) & 0x1F,
        }
    }
    #[inline(always)]
    pub fn to_n8(self) -> [u8; 3] {
        [
            n5::n8(self.r5 as u8),
            n6::n8(self.g6 as u8),
            n5::n8(self.b5 as u8),
        ]
    }
    #[inline(always)]
    pub fn to_n16(self) -> [u16; 3] {
        [
            n5::n16(self.r5 as u8),
            n6::n16(self.g6 as u8),
            n5::n16(self.b5 as u8),
        ]
    }
    #[inline(always)]
    pub fn to_f32(self) -> [f32; 3] {
        [
            n5::f32(self.r5 as u8),
            n6::f32(self.g6 as u8),
            n5::f32(self.b5 as u8),
        ]
    }

    // The nearest RGB8 color that represents `self * 2/3 + color * 1/3`.
    pub(crate) fn one_third_color_rgb8(self, color: Self) -> [u8; 3] {
        let r = self.r5 * 2 + color.r5;
        let g = self.g6 * 2 + color.g6;
        let b = self.b5 * 2 + color.b5;

        let r = ((r * 351 + 61) >> 7) as u8;
        let g = ((g as u32 * 2763 + 1039) >> 11) as u8;
        let b = ((b * 351 + 61) >> 7) as u8;
        [r, g, b]
    }
    // The nearest RGB8 color that represents `self * 1/2 + color * 1/2`.
    pub(crate) fn mid_color_rgb8(self, color: Self) -> [u8; 3] {
        let r = self.r5 + color.r5;
        let g = self.g6 + color.g6;
        let b = self.b5 + color.b5;

        let r = ((r * 1053 + 125) >> 8) as u8;
        let g = ((g as u32 * 4145 + 1019) >> 11) as u8;
        let b = ((b * 1053 + 125) >> 8) as u8;
        [r, g, b]
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct B5G5R5A1 {
    pub r5: u16,
    pub g5: u16,
    pub b5: u16,
    pub a1: u16,
}
impl B5G5R5A1 {
    #[inline(always)]
    pub fn from_u16(u: u16) -> Self {
        Self {
            b5: u & 0x1F,
            g5: (u >> 5) & 0x1F,
            r5: (u >> 10) & 0x1F,
            a1: (u >> 15) & 0x1,
        }
    }
    #[inline(always)]
    pub fn to_n8(self) -> [u8; 4] {
        [
            n5::n8(self.r5 as u8),
            n5::n8(self.g5 as u8),
            n5::n8(self.b5 as u8),
            n1::n8(self.a1 as u8),
        ]
    }
    #[inline(always)]
    pub fn to_n16(self) -> [u16; 4] {
        [
            n5::n16(self.r5 as u8),
            n5::n16(self.g5 as u8),
            n5::n16(self.b5 as u8),
            n1::n16(self.a1 as u8),
        ]
    }
    #[inline(always)]
    pub fn to_f32(self) -> [f32; 4] {
        [
            n5::f32(self.r5 as u8),
            n5::f32(self.g5 as u8),
            n5::f32(self.b5 as u8),
            n1::f32(self.a1 as u8),
        ]
    }
}

/// Functions for converting **FROM Unorm1** values to other formats.
pub(crate) mod n1 {
    #[inline(always)]
    pub fn n8(x: u8) -> u8 {
        debug_assert!(x <= 1);
        if x == 0 {
            0
        } else {
            u8::MAX
        }
    }
    #[inline(always)]
    pub fn n16(x: u8) -> u16 {
        debug_assert!(x <= 1);
        if x == 0 {
            0
        } else {
            u16::MAX
        }
    }
    #[inline(always)]
    pub fn f32(x: u8) -> f32 {
        debug_assert!(x <= 1);
        if x == 0 {
            0.0
        } else {
            1.0
        }
    }
}

/// Functions for converting **FROM Unorm2** values to other formats.
pub(crate) mod n2 {
    #[inline(always)]
    pub fn n8(x: u8) -> u8 {
        debug_assert!(x <= 3);
        x * 85
    }
    #[inline(always)]
    pub fn n16(x: u8) -> u16 {
        debug_assert!(x <= 3);
        x as u16 * 21845
    }
    #[inline(always)]
    pub fn f32(x: u8) -> f32 {
        debug_assert!(x <= 3);
        // This turns out to be exact, so we don't need another method.
        const F: f32 = 1.0 / 3.0;
        x as f32 * F
    }
}

/// Functions for converting **FROM Unorm4** values to other formats.
pub(crate) mod n4 {
    #[inline(always)]
    pub fn n8(x: u8) -> u8 {
        debug_assert!(x <= 15);
        x * 17
    }
    #[inline(always)]
    pub fn n16(x: u8) -> u16 {
        debug_assert!(x <= 15);
        x as u16 * 4369
    }
    #[inline(always)]
    pub fn f32(x: u8) -> f32 {
        debug_assert!(x <= 15);
        const F: f32 = 1.0 / 15.0;
        x as f32 * F
    }
    #[inline(always)]
    pub fn f32_exact(x: u8) -> f32 {
        debug_assert!(x <= 15);
        // Adopted from: https://fgiesen.wordpress.com/2024/11/06/exact-unorm8-to-float/
        // k0=3 was found by trial and error.
        const K0: f32 = 3.0;
        const K1: f32 = 1.0 / (15.0 * K0);
        (x as f32 * K0) * K1
    }
}

/// Functions for converting **FROM Unorm5** values to other formats.
pub(crate) mod n5 {
    #[inline(always)]
    pub fn n8(x: u8) -> u8 {
        debug_assert!(x <= 31);
        ((x as u16 * 2108 + 92) >> 8) as u8
    }
    #[inline(always)]
    pub fn n16(x: u8) -> u16 {
        debug_assert!(x <= 31);
        ((x as u32 * 138547200) >> 16) as u16
    }
    #[inline(always)]
    pub fn f32(x: u8) -> f32 {
        debug_assert!(x <= 31);
        const F: f32 = 1.0 / 31.0;
        x as f32 * F
    }
    #[inline(always)]
    pub fn f32_exact(x: u8) -> f32 {
        debug_assert!(x <= 31);
        // Adopted from: https://fgiesen.wordpress.com/2024/11/06/exact-unorm8-to-float/
        // k0=3 was found by trial and error.
        const K0: f32 = 3.0;
        const K1: f32 = 1.0 / (31.0 * K0);
        (x as f32 * K0) * K1
    }
}

/// Functions for converting **FROM Unorm5** values to other formats.
pub(crate) mod n6 {
    #[inline(always)]
    pub fn n8(x: u8) -> u8 {
        debug_assert!(x <= 63);
        ((x as u16 * 1036 + 132) >> 8) as u8
    }
    #[inline(always)]
    pub fn n16(x: u8) -> u16 {
        debug_assert!(x <= 63);
        ((x as u32 * 68173056 + 30976) >> 16) as u16
    }
    #[inline(always)]
    pub fn f32(x: u8) -> f32 {
        debug_assert!(x <= 63);
        const F: f32 = 1.0 / 63.0;
        x as f32 * F
    }
    #[inline(always)]
    pub fn f32_exact(x: u8) -> f32 {
        debug_assert!(x <= 63);
        // Adopted from: https://fgiesen.wordpress.com/2024/11/06/exact-unorm8-to-float/
        // k0=5 was found by trial and error.
        const K0: f32 = 5.0;
        const K1: f32 = 1.0 / (63.0 * K0);
        (x as f32 * K0) * K1
    }
}

/// Functions for converting **FROM Unorm8** values to other formats.
pub(crate) mod n8 {
    #[inline(always)]
    pub fn n16(x: u8) -> u16 {
        x as u16 * 257
    }
    #[inline(always)]
    pub fn f32(x: u8) -> f32 {
        const F: f32 = 1.0 / 255.0;
        x as f32 * F
    }
    #[inline(always)]
    pub fn f32_exact(x: u8) -> f32 {
        // https://fgiesen.wordpress.com/2024/11/06/exact-unorm8-to-float/
        const K0: f32 = 3.0;
        const K1: f32 = 1.0 / (255.0 * K0);
        (x as f32 * K0) * K1
    }
}

/// Functions for converting **FROM Unorm10** values to other formats.
pub(crate) mod n10 {
    #[inline(always)]
    pub fn n8(x: u16) -> u8 {
        debug_assert!(x <= 1023);
        ((x as u32 * 16336 + 32656) >> 16) as u8
    }
    #[inline(always)]
    pub fn n16(x: u16) -> u16 {
        debug_assert!(x <= 1023);
        ((x as u32 * 4198340 + 32660) >> 16) as u16
    }
    #[inline(always)]
    pub fn f32(x: u16) -> f32 {
        debug_assert!(x <= 1023);
        const F: f32 = 1.0 / 1023.0;
        x as f32 * F
    }
    #[inline(always)]
    pub fn f32_exact(x: u16) -> f32 {
        debug_assert!(x <= 1023);
        // Adopted from: https://fgiesen.wordpress.com/2024/11/06/exact-unorm8-to-float/
        // k0=85 was found by trial and error.
        const K0: f32 = 85.0;
        const K1: f32 = 1.0 / (1023.0 * K0);
        (x as f32 * K0) * K1
    }
}

/// Functions for converting **FROM Unorm16** values to other formats.
pub(crate) mod n16 {
    #[inline(always)]
    pub fn n8(x: u16) -> u8 {
        ((x as u32 * 255 + 32895) >> 16) as u8
    }
    #[inline(always)]
    pub fn f32(x: u16) -> f32 {
        const F: f32 = 1.0 / 65535.0;
        x as f32 * F
    }
    #[inline(always)]
    pub fn f32_exact(x: u16) -> f32 {
        // Adopted from https://fgiesen.wordpress.com/2024/11/06/exact-unorm8-to-float/
        // I couldn't find any k0 that would work, so I used the infinite sum
        // approach from the article instead. This is slower, but oh well.
        const C0: f32 = 1.0 / 65536.0;
        const C1: f32 = (1.0 + 65536.0) / 65536.0 / 65536.0 / 65536.0;
        let temp = x as f32;
        (temp * C0) + (temp * C1)
    }
}

/// Functions for converting **FROM Snorm8** values to other formats.
pub(crate) mod s8 {
    /// Brings it in the range `[0, 254]`.
    #[inline(always)]
    pub fn norm(x: u8) -> u8 {
        // If you think that we can just do `x.wrapping_add(128)`, you'd be wrong.
        // https://learn.microsoft.com/en-us/windows/win32/api/dxgiformat/ne-dxgiformat-dxgi_format#format-modifiers
        // Both -128 and -127 map to -1.0. So we have to do more work:
        x.wrapping_add(128).saturating_sub(1)
    }

    #[inline(always)]
    pub fn n8(mut x: u8) -> u8 {
        x = norm(x);
        ((x as u16 * 258 + 2) >> 8) as u8
    }
    #[inline(always)]
    pub fn n16(mut x: u8) -> u16 {
        x = norm(x);
        ((x as u32 * 16909064 + 32520) >> 16) as u16
    }
    /// Unsigned f32.
    #[inline(always)]
    pub fn uf32(mut x: u8) -> f32 {
        x = norm(x);
        const F: f32 = 1.0 / 254.0;
        x as f32 * F
    }
    /// Unsigned f32.
    #[inline(always)]
    pub fn uf32_exact(mut x: u8) -> f32 {
        x = norm(x);
        // Adopted from: https://fgiesen.wordpress.com/2024/11/06/exact-unorm8-to-float/
        // k0=31 was found by trial and error.
        const K0: f32 = 31.0;
        const K1: f32 = 1.0 / (254.0 * K0);
        (x as f32 * K0) * K1
    }
}

/// Functions for converting **FROM Snorm16** values to other formats.
pub(crate) mod s16 {
    /// Brings it in the range `[0, 65534]`.
    #[inline(always)]
    pub fn norm(x: u16) -> u16 {
        // Same for as for Snorm8.
        x.wrapping_add(32768).saturating_sub(1)
    }

    #[inline(always)]
    pub fn n8(mut x: u16) -> u8 {
        x = norm(x);
        ((x as u32 * 65282 + 8388354) >> 24) as u8
    }
    #[inline(always)]
    pub fn n16(mut x: u16) -> u16 {
        x = norm(x);
        ((x as u32 * 65538 + 2) >> 16) as u16
    }
    /// Unsigned f32.
    #[inline(always)]
    pub fn uf32(mut x: u16) -> f32 {
        x = norm(x);
        const F: f32 = 1.0 / 65534.0;
        x as f32 * F
    }
    /// Unsigned f32.
    #[inline(always)]
    pub fn uf32_exact(mut x: u16) -> f32 {
        x = norm(x);
        // Adopted from: https://fgiesen.wordpress.com/2024/11/06/exact-unorm8-to-float/
        // k0=73 was found by trial and error.
        const K0: f32 = 73.0;
        const K1: f32 = 1.0 / (65534.0 * K0);
        (x as f32 * K0) * K1
    }
}

/// Functions for converting **FROM 10-bit XR_BIAS** values to other formats.
///
/// These are 2.8 fixed-point numbers, meaning 2 integer bits and 8 fractional
/// bits. These numbers are biased by -1.5 and then scaled by 256/510, resulting
/// in an effective range of `[-0.75294, 1.25294]`. XR (probably) means extended
/// range.
///
/// The conversion from 10-bit XR_BIAS to float is:
///
/// ```c
/// // source: https://learn.microsoft.com/en-us/windows-hardware/drivers/display/xr-bias-to-float-conversion-rules
/// float XRtoFloat( UINT XRComponent ) {
///     // The & 0x3ff shows that only 10 bits contribute to the conversion.
///     return (float)( (XRComponent & 0x3ff) - 0x180 ) / 510.f;
/// }
/// ```
pub(crate) mod xr10 {
    #[inline(always)]
    pub fn n8(x: u16) -> u8 {
        // new range: [-384, 639] (or [-0.75294, 1.25294])
        let x = x as i16 - 0x180;
        // new range: [0, 510] (or [0.0, 1.0])
        let x = x.clamp(0, 510) as u16;
        // this is round(x / 510 * 255), but faster
        ((x + 1) >> 1) as u8
    }
    #[inline(always)]
    pub fn n16(x: u16) -> u16 {
        // new range: [-384, 639] (or [-0.75294, 1.25294])
        let x = x as i16 - 0x180;
        // new range: [0, 510] (or [0.0, 1.0])
        let x = x.clamp(0, 510) as u16;
        // this is round(x / 510 * 65535), but faster
        ((x as u32 * 8421376 + 65535) >> 16) as u16
    }
    #[inline(always)]
    pub fn f32(x: u16) -> f32 {
        // 0x180 == 1.5 in 2.8 fixed-point.
        const F: f32 = 1.0 / 510.0;
        (x as i16 - 0x180) as f32 * F
    }
}

/// Functions for converting `f32` values to other formats.
pub(crate) mod fp {
    #[inline(always)]
    pub fn n8(x: f32) -> u8 {
        (x * 255.0 + 0.5) as u8
    }
    #[inline(always)]
    pub fn n16(x: f32) -> u16 {
        (x * 65535.0 + 0.5) as u16
    }
}

/// Functions for converting `f16` values to other formats.
pub(crate) mod fp16 {
    use crate::util::{two_powi, unlikely_branch};

    #[inline]
    pub fn n8(x: u16) -> u8 {
        // This is optimized implementation, combining fp16::f32 -> fp::n8 into one step.
        let exp: u16 = x >> 10 & 0b1_1111;
        let mant: u16 = x & 0b11_1111_1111;
        // Note: denorm all go to zero after rounding, so they don't need an extra branch.
        let val: u8 = if exp != 31 {
            ((mant as f32 + 1024_f32) * two_powi(exp as i8 - 25) * 255.0 + 0.5) as u8
        } else {
            unlikely_branch();
            if mant == 0 {
                // Inf goes to u8::MAX
                u8::MAX
            } else {
                // NaN goes to zero
                0
            }
        };
        if x & 0x8000 != 0 {
            // negative numbers go to zero
            0
        } else {
            val
        }
    }
    #[inline]
    pub fn n16(x: u16) -> u16 {
        // This is optimized implementation, combining fp16::f32 -> fp::n16 into one step.
        let exp: u16 = x >> 10 & 0b1_1111;
        let mant: u16 = x & 0b11_1111_1111;
        let val: u16 = if exp == 0 {
            // denorm
            unlikely_branch();
            const F: f32 = 65535.0 / 16777216.0;
            (mant as f32 * F + 0.5) as u16
        } else if exp != 31 {
            ((mant as f32 + 1024_f32) * two_powi(exp as i8 - 25) * 65535.0 + 0.5) as u16
        } else {
            unlikely_branch();
            if mant == 0 {
                // Inf goes to u16::MAX
                u16::MAX
            } else {
                // NaN goes to zero
                0
            }
        };
        if x & 0x8000 != 0 {
            // negative numbers go to zero
            0
        } else {
            val
        }
    }
    #[inline]
    pub fn f32(x: u16) -> f32 {
        // https://stackoverflow.com/questions/36008434/how-can-i-decode-f16-to-f32-using-only-the-stable-standard-library
        let exp: u16 = x >> 10 & 0b1_1111;
        let mant: u16 = x & 0b11_1111_1111;
        let val: f32 = if exp == 0 {
            // denorm
            unlikely_branch();
            mant as f32 * two_powi(-24)
        } else if exp != 31 {
            (mant as f32 + 1024_f32) * two_powi(exp as i8 - 25)
        } else {
            unlikely_branch();
            if mant == 0 {
                f32::INFINITY
            } else {
                f32::NAN
            }
        };
        if x & 0x8000 != 0 {
            -val
        } else {
            val
        }
    }
}

/// Functions for converting `f11` values to other formats.
pub(crate) mod fp11 {
    use crate::util::{two_powi, unlikely_branch};

    #[inline]
    pub fn n8(x: u16) -> u8 {
        let exp: u16 = x >> 6 & 0b1_1111;
        let mant: u16 = x & 0b11_1111;
        // no sign bit

        if exp != 31 {
            ((mant as f32 + 64_f32) * two_powi(exp as i8 - 21) * 255.0 + 0.5) as u8
        } else {
            unlikely_branch();
            if mant == 0 {
                255
            } else {
                0
            }
        }
    }
    #[inline]
    pub fn n16(x: u16) -> u16 {
        let exp: u16 = x >> 6 & 0b1_1111;
        let mant: u16 = x & 0b11_1111;
        // no sign bit

        if exp == 0 {
            // denorm
            // (mant as f32 * two_powi(-20) * 65535.0 + 0.5) as u16
            (mant + 7) >> 4
        } else if exp != 31 {
            ((mant as f32 + 64_f32) * two_powi(exp as i8 - 21) * 65535.0 + 0.5) as u16
        } else {
            unlikely_branch();
            if mant == 0 {
                u16::MAX
            } else {
                0
            }
        }
    }
    #[inline]
    pub fn f32(x: u16) -> f32 {
        // based on f16_to_f32
        let exp: u16 = x >> 6 & 0b1_1111;
        let mant: u16 = x & 0b11_1111;
        // no sign bit

        if exp == 0 {
            // denorm
            mant as f32 * two_powi(-20)
        } else if exp != 31 {
            (mant as f32 + 64_f32) * two_powi(exp as i8 - 21)
        } else {
            unlikely_branch();
            if mant == 0 {
                f32::INFINITY
            } else {
                f32::NAN
            }
        }
    }
}

/// Functions for converting `f10` values to other formats.
pub(crate) mod fp10 {
    use crate::util::{two_powi, unlikely_branch};

    #[inline]
    pub fn n8(x: u16) -> u8 {
        let exp: u16 = x >> 5 & 0b1_1111;
        let mant: u16 = x & 0b1_1111;
        // no sign bit

        if exp != 31 {
            ((mant as f32 + 32_f32) * two_powi(exp as i8 - 20) * 255.0 + 0.5) as u8
        } else {
            unlikely_branch();
            if mant == 0 {
                255
            } else {
                0
            }
        }
    }
    #[inline]
    pub fn n16(x: u16) -> u16 {
        let exp: u16 = x >> 5 & 0b1_1111;
        let mant: u16 = x & 0b1_1111;
        // no sign bit

        if exp == 0 {
            // denorm
            // (mant as f32 * two_powi(-19) * 65535.0 + 0.5) as u16
            (mant + 3) >> 3
        } else if exp != 31 {
            ((mant as f32 + 32_f32) * two_powi(exp as i8 - 20) * 65535.0 + 0.5) as u16
        } else {
            unlikely_branch();
            if mant == 0 {
                u16::MAX
            } else {
                0
            }
        }
    }
    #[inline]
    pub fn f32(x: u16) -> f32 {
        // based on f16_to_f32
        let exp: u16 = x >> 5 & 0b1_1111;
        let mant: u16 = x & 0b1_1111;
        // no sign bit

        if exp == 0 {
            // denorm
            mant as f32 * two_powi(-19)
        } else if exp != 31 {
            (mant as f32 + 32_f32) * two_powi(exp as i8 - 20)
        } else {
            unlikely_branch();
            if mant == 0 {
                f32::INFINITY
            } else {
                f32::NAN
            }
        }
    }
}

// TODO: Check whether these methods correctly implement the DirectX spec:
// https://microsoft.github.io/DirectX-Specs/d3d/archive/D3D11_3_FunctionalSpec.htm#3.2.2%20Floating%20Point%20Conversion

/// Optimized functions for the R9G9B9E5_SHAREDEXP format.
pub(crate) mod rgb9995f {
    use crate::util::two_powi;

    // This is 2 ** -23
    const C23: f32 = 1.0 / 8388608.0;

    #[inline]
    pub fn f32(rgb: u32) -> [f32; 3] {
        let r_mant = rgb & 0x1FF;
        let g_mant = (rgb >> 9) & 0x1FF;
        let b_mant = (rgb >> 18) & 0x1FF;
        let exp = (rgb >> 27) & 0x1F;

        if exp == 0 {
            // denorm
            let f = C23;
            [r_mant as f32 * f, g_mant as f32 * f, b_mant as f32 * f]
        } else if exp != 31 {
            let f = two_powi(exp as i8 - 24);
            [r_mant as f32 * f, g_mant as f32 * f, b_mant as f32 * f]
        } else {
            [
                if r_mant == 0 { f32::INFINITY } else { f32::NAN },
                if g_mant == 0 { f32::INFINITY } else { f32::NAN },
                if b_mant == 0 { f32::INFINITY } else { f32::NAN },
            ]
        }
    }
    #[inline]
    pub fn n8(rgb: u32) -> [u8; 3] {
        let r_mant = rgb & 0x1FF;
        let g_mant = (rgb >> 9) & 0x1FF;
        let b_mant = (rgb >> 18) & 0x1FF;
        let exp = (rgb >> 27) & 0x1F;

        if exp == 0 {
            // denorms are technically a special case, but since they will
            // always be rounded to 0, no matter the mantissa, we can just let
            // them fall through. One branch less.
        }

        if exp != 31 {
            // This is just the f32 conversion and f32 -> UNORM8 conversion
            // combined into one step.
            //
            // NOTE: I originally used a fixed-point math implementation, but
            // it was around 50% slower. I also looked into using f16 -> u8
            // hardware instructions (x86 f16c VCVTPH2PS), but this isn't
            // possible simply because the mantissa here has an *explicit* one
            // at the start. I also suspect that fixing up the one bit would make
            // R9G9B9E5 -> f16 -> f32 -> u8 slower than what I use below.

            let f = two_powi(exp as i8 - 24) * 255.0;
            [
                (r_mant as f32 * f + 0.5) as u8,
                (g_mant as f32 * f + 0.5) as u8,
                (b_mant as f32 * f + 0.5) as u8,
            ]
        } else {
            // NaN maps to 0 and Inf maps to 255
            [
                if r_mant == 0 { 255 } else { 0 },
                if g_mant == 0 { 255 } else { 0 },
                if b_mant == 0 { 255 } else { 0 },
            ]
        }
    }
    #[inline]
    pub fn n16(rgb: u32) -> [u16; 3] {
        let r_mant = rgb & 0x1FF;
        let g_mant = (rgb >> 9) & 0x1FF;
        let b_mant = (rgb >> 18) & 0x1FF;
        let exp = (rgb >> 27) & 0x1F;

        // This method is essentially the same as the above n8 method, so see
        // above for more information. The only difference is that denorms can
        // no longer fall through.

        if exp == 0 {
            // denorm
            const F: f32 = C23 * 65535.0;
            [
                (r_mant as f32 * F + 0.5) as u16,
                (g_mant as f32 * F + 0.5) as u16,
                (b_mant as f32 * F + 0.5) as u16,
            ]
        } else if exp != 31 {
            let f = two_powi(exp as i8 - 24) * 65535.0;
            [
                (r_mant as f32 * f + 0.5) as u16,
                (g_mant as f32 * f + 0.5) as u16,
                (b_mant as f32 * f + 0.5) as u16,
            ]
        } else {
            [
                if r_mant == 0 { u16::MAX } else { 0 },
                if g_mant == 0 { u16::MAX } else { 0 },
                if b_mant == 0 { u16::MAX } else { 0 },
            ]
        }
    }
}

pub(crate) trait ToRgba {
    type Channel;
    fn to_rgba(self) -> [Self::Channel; 4];
}
impl<T: Norm> ToRgba for [T; 3] {
    type Channel = T;

    #[inline(always)]
    fn to_rgba(self) -> [T; 4] {
        let [r, g, b] = self;
        [r, g, b, Norm::ONE]
    }
}
impl<T: Norm> ToRgba for [T; 1] {
    type Channel = T;

    #[inline(always)]
    fn to_rgba(self) -> [T; 4] {
        let [gray] = self;
        [gray, gray, gray, Norm::ONE]
    }
}

pub(crate) trait ToRgb {
    type Channel;
    fn to_rgb(self) -> [Self::Channel; 3];
}
impl<T> ToRgb for [T; 4] {
    type Channel = T;

    #[inline(always)]
    fn to_rgb(self) -> [T; 3] {
        let [r, g, b, _] = self;
        [r, g, b]
    }
}
impl<T: Copy> ToRgb for [T; 1] {
    type Channel = T;

    #[inline(always)]
    fn to_rgb(self) -> [T; 3] {
        let [gray] = self;
        [gray, gray, gray]
    }
}

pub(crate) trait SwapRB {
    fn swap_rb(self) -> Self;
}
impl<T> SwapRB for [T; 3] {
    #[inline(always)]
    fn swap_rb(self) -> Self {
        let [r, g, b] = self;
        [b, g, r]
    }
}
impl<T> SwapRB for [T; 4] {
    #[inline(always)]
    fn swap_rb(self) -> Self {
        let [r, g, b, a] = self;
        [b, g, r, a]
    }
}

pub(crate) trait Norm: Copy + Default {
    const ZERO: Self;
    const HALF: Self;
    const ONE: Self;
}
impl Norm for u8 {
    const ZERO: Self = 0;
    const HALF: Self = 128;
    const ONE: Self = u8::MAX;
}
impl Norm for u16 {
    const ZERO: Self = 0;
    const HALF: Self = 32768;
    const ONE: Self = u16::MAX;
}
impl Norm for f32 {
    const ZERO: Self = 0.0;
    const HALF: Self = 0.5;
    const ONE: Self = 1.0;
}

pub(crate) trait NormConvert<To> {
    fn to(self) -> To;
}
impl<T> NormConvert<T> for T {
    #[inline(always)]
    fn to(self) -> T {
        self
    }
}
impl NormConvert<u16> for u8 {
    #[inline(always)]
    fn to(self) -> u16 {
        n8::n16(self)
    }
}
impl NormConvert<f32> for u8 {
    #[inline(always)]
    fn to(self) -> f32 {
        n8::f32(self)
    }
}
impl NormConvert<u8> for u16 {
    #[inline(always)]
    fn to(self) -> u8 {
        n16::n8(self)
    }
}
impl NormConvert<f32> for u16 {
    #[inline(always)]
    fn to(self) -> f32 {
        n16::f32(self)
    }
}
impl NormConvert<u8> for f32 {
    #[inline(always)]
    fn to(self) -> u8 {
        fp::n8(self)
    }
}
impl NormConvert<u16> for f32 {
    #[inline(always)]
    fn to(self) -> u16 {
        fp::n16(self)
    }
}

pub(crate) trait WithPrecision {
    const PRECISION: Precision;
}
impl WithPrecision for u8 {
    const PRECISION: Precision = Precision::U8;
}
impl WithPrecision for u16 {
    const PRECISION: Precision = Precision::U16;
}
impl WithPrecision for f32 {
    const PRECISION: Precision = Precision::F32;
}

#[cfg(test)]
mod test {
    use crate::decode::convert::fp;

    macro_rules! test_to_unorm {
        ($t:ident, $name:ident, $convert:path, $max_in:expr) => {
            #[test]
            fn $name() {
                assert_eq!($convert(0), 0);
                assert_eq!($convert($max_in), $t::MAX);

                for x in 0..=$max_in {
                    let expected = (x as f64 * $t::MAX as f64 / $max_in as f64).round() as $t;
                    assert_eq!($convert(x), expected);
                }
            }
        };
    }
    test_to_unorm!(u8, n1_to_n8, super::n1::n8, 1);
    test_to_unorm!(u8, n2_to_n8, super::n2::n8, 3);
    test_to_unorm!(u8, n4_to_n8, super::n4::n8, 15);
    test_to_unorm!(u8, n5_to_n8, super::n5::n8, 31);
    test_to_unorm!(u8, n6_to_n8, super::n6::n8, 63);
    test_to_unorm!(u8, n10_to_n8, super::n10::n8, 1023);
    test_to_unorm!(u8, n16_to_n8, super::n16::n8, 65535);
    test_to_unorm!(u16, n1_to_n16, super::n1::n16, 1);
    test_to_unorm!(u16, n2_to_n16, super::n2::n16, 3);
    test_to_unorm!(u16, n4_to_n16, super::n4::n16, 15);
    test_to_unorm!(u16, n5_to_n16, super::n5::n16, 31);
    test_to_unorm!(u16, n6_to_n16, super::n6::n16, 63);
    test_to_unorm!(u16, n8_to_n16, super::n8::n16, 255);
    test_to_unorm!(u16, n10_to_n16, super::n10::n16, 1023);

    macro_rules! test_to_f32 {
        ($name:ident, $convert:path, $max_in:expr) => {
            #[test]
            fn $name() {
                assert_eq!($convert(0), 0.0);
                assert_eq!($convert($max_in), 1.0);

                for x in 0..=$max_in {
                    let expected = (x as f64 / $max_in as f64) as f32;
                    let actual = $convert(x);
                    if expected != actual {
                        let rel_err = (actual as f64 - expected as f64).abs() / expected as f64;
                        const MAX_REL_ERROR: f64 = 1.0 / $max_in as f64 / 100.0;
                        if rel_err > MAX_REL_ERROR {
                            assert_eq!(actual, expected, "failed for x={}, rel_err={}", x, rel_err);
                        }
                    }
                }
            }
        };
    }
    test_to_f32!(n1_to_f32, super::n1::f32, 1);
    test_to_f32!(n2_to_f32, super::n2::f32, 3);
    test_to_f32!(n4_to_f32, super::n4::f32, 15);
    test_to_f32!(n5_to_f32, super::n5::f32, 31);
    test_to_f32!(n6_to_f32, super::n6::f32, 63);
    test_to_f32!(n8_to_f32, super::n8::f32, 255);
    test_to_f32!(n10_to_f32, super::n10::f32, 1023);
    test_to_f32!(n16_to_f32, super::n16::f32, 65535);

    macro_rules! test_to_f32_exact {
        ($name:ident, $convert:path, $max_in:expr) => {
            #[test]
            fn $name() {
                assert_eq!($convert(0), 0.0);
                assert_eq!($convert($max_in), 1.0);

                for x in 0..=$max_in {
                    let expected = (x as f64 / $max_in as f64) as f32;
                    assert_eq!($convert(x), expected, "failed for x={}", x);
                }
            }
        };
    }
    test_to_f32_exact!(n1_to_f32_exact, super::n1::f32, 1);
    test_to_f32_exact!(n2_to_f32_exact, super::n2::f32, 3);
    test_to_f32_exact!(n4_to_f32_exact, super::n4::f32_exact, 15);
    test_to_f32_exact!(n5_to_f32_exact, super::n5::f32_exact, 31);
    test_to_f32_exact!(n6_to_f32_exact, super::n6::f32_exact, 63);
    test_to_f32_exact!(n8_to_f32_exact, super::n8::f32_exact, 255);
    test_to_f32_exact!(n10_to_f32_exact, super::n10::f32_exact, 1023);
    test_to_f32_exact!(n16_to_f32_exact, super::n16::f32_exact, 65535);

    macro_rules! test_snorm_to_unorm {
        ($in:ident / $in_unsigned:ident => $t:ident, $name:ident, $convert:path) => {
            #[test]
            fn $name() {
                assert_eq!($convert($in::MIN as $in_unsigned), 0);
                assert_eq!($convert(-$in::MAX as $in_unsigned), 0);
                assert_eq!($convert(0 as $in as $in_unsigned), $t::MAX / 2 + 1);
                assert_eq!($convert($in::MAX as $in_unsigned), $t::MAX);

                for x in 0..=$in_unsigned::MAX {
                    let xi = x as $in;
                    let expected = ((xi.max(-$in::MAX) as f64 / $in::MAX as f64 + 1.0) / 2.0
                        * $t::MAX as f64)
                        .round() as $t;
                    assert_eq!($convert(x), expected, "failed for x={} (u{})", xi, x);
                }
            }
        };
    }
    test_snorm_to_unorm!(i8/u8 => u8, s8_to_n8, super::s8::n8);
    test_snorm_to_unorm!(i16/u16 => u8, s16_to_n8, super::s16::n8);
    test_snorm_to_unorm!(i8/u8 => u16, s8_to_n16, super::s8::n16);
    test_snorm_to_unorm!(i16/u16 => u16, s16_to_n16, super::s16::n16);

    macro_rules! test_snorm_to_f32 {
        ($in:ident / $in_unsigned:ident, $name:ident, $convert:path) => {
            #[test]
            fn $name() {
                assert_eq!($convert($in::MIN as $in_unsigned), 0.0);
                assert_eq!($convert(-$in::MAX as $in_unsigned), 0.0);
                assert_eq!($convert(0 as $in as $in_unsigned), 0.5);
                assert_eq!($convert($in::MAX as $in_unsigned), 1.0);

                for x in 0..=$in_unsigned::MAX {
                    let xi = x as $in;
                    let expected =
                        ((xi.max(-$in::MAX) as f64 / $in::MAX as f64 + 1.0) / 2.0) as f32;
                    let actual = $convert(x);
                    if expected != actual {
                        let rel_err = (actual as f64 - expected as f64).abs() / expected as f64;
                        const MAX_REL_ERROR: f64 = 1.0 / $in::MAX as f64 / 100.0;
                        if rel_err > MAX_REL_ERROR {
                            assert_eq!(
                                actual, expected,
                                "failed for x={} (u{}), rel_err={}",
                                xi, x, rel_err
                            );
                        }
                    }
                }
            }
        };
    }
    test_snorm_to_f32!(i8 / u8, s8_to_uf32, super::s8::uf32);
    test_snorm_to_f32!(i16 / u16, s16_to_uf32, super::s16::uf32);

    macro_rules! test_snorm_to_f32_exact {
        ($in:ident / $in_unsigned:ident, $name:ident, $convert:path) => {
            #[test]
            fn $name() {
                assert_eq!($convert($in::MIN as $in_unsigned), 0.0);
                assert_eq!($convert(-$in::MAX as $in_unsigned), 0.0);
                assert_eq!($convert(0 as $in as $in_unsigned), 0.5);
                assert_eq!($convert($in::MAX as $in_unsigned), 1.0);

                for x in 0..=$in_unsigned::MAX {
                    let xi = x as $in;
                    let expected =
                        ((xi.max(-$in::MAX) as f64 / $in::MAX as f64 + 1.0) / 2.0) as f32;
                    assert_eq!($convert(x), expected, "failed for x={} (u{})", xi, x);
                }
            }
        };
    }
    test_snorm_to_f32_exact!(i8 / u8, s8_to_uf32_exact, super::s8::uf32_exact);
    test_snorm_to_f32_exact!(i16 / u16, s16_to_uf32_exact, super::s16::uf32_exact);

    #[test]
    fn fp_to_n8() {
        use super::fp;

        assert_eq!(fp::n8(f32::NEG_INFINITY), 0);
        assert_eq!(fp::n8(-1000.0), 0);
        assert_eq!(fp::n8(-1.0), 0);
        assert_eq!(fp::n8(0.0), 0);
        assert_eq!(fp::n8(0.5), 128);
        assert_eq!(fp::n8(1.0), 255);
        assert_eq!(fp::n8(1000.0), 255);
        assert_eq!(fp::n8(f32::INFINITY), 255);

        assert_eq!(fp::n8(f32::NAN), 0);
    }
    #[test]
    fn fp_to_n16() {
        use super::fp;

        assert_eq!(fp::n16(f32::NEG_INFINITY), 0);
        assert_eq!(fp::n16(-1000.0), 0);
        assert_eq!(fp::n16(-1.0), 0);
        assert_eq!(fp::n16(0.0), 0);
        assert_eq!(fp::n16(0.5), 32768);
        assert_eq!(fp::n16(1.0), u16::MAX);
        assert_eq!(fp::n16(1000.0), u16::MAX);
        assert_eq!(fp::n16(f32::INFINITY), u16::MAX);

        assert_eq!(fp::n16(f32::NAN), 0);
    }

    #[test]
    fn fp16_to_n8() {
        for i in 0..=u16::MAX {
            let expected = super::fp::n8(super::fp16::f32(i));
            let actual = super::fp16::n8(i);
            assert_eq!(actual, expected, "failed for i={}", i);
        }
    }
    #[test]
    fn fp16_to_n16() {
        for i in 0..=u16::MAX {
            let expected = super::fp::n16(super::fp16::f32(i));
            let actual = super::fp16::n16(i);
            assert_eq!(actual, expected, "failed for i={}", i);
        }
    }

    #[test]
    fn fp11_to_n8() {
        for i in 0..2048 {
            let expected = super::fp::n8(super::fp11::f32(i));
            let actual = super::fp11::n8(i);
            assert_eq!(actual, expected, "failed for i={}", i);
        }
    }
    #[test]
    fn fp11_to_n16() {
        for i in 0..2048 {
            let expected = super::fp::n16(super::fp11::f32(i));
            let actual = super::fp11::n16(i);
            assert_eq!(actual, expected, "failed for i={}", i);
        }
    }

    #[test]
    fn fp10_to_n8() {
        for i in 0..1024 {
            let expected = super::fp::n8(super::fp10::f32(i));
            let actual = super::fp10::n8(i);
            assert_eq!(actual, expected, "failed for i={}", i);
        }
    }
    #[test]
    fn fp10_to_n16() {
        for i in 0..1024 {
            let expected = super::fp::n16(super::fp10::f32(i));
            let actual = super::fp10::n16(i);
            assert_eq!(actual, expected, "failed for i={}", i);
        }
    }

    #[test]
    fn xr10_to_n8() {
        for i in 0..1024 {
            let expected = super::fp::n8(super::xr10::f32(i));
            let actual = super::xr10::n8(i);
            assert_eq!(actual, expected, "failed for i={}", i);
        }
    }
    #[test]
    fn xr10_to_n16() {
        for i in 0..1024 {
            let expected = super::fp::n16(super::xr10::f32(i));
            let actual = super::xr10::n16(i);
            assert_eq!(actual, expected, "failed for i={}", i);
        }
    }

    #[test]
    fn rgb9995f_to_n8() {
        // This will exhaustively test all exponent and B mantissa values.
        // R and G won't be tested, since they behave the same as B.
        for e in 0..=31 {
            for b in 0..=255 {
                let i = e << 27 | b << 18;

                let expected = super::rgb9995f::f32(i).map(fp::n8);
                let actual = super::rgb9995f::n8(i);
                assert_eq!(actual, expected, "failed for exp={} mant={}", e, b);
            }
        }
    }
    #[test]
    fn rgb9995f_to_n16() {
        // This will exhaustively test all exponent and B mantissa values.
        // R and G won't be tested, since they behave the same as B.
        for e in 0..=31 {
            for b in 0..=255 {
                let i = e << 27 | b << 18;

                let expected = super::rgb9995f::f32(i).map(fp::n16);
                let actual = super::rgb9995f::n16(i);
                assert_eq!(actual, expected, "failed for exp={} mant={}", e, b);
            }
        }
    }
}
