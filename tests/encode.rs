use std::{
    fs::File,
    io::{Cursor, Read},
    path::{Path, PathBuf},
};

use ddsd::*;
use util::test_data_dir;
use Precision::*;

mod util;

const FORMATS: &[EncodeFormat] = &[
    // uncompressed formats
    EncodeFormat::R8G8B8_UNORM,
    EncodeFormat::B8G8R8_UNORM,
    EncodeFormat::R8G8B8A8_UNORM,
    EncodeFormat::R8G8B8A8_SNORM,
    EncodeFormat::B8G8R8A8_UNORM,
    EncodeFormat::B8G8R8X8_UNORM,
    EncodeFormat::B5G6R5_UNORM,
    EncodeFormat::B5G5R5A1_UNORM,
    EncodeFormat::B4G4R4A4_UNORM,
    EncodeFormat::A4B4G4R4_UNORM,
    EncodeFormat::R8_SNORM,
    EncodeFormat::R8_UNORM,
    EncodeFormat::R8G8_UNORM,
    EncodeFormat::R8G8_SNORM,
    EncodeFormat::A8_UNORM,
    EncodeFormat::R16_UNORM,
    EncodeFormat::R16_SNORM,
    EncodeFormat::R16G16_UNORM,
    EncodeFormat::R16G16_SNORM,
    EncodeFormat::R16G16B16A16_UNORM,
    EncodeFormat::R16G16B16A16_SNORM,
    EncodeFormat::R10G10B10A2_UNORM,
    EncodeFormat::R11G11B10_FLOAT,
    EncodeFormat::R9G9B9E5_SHAREDEXP,
    EncodeFormat::R16_FLOAT,
    EncodeFormat::R16G16_FLOAT,
    EncodeFormat::R16G16B16A16_FLOAT,
    EncodeFormat::R32_FLOAT,
    EncodeFormat::R32G32_FLOAT,
    EncodeFormat::R32G32B32_FLOAT,
    EncodeFormat::R32G32B32A32_FLOAT,
    EncodeFormat::R10G10B10_XR_BIAS_A2_UNORM,
    EncodeFormat::AYUV,
    EncodeFormat::Y410,
    EncodeFormat::Y416,
    // sub-sampled formats
    EncodeFormat::R1_UNORM,
    EncodeFormat::R8G8_B8G8_UNORM,
    EncodeFormat::G8R8_G8B8_UNORM,
    EncodeFormat::UYVY,
    EncodeFormat::YUY2,
    EncodeFormat::Y210,
    EncodeFormat::Y216,
    // block compression formats
    EncodeFormat::BC1_UNORM,
    EncodeFormat::BC2_UNORM,
    EncodeFormat::BC2_UNORM_PREMULTIPLIED_ALPHA,
    EncodeFormat::BC3_UNORM,
    EncodeFormat::BC3_UNORM_PREMULTIPLIED_ALPHA,
    EncodeFormat::BC4_UNORM,
    EncodeFormat::BC4_SNORM,
    EncodeFormat::BC5_UNORM,
    EncodeFormat::BC5_SNORM,
    EncodeFormat::BC6H_UF16,
    EncodeFormat::BC6H_SF16,
    EncodeFormat::BC7_UNORM,
];

#[test]
fn encode_base() {
    let base = util::read_png_u8(&util::test_data_dir().join("base.png")).unwrap();
    assert!(base.channels == Channels::Rgba);
    let base_u16 = base
        .data
        .iter()
        .map(|&x| u16::from(x) * 257)
        .collect::<Vec<_>>();
    let base_f32 = base
        .data
        .iter()
        .map(|&x| f32::from(x) / 255.)
        .collect::<Vec<_>>();

    let test = |format: EncodeFormat| -> Result<(), Box<dyn std::error::Error>> {
        let header: Header = if let Ok(dxgi_format) = format.try_into() {
            Header::new_image(base.size.width, base.size.height, dxgi_format)
        } else if let Ok(format) = format.try_into() {
            Dx9Header::new_image(base.size.width, base.size.height, format).into()
        } else {
            // skip non-DX10 formats for now
            return Ok(());
        };

        let mut output = Vec::new();
        output.extend_from_slice(&Header::MAGIC);
        header.to_raw().write(&mut output).unwrap();

        // and now the image data
        if format.precision() == Precision::U16 {
            format.encode(
                &mut output,
                base.size,
                ColorFormat::RGBA_U16,
                util::as_bytes(&base_u16),
            )?;
        } else if format.precision() == Precision::F32 {
            format.encode(
                &mut output,
                base.size,
                ColorFormat::RGBA_F32,
                util::as_bytes(&base_f32),
            )?;
        } else {
            format.encode(&mut output, base.size, ColorFormat::RGBA_U8, &base.data)?;
        }

        // write to disk
        let name = format!("{:?}.dds", format);
        let path = test_data_dir().join("output-encode").join(&name);
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(&path, &output)?;

        Ok(())
    };

    let mut failed_count = 0;
    for format in FORMATS.iter().copied() {
        if let Err(e) = test(format) {
            eprintln!("Failed to encode {:?}: {}", format, e);
            failed_count += 1;
        }
    }
    if failed_count > 0 {
        panic!("{} tests failed", failed_count);
    }
}
