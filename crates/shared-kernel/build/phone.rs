// crates/shared-kernel/build/phone.rs

use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

pub fn generate_country_codes(out_dir: &str) {
    let dest_path = Path::new(out_dir).join("codegen_country_codes.rs");
    let mut file = BufWriter::new(File::create(&dest_path).unwrap());

    let data_path = Path::new("data/country_codes.txt");
    let data_file = File::open(data_path).expect("Missing country_codes.txt file");
    let reader = BufReader::new(data_file);

    let mut set_2 = phf_codegen::Set::new();
    let mut set_3 = phf_codegen::Set::new();

    for line in reader.lines() {
        let raw_line = line.unwrap();
        let trimmed = raw_line.trim();

        let clean_code = match trimmed.split_once('#') {
            Some((before, _after)) => before.trim(),
            None => trimmed,
        };

        if clean_code.is_empty() {
            continue;
        }

        match clean_code.len() {
            2 => {
                set_2.entry(clean_code.to_string());
            }
            3 => {
                set_3.entry(clean_code.to_string());
            }
            _ => panic!(
                "Invalid ITU country code length in data file: {}",
                clean_code
            ),
        }
    }

    writeln!(
        file,
        "static CODES_3_DIGITS: phf::Set<&'static str> = {};\n",
        set_3.build()
    )
    .unwrap();
    writeln!(
        file,
        "static CODES_2_DIGITS: phf::Set<&'static str> = {};",
        set_2.build()
    )
    .unwrap();
}
