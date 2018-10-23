use std::env;
use std::process::Command;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    // ./third_party/depot_tools/gn gen $OUT_DIR
    let gn_status = Command::new("./third_party/depot_tools/gn")
        .args(&["gen", &out_dir])
        .status()
        .expect("Failed to execute GN.");

    if !gn_status.success() {
        println!("Failed to generate GN build configuration.");
        std::process::exit(-1);
    }


    // DENO_BUILD_PATH=$OUT_DIR python ./tools/build.py
    let py_status = Command::new("python")
        .env("DENO_BUILD_PATH", out_dir)
        .arg("./tools/build.py")
        .arg("libdeno")
        .status()
        .expect("Failed to build Deno.\nMake sure you have `python` in your PATH.");
    if !py_status.success() {
        std::process::exit(-1);
    }
}