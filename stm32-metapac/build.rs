use std::env;
use std::path::PathBuf;
use stm32_metapac_gen::*;

fn parse_chip_core(chip_and_core: &str) -> (String, Option<String>) {
    let mut s = chip_and_core.split('-');
    let chip_name: String = s.next().unwrap().to_string();
    if let Some(c) = s.next() {
        if c.starts_with("CM") {
            return (chip_name, Some(c.to_ascii_lowercase()));
        }
    }

    (chip_and_core.to_string(), None)
}

fn main() {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let data_dir = PathBuf::from("../stm32-data/data");

    println!("cwd: {:?}", env::current_dir());

    let chip_core_name = env::vars_os()
        .map(|(a, _)| a.to_string_lossy().to_string())
        .find(|x| x.starts_with("CARGO_FEATURE_STM32"))
        .expect("No stm32xx Cargo feature enabled")
        .strip_prefix("CARGO_FEATURE_")
        .unwrap()
        .to_ascii_uppercase()
        .replace('_', "-");

    let (chip_name, _) = parse_chip_core(&chip_core_name);

    gen(Options {
        out_dir: out_dir.clone(),
        data_dir: data_dir.clone(),
        chips: vec![chip_name],
    });

    println!(
        "cargo:rustc-link-search={}/src/chips/{}",
        out_dir.display(),
        chip_core_name.to_ascii_lowercase()
    );

    #[cfg(feature = "memory-x")]
    println!(
        "cargo:rustc-link-search={}/src/chips/{}/memory_x/",
        out_dir.display(),
        chip_core_name.to_ascii_lowercase()
    );

    println!("cargo:rerun-if-changed=build.rs");
}
