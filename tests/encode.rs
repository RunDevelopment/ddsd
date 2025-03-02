use std::{
    fs::File,
    io::{Cursor, Read},
    path::{Path, PathBuf},
};

use ddsd::*;
use util::{test_data_dir, WithPrecision};
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

fn encode_image<T: WithPrecision + util::Castable, W: std::io::Write>(
    image: &util::Image<T>,
    format: EncodeFormat,
    writer: &mut W,
    options: &EncodeOptions,
) -> Result<(), EncodeError> {
    format.encode(writer, image.size, image.color(), image.as_bytes(), options)
}

#[test]
fn encode_base() {
    let base_u8 = util::read_png_u8(&util::test_data_dir().join("base.png")).unwrap();
    assert!(base_u8.channels == Channels::Rgba);
    let base_u16 = base_u8.to_u16();
    let base_f32 = base_u8.to_f32();

    let test = |format: EncodeFormat| -> Result<(), Box<dyn std::error::Error>> {
        let header: Header = if let Ok(dxgi_format) = format.try_into() {
            Header::new_image(base_u8.size.width, base_u8.size.height, dxgi_format)
        } else if let Ok(format) = format.try_into() {
            Dx9Header::new_image(base_u8.size.width, base_u8.size.height, format).into()
        } else {
            // skip non-DX10 formats for now
            return Ok(());
        };

        let mut output = Vec::new();
        output.extend_from_slice(&Header::MAGIC);
        header.to_raw().write(&mut output).unwrap();

        let options = EncodeOptions::default();

        // and now the image data
        if format.precision() == Precision::U16 {
            encode_image(&base_u16, format, &mut output, &options)?;
        } else if format.precision() == Precision::F32 {
            encode_image(&base_f32, format, &mut output, &options)?;
        } else {
            encode_image(&base_u8, format, &mut output, &options)?;
        }

        // write to disk
        let name = format!("{:?}.dds", format);
        let path = test_data_dir().join("output-encode/base").join(&name);
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

#[test]
fn encode_dither() {
    let test = |format: EncodeFormat,
                image: &util::Image<f32>,
                name: &str|
     -> Result<(), Box<dyn std::error::Error>> {
        let header: Header = if let Ok(dxgi_format) = format.try_into() {
            Header::new_image(image.size.width, image.size.height, dxgi_format)
        } else if let Ok(format) = format.try_into() {
            Dx9Header::new_image(image.size.width, image.size.height, format).into()
        } else {
            // skip non-DX10 formats for now
            return Ok(());
        };

        let mut output = Vec::new();
        output.extend_from_slice(&Header::MAGIC);
        header.to_raw().write(&mut output).unwrap();

        let mut options = EncodeOptions::default();
        options.dither = DitheredChannels::All;
        encode_image(image, format, &mut output, &options)?;

        // write to disk
        let name = format!("{:?} {}.dds", format, name);
        let path = test_data_dir().join("output-encode/dither").join(&name);
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(&path, &output)?;

        Ok(())
    };

    let base = util::read_png_u8(&util::test_data_dir().join("base.png"))
        .unwrap()
        .to_f32();
    let twirl = util::read_png_u8(&util::test_data_dir().join("color-twirl.png"))
        .unwrap()
        .to_f32();

    let mut failed_count = 0;
    for format in FORMATS
        .iter()
        .copied()
        .filter(|f| f.supports_dither() != DitheredChannels::None)
    {
        let dither = format.supports_dither();

        if let Err(e) = test(format, &base, "base") {
            eprintln!("Failed to encode {:?}: {}", format, e);
            failed_count += 1;
        }

        if dither != DitheredChannels::AlphaOnly {
            if let Err(e) = test(format, &twirl, "twirl") {
                eprintln!("Failed to encode {:?}: {}", format, e);
                failed_count += 1;
            }
        }
    }
    if failed_count > 0 {
        panic!("{} tests failed", failed_count);
    }
}
