use std::env;
use std::path::PathBuf;
use std::process::exit;
use std::process::Command;

fn build_tcc(config_arg: Option<&[&str]>, make_arg: Option<&[&str]>) {
  let tcc_src = env::current_dir().unwrap().join("src/tcc-0.9.27");
  let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

  let mut configure = Command::new(tcc_src.join("configure"));
  configure.current_dir(&out_dir);
  if let Some(args) = config_arg {
    configure.args(args);
  }
  let status = configure.status().unwrap();
  if !status.success() {
    eprintln!("Fail to configure: {:?}", status);
    exit(1);
  }

  let mut make = Command::new("make");
  make.current_dir(&out_dir).arg(format!(
    "-j{}",
    env::var("NUM_JOBS").unwrap_or_else(|_| String::from("1"))
  ));
  if let Some(args) = make_arg {
    make.args(args);
  }
  let status = make.status().unwrap();

  if !status.success() {
    eprintln!("Fail to make: {:?}", status);
    exit(1);
  }

  println!("cargo:rustc-link-search=native={}", out_dir.display());
  println!("cargo:rerun-if-changed={}", tcc_src.display());
}

fn main() {
  // TODO(@littledivy): Windows
  println!("cargo:rustc-link-search=native=/usr/local/lib");
  println!("cargo:rustc-link-lib=static=tcc");
}
