#[no_mangle]
pub extern "C" fn print_something() {
  println!("something");
}

#[no_mangle]
pub extern "C" fn add(a: u32, b: u32) -> u32 {
  a + b
}
