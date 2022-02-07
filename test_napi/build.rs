fn main() {
    // macOS
    println!("cargo:rustc-cdylib-link-arg=-Wl");
    println!("cargo:rustc-cdylib-link-arg=-undefined");
    println!("cargo:rustc-cdylib-link-arg=dynamic_lookup");
}