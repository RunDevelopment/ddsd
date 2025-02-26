use ddsd::*;

use std::{fs::File, num::NonZero, path::PathBuf};

mod util;

fn get_header_byte_len(header: &Header) -> u64 {
    4 + 124 + if header.dxt10.is_some() { 20 } else { 0 }
}

#[test]
fn parse_data_layout_of_all_dds_files() {
    for dds_path in util::example_dds_files() {
        let mut file = File::open(&dds_path).expect("Failed to open file");
        let file_len = file.metadata().unwrap().len();

        let mut options = Options::default();
        options.permissive = true;
        options.file_len = Some(file_len);

        let decoder_result = DdsDecoder::new_with(&mut file, &options);
        let decoder = match decoder_result {
            Ok(decoder) => decoder,
            Err(e) => panic!("Failed to decode {}\nFile: {:?}", e, file),
        };

        let header = decoder.header();

        // skip cubemaps with array_size == 6 for now
        // https://github.com/RunDevelopment/ddsd/issues/4
        if let Some(dx10) = &header.dxt10 {
            if dx10.array_size == 6 {
                continue;
            }
        }

        let data_len = file_len - get_header_byte_len(header);
        let expected_len = decoder.layout().data_len();
        assert_eq!(data_len, expected_len, "File: {:?}", &dds_path);
    }
}

