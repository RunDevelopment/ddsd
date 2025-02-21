use std::{collections::HashSet, fs::File};

use ddsd::*;

mod util;

fn get_headers() -> Vec<Header> {
    let header_set: HashSet<Header> = util::example_dds_files()
        .into_iter()
        .map(|p| {
            let mut file = File::open(p)?;
            let file_len = file.metadata()?.len();

            let mut options = Options::default();
            options.permissive = true;
            options.file_len = Some(file_len);
            Header::read(&mut file, &options)
        })
        .filter_map(Result::ok)
        .collect();

    let mut headers: Vec<Header> = header_set.into_iter().collect();
    headers.sort_by(|a, b| {
        let cmp = a
            .width
            .cmp(&b.width)
            .then(a.height.cmp(&b.height))
            .then(a.depth.cmp(&b.depth))
            .then(a.mipmap_count.get().cmp(&b.mipmap_count.get()))
            .then(a.caps2.bits().cmp(&b.caps2.bits()));

        match (&a.format, &b.format) {
            (PixelFormat::FourCC(a), PixelFormat::FourCC(b)) => a.0.cmp(&b.0).then(cmp),
            (PixelFormat::Mask(a), PixelFormat::Mask(b)) => a
                .flags
                .bits()
                .cmp(&b.flags.bits())
                .then(a.rgb_bit_count.cmp(&b.rgb_bit_count))
                .then(a.r_bit_mask.cmp(&b.r_bit_mask))
                .then(a.g_bit_mask.cmp(&b.g_bit_mask))
                .then(a.b_bit_mask.cmp(&b.b_bit_mask))
                .then(a.a_bit_mask.cmp(&b.a_bit_mask))
                .then(cmp),
            (PixelFormat::Dx10(a), PixelFormat::Dx10(b)) => u32::from(a.resource_dimension)
                .cmp(&u32::from(b.resource_dimension))
                .then(a.misc_flag.bits().cmp(&b.misc_flag.bits()))
                .then(u32::from(a.dxgi_format).cmp(&u32::from(b.dxgi_format)))
                .then(a.array_size.cmp(&b.array_size))
                .then(cmp)
                .then(a.misc_flags2.bits().cmp(&b.misc_flags2.bits())),
            _ => {
                let a = match &a.format {
                    PixelFormat::FourCC(_) => 0,
                    PixelFormat::Mask(_) => 1,
                    PixelFormat::Dx10(_) => 2,
                };
                let b = match &b.format {
                    PixelFormat::FourCC(_) => 0,
                    PixelFormat::Mask(_) => 1,
                    PixelFormat::Dx10(_) => 2,
                };
                a.cmp(&b).then(cmp)
            }
        }
    });

    headers
}

#[test]
fn raw_header_snapshot() {
    let headers = get_headers();

    fn collect_info(header: &Header) -> String {
        let mut output = String::new();

        // HEADER
        util::pretty_print_header(&mut output, header);
        output.push('\n');

        // RAW HEADER
        let raw = header.to_raw();
        util::pretty_print_raw_header(&mut output, &raw);

        output
    }

    // create expected info
    let mut output = String::new();
    for header in headers {
        let info = collect_info(&header);

        for line in info.lines() {
            output.push_str(format!("    {}", line).trim_end());
            output.push('\n');
        }

        output.push('\n');
        output.push('\n');
        output.push('\n');
    }

    util::compare_snapshot_text(
        &util::test_data_dir().join("raw_header_snapshot.txt"),
        &output,
    );
}
