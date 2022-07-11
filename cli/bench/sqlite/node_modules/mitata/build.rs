fn main() {
  unsafe {
    println!("cargo:rustc-env=TARGET={}", std::env::var("TARGET").unwrap_unchecked());
  }
}