#[test]
fn full_layout_snapshot() {
    let mut files: Vec<_> = util::example_dds_files()
        .into_iter()
        .map(|p| {
            let name = p
                .strip_prefix(util::test_data_dir())
                .unwrap()
                .to_str()
                .unwrap()
                .replace("\\", "/")
                .trim_matches('/')
                .to_owned();
            (name, p)
        })
        .collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));

    fn strict_header(dds_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let mut file = File::open(dds_path)?;
        let file_len = file.metadata()?.len();

        let mut options = Options::default();
        options.permissive = false;
        let decoder = DdsDecoder::new_with(&mut file, &options)?;

        let data_len = file_len - get_header_byte_len(decoder.header());
        if data_len != decoder.layout().data_len() {
            return Err("Data length mismatch".into());
        }

        Ok(())
    }

    fn collect_info(dds_path: &PathBuf) -> Result<String, Box<dyn std::error::Error>> {
        let mut file = File::open(dds_path)?;
        let file_len = file.metadata()?.len();

        let mut options = Options::default();
        options.permissive = true;
        options.file_len = Some(file_len);
        let decoder = DdsDecoder::new_with(&mut file, &options)?;

        let header = decoder.header();
        let format = decoder.format();
        let layout = decoder.layout();

        let mut output = String::new();

        if let Err(e) = strict_header(dds_path) {
            output.push_str(&format!("Error if strict: {}\n\n", e));
        }

        let data_len = file_len - get_header_byte_len(header);
        if data_len != layout.data_len() {
            output.push_str(&format!(
                "Data length mismatch: {} != {}\n\n",
                data_len,
                layout.data_len()
            ));
        }

        // HEADER
        output.push_str("Header:\n");
        output.push_str(&format!("    flags: {:?}\n", header.flags));
        if let Some(d) = header.depth {
            output.push_str(&format!(
                "    w/h/d: {:?} x {:?} x {:?}\n",
                header.width, header.height, d
            ));
        } else {
            output.push_str(&format!(
                "    w/h: {:?} x {:?}\n",
                header.width, header.height
            ));
        }
        output.push_str(&format!("    mipmap_count: {:?}\n", header.mipmap_count));
        if let Some(four_cc) = header.pixel_format.four_cc {
            output.push_str(&format!("    pixel_format: {:?}\n", four_cc));
        } else {
            output.push_str("    pixel_format:\n");
            output.push_str(&format!("        flags: {:?}\n", header.pixel_format.flags));
            output.push_str(&format!(
                "        rgb_bit_count: {:?}\n",
                header.pixel_format.rgb_bit_count
            ));
            output.push_str(&format!(
                "        bit_mask: r:0x{:x} g:0x{:x} b:0x{:x} a:0x{:x}\n",
                header.pixel_format.r_bit_mask,
                header.pixel_format.g_bit_mask,
                header.pixel_format.b_bit_mask,
                header.pixel_format.a_bit_mask
            ));
        }
        output.push_str(&format!("    caps: {:?}\n", header.caps));
        output.push_str(&format!("    caps2: {:?}\n", header.caps2));
        if let Some(dxt10) = &header.dxt10 {
            output.push_str("    dxt10:\n");
            output.push_str(&format!("        dxgi_format: {:?}\n", dxt10.dxgi_format));
            output.push_str(&format!(
                "        resource_dimension: {:?}\n",
                dxt10.resource_dimension
            ));
            output.push_str(&format!("        misc_flag: {:?}\n", dxt10.misc_flag));
            output.push_str(&format!("        array_size: {:?}\n", dxt10.array_size));
        }

        // FORMAT INFO
        output.push_str("\nPixel Format:\n");
        output.push_str(&format!("    format: {:?}", format));
        if decoder.is_srgb() {
            output.push_str(" (sRGB)");
        }
        output.push_str(&format!(
            "\n    pixel_info: {:?}\n",
            PixelInfo::from(format)
        ));

        // LAYOUT
        output.push_str("\nLayout: ");
        match layout {
            DataLayout::Texture(texture) => {
                output.push_str(&format!("Texture ({} bytes)\n", texture.data_len()));
                for (i, surface) in texture.iter_mips().enumerate() {
                    output.push_str(&format!(
                        "    Surface[{i}] {}x{} ({} bytes)\n",
                        surface.width(),
                        surface.height(),
                        surface.data_len()
                    ));
                }
            }
            DataLayout::Volume(volume) => {
                output.push_str(&format!("Volume ({} bytes)\n", volume.data_len()));
                for (i, volume) in volume.iter_mips().enumerate() {
                    output.push_str(&format!(
                        "    Volume[{i}] {}x{}x{} ({} bytes)\n",
                        volume.width(),
                        volume.height(),
                        volume.depth(),
                        volume.data_len()
                    ));
                    for (i, surface) in volume.iter_depth_slices().enumerate() {
                        output.push_str(&format!(
                            "        Surface[{i}] {}x{} ({} bytes)\n",
                            surface.width(),
                            surface.height(),
                            surface.data_len()
                        ));
                    }
                }
            }
            DataLayout::TextureArray(texture_array) => {
                output.push_str(&format!(
                    "TextureArray len:{} kind:{:?} ({} bytes)\n",
                    texture_array.len(),
                    texture_array.kind(),
                    texture_array.data_len()
                ));
                for (i, texture) in texture_array.iter().enumerate() {
                    output.push_str(&format!(
                        "    Texture[{i}] ({} bytes)\n",
                        texture.data_len()
                    ));
                    for (i, surface) in texture.iter_mips().enumerate() {
                        output.push_str(&format!(
                            "        Surface[{i}] {}x{} ({} bytes)\n",
                            surface.width(),
                            surface.height(),
                            surface.data_len()
                        ));
                    }
                }
            }
        }

        Ok(output)
    }

    // create expected info
    let mut output = String::new();
    for (name, dds_path) in files {
        output.push_str(&name);
        output.push('\n');

        let info = match collect_info(&dds_path) {
            Ok(info) => info,
            Err(e) => format!("Error: {}", e),
        };

        for line in info.lines() {
            if line.is_empty() {
                output.push('\n');
            } else {
                output.push_str(&format!("    {}\n", line));
            }
        }

        output.push('\n');
        output.push('\n');
        output.push('\n');
    }
    output = output.replace("\r\n", "\n");

    // compare to snapshot
    let snapshot_file = util::test_data_dir().join("layout_snapshot.txt");
    let file_exists = snapshot_file.exists();
    let mut native_line_ends = "\n";

    if file_exists {
        let mut snapshot = std::fs::read_to_string(&snapshot_file).unwrap();
        if snapshot.contains("\r\n") {
            native_line_ends = "\r\n";
            snapshot = snapshot.replace("\r\n", "\n");
        }

        if output.trim() == snapshot.trim() {
            // all ok
            return;
        }
    }

    // write snapshot
    if !util::is_ci() {
        println!("Writing snapshot: {:?}", snapshot_file);

        std::fs::create_dir_all(snapshot_file.parent().unwrap()).unwrap();
        std::fs::write(&snapshot_file, output.replace("\n", native_line_ends)).unwrap();
    }

    if !file_exists {
        panic!("Snapshot file not found: {:?}", snapshot_file);
    } else {
        panic!("Layout snapshot differs from expected.");
    }
}

