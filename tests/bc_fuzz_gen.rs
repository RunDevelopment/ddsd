//! This module is not a test per se, but a script for generating test files.
//!
//! This script is responsible for generating random block-compression images
//! that exhaustively test certain properties.

use std::{fs::File, io::Write};

use ddsd::*;
use rand::SeedableRng;

fn create_bc_data<const N: usize>(
    mut w: impl Write,
    blocks_x: u32,
    blocks_y: u32,
    format: DxgiFormat,
    mut gen: impl FnMut(u32, u32) -> [u8; N],
) -> Result<(), std::io::Error> {
    let pixel_info = PixelInfo::try_from(format).unwrap();
    if let PixelInfo::Block {
        bytes_per_block,
        block_size,
    } = pixel_info
    {
        assert_eq!(N, bytes_per_block as usize);
        assert_eq!((4, 4), block_size);
    } else {
        panic!("Not a block format");
    }

    let write_u32 = |w: &mut dyn Write, x: u32| w.write_all(&x.to_le_bytes());

    // Header
    w.write_all(&Header::MAGIC)?;
    write_u32(&mut w, 124)?;
    write_u32(&mut w, DdsFlags::REQUIRED.bits())?;
    write_u32(&mut w, blocks_y * 4)?; // height
    write_u32(&mut w, blocks_x * 4)?; // width
    write_u32(&mut w, 0)?; // pitch_or_linear_size
    write_u32(&mut w, 0)?; // depth
    write_u32(&mut w, 0)?; // mip_map_count
    for _ in 0..11 {
        write_u32(&mut w, 0)?;
    }
    write_u32(&mut w, 32)?; // size
    write_u32(&mut w, PixelFormatFlags::FOURCC.bits())?; // flags
    write_u32(&mut w, FourCC::DX10.into())?; // four_cc
    write_u32(&mut w, 0)?; // rgb_bit_count
    write_u32(&mut w, 0)?; // r_bit_mask
    write_u32(&mut w, 0)?; // g_bit_mask
    write_u32(&mut w, 0)?; // b_bit_mask
    write_u32(&mut w, 0)?; // a_bit_mask
    write_u32(&mut w, DdsCaps::TEXTURE.bits())?; // caps
    write_u32(&mut w, 0)?; // caps2
    write_u32(&mut w, 0)?; // caps3
    write_u32(&mut w, 0)?; // caps4
    write_u32(&mut w, 0)?; // reserved2
    write_u32(&mut w, format.into())?; // dxgiFormat
    write_u32(&mut w, ResourceDimension::Texture2D.into())?; // resource_dimension
    write_u32(&mut w, 0)?; // misc_flag
    write_u32(&mut w, 1)?; // array_size
    write_u32(&mut w, 0)?; // misc_flags2

    // now for the actual data
    for y in 0..blocks_y {
        for x in 0..blocks_x {
            let block = gen(x, y);
            w.write_all(&block)?;
        }
    }

    Ok(())
}

fn create_rng() -> impl rand::Rng {
    rand_chacha::ChaChaRng::seed_from_u64(123456789)
}

fn random_block<const N: usize>(rng: &mut impl rand::Rng) -> [u8; N] {
    let mut block = [0; N];
    rng.fill_bytes(&mut block);
    block
}

fn set_lsb(block: &mut u128, count: u8, value: u128) {
    *block &= !((1 << count) - 1);
    *block |= value;
}
fn push_mode(block: &mut u128, mode: u8) {
    *block = (*block << (mode + 1)) | (1 << mode);
}

#[test]
fn bc7_mode_0() {
    // Mode 0 has 4 partition bits that we want to check exhaustively.
    let mut file = File::create("test-data/images/bc fuzz/bc7 mode 0.dds").unwrap();
    let mut rng = create_rng();
    create_bc_data(&mut file, 256, 16, DxgiFormat::BC7_UNORM, |_, y| {
        let mut block = u128::from_le_bytes(random_block(&mut rng));

        set_lsb(&mut block, 4, y as u128);
        push_mode(&mut block, 0);
        block.to_le_bytes()
    })
    .unwrap();
}

#[test]
fn bc7_mode_1_2_3() {
    // Mode 1/2/3 have 6 partition bits and otherwise nothing interesting
    for mode in 1..=3 {
        let mut file =
            File::create(format!("test-data/images/bc fuzz/bc7 mode {}.dds", mode)).unwrap();
        let mut rng = create_rng();
        create_bc_data(&mut file, 128, 64, DxgiFormat::BC7_UNORM, |_, y| {
            let mut block = u128::from_le_bytes(random_block(&mut rng));

            set_lsb(&mut block, 6, y as u128);
            push_mode(&mut block, mode);
            block.to_le_bytes()
        })
        .unwrap();
    }
}

#[test]
fn bc7_mode_4() {
    // Mode 4 has 2 bits rotations and 1 index mode bit
    let mut file = File::create("test-data/images/bc fuzz/bc7 mode 4.dds").unwrap();
    let mut rng = create_rng();
    create_bc_data(&mut file, 256, 8, DxgiFormat::BC7_UNORM, |_, y| {
        let mut block = u128::from_le_bytes(random_block(&mut rng));

        set_lsb(&mut block, 3, y as u128);
        push_mode(&mut block, 4);
        block.to_le_bytes()
    })
    .unwrap();
}

#[test]
fn bc7_mode_5() {
    // Mode 5 has 2 bits rotations
    let mut file = File::create("test-data/images/bc fuzz/bc7 mode 5.dds").unwrap();
    let mut rng = create_rng();
    create_bc_data(&mut file, 256, 4, DxgiFormat::BC7_UNORM, |_, y| {
        let mut block = u128::from_le_bytes(random_block(&mut rng));

        set_lsb(&mut block, 2, y as u128);
        push_mode(&mut block, 5);
        block.to_le_bytes()
    })
    .unwrap();
}

#[test]
fn bc7_mode_6() {
    // Mode 6 has no special bits, so pure random is enough
    let mut file = File::create("test-data/images/bc fuzz/bc7 mode 6.dds").unwrap();
    let mut rng = create_rng();
    create_bc_data(&mut file, 64, 64, DxgiFormat::BC7_UNORM, |_, _| {
        let mut block = u128::from_le_bytes(random_block(&mut rng));
        push_mode(&mut block, 6);
        block.to_le_bytes()
    })
    .unwrap();
}

#[test]
fn bc7_mode_7() {
    // Mode 7 has 6 partition bits that we want to check exhaustively.
    let mut file = File::create("test-data/images/bc fuzz/bc7 mode 7.dds").unwrap();
    let mut rng = create_rng();
    create_bc_data(&mut file, 128, 64, DxgiFormat::BC7_UNORM, |_, y| {
        let mut block = u128::from_le_bytes(random_block(&mut rng));

        set_lsb(&mut block, 6, y as u128);
        push_mode(&mut block, 7);
        block.to_le_bytes()
    })
    .unwrap();
}
