#[no_mangle]
pub extern "C" fn print_something() {
  println!("something");
}

#[no_mangle]
pub extern "C" fn add_two(arg: u32) -> u32 {
  arg + 2
}