#[test]
fn iter_and_get_volume() {
    let header_volume = Header {
        flags: DdsFlags::REQUIRED | DdsFlags::MIPMAP_COUNT | DdsFlags::DEPTH,
        height: 128,
        width: 256,
        depth: Some(4),
        mipmap_count: NonZero::new(5).unwrap(),
        pixel_format: PixelFormat::new_four_cc(FourCC::DX10),
        caps: DdsCaps::REQUIRED | DdsCaps::COMPLEX,
        caps2: DdsCaps2::empty(),
        dxt10: Some(HeaderDxt10 {
            dxgi_format: DxgiFormat::R8G8B8A8_UNORM,
            resource_dimension: ResourceDimension::Texture3D,
            misc_flag: MiscFlags::empty(),
            array_size: 1,
            misc_flags2: MiscFlags2::empty(),
        }),
    };

    let layout = DataLayout::from_header(&header_volume).unwrap();
    assert!(matches!(layout, DataLayout::Volume(_)));
    assert!(layout.texture().is_none());
    assert!(layout.texture_array().is_none());

    let volume = layout.volume().unwrap();

    let from_iter: Vec<VolumeDescriptor> = volume.iter_mips().collect();
    let from_get: Vec<VolumeDescriptor> = (0..u8::MAX).map_while(|i| volume.get(i)).collect();
    assert_eq!(from_iter, from_get);

    assert_eq!(volume.main(), volume.get(0).unwrap());

    for volume in volume.iter_mips() {
        let from_iter: Vec<SurfaceDescriptor> = volume.iter_depth_slices().collect();
        let from_get: Vec<SurfaceDescriptor> = (0..u32::MAX)
            .map_while(|i| volume.get_depth_slice(i))
            .collect();
        assert_eq!(from_iter, from_get);
    }
}

#[test]
fn iter_and_get_texture_array() {
    let header_texture_array = Header {
        flags: DdsFlags::REQUIRED | DdsFlags::MIPMAP_COUNT,
        height: 128,
        width: 256,
        depth: None,
        mipmap_count: NonZero::new(5).unwrap(),
        pixel_format: PixelFormat::new_four_cc(FourCC::DX10),
        caps: DdsCaps::REQUIRED,
        caps2: DdsCaps2::empty(),
        dxt10: Some(HeaderDxt10 {
            dxgi_format: DxgiFormat::R8G8B8A8_UNORM,
            resource_dimension: ResourceDimension::Texture2D,
            misc_flag: MiscFlags::empty(),
            array_size: 4,
            misc_flags2: MiscFlags2::empty(),
        }),
    };

    let layout = DataLayout::from_header(&header_texture_array).unwrap();
    assert!(matches!(layout, DataLayout::TextureArray(_)));
    assert!(layout.texture().is_none());
    assert!(layout.volume().is_none());

    let array = layout.texture_array().unwrap();
    assert!(array.len() == 4);
    assert!(!array.is_empty());

    let from_iter: Vec<Texture> = array.iter().collect();
    let from_get: Vec<Texture> = (0..usize::MAX).map_while(|i| array.get(i)).collect();
    assert_eq!(from_iter, from_get);

    for texture in array.iter() {
        let from_iter: Vec<SurfaceDescriptor> = texture.iter_mips().collect();
        let from_get: Vec<SurfaceDescriptor> = (0..u8::MAX).map_while(|i| texture.get(i)).collect();
        assert_eq!(from_iter, from_get);

        assert_eq!(texture.main(), texture.get(0).unwrap());
    }
}

#[test]
fn empty_array() {
    #![allow(clippy::len_zero)]

    let header_texture_array = Header {
        flags: DdsFlags::REQUIRED | DdsFlags::MIPMAP_COUNT,
        height: 128,
        width: 256,
        depth: None,
        mipmap_count: NonZero::new(5).unwrap(),
        pixel_format: PixelFormat::new_four_cc(FourCC::DX10),
        caps: DdsCaps::REQUIRED,
        caps2: DdsCaps2::empty(),
        dxt10: Some(HeaderDxt10 {
            dxgi_format: DxgiFormat::R8G8B8A8_UNORM,
            resource_dimension: ResourceDimension::Texture2D,
            misc_flag: MiscFlags::empty(),
            array_size: 0, // empty
            misc_flags2: MiscFlags2::empty(),
        }),
    };

    let layout = DataLayout::from_header(&header_texture_array).unwrap();
    let array = layout.texture_array().unwrap();

    assert!(array.len() == 0);
    assert!(array.is_empty());
    assert!(array.iter().next().is_none());
    assert!(array.data_len() == 0);
}
