// crates/shared-kernel/build.rs

#[path = "build/phone.rs"]
mod phone;

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();

    phone::generate_country_codes(&out_dir);

    println!("cargo:rerun-if-changed=build/phone.rs");
    println!("cargo:rerun-if-changed=data/country_codes.txt");
}
