#[no_mangle]
pub fn print_something() {
  println!("something");
}

#[no_mangle]
pub fn add_one_i8(arg: i8) -> i8 {
  arg + 1
}

#[no_mangle]
pub fn add_one_i16(arg: i16) -> i16 {
  arg + 1
}

#[no_mangle]
pub fn add_one_i32(arg: i32) -> i32 {
  arg + 1
}

#[no_mangle]
pub fn add_one_i64(arg: i64) -> i64 {
  arg + 1
}

#[no_mangle]
pub fn add_one_u8(arg: u8) -> u8 {
  arg + 1
}

#[no_mangle]
pub fn add_one_u16(arg: u16) -> u16 {
  arg + 1
}

#[no_mangle]
pub fn add_one_u32(arg: u32) -> u32 {
  arg + 1
}

#[no_mangle]
pub fn add_one_u64(arg: u64) -> u64 {
  arg + 1
}

#[no_mangle]
pub fn add_one_f32(arg: f32) -> f32 {
  arg + 1.0
}

#[no_mangle]
pub fn add_one_f64(arg: f64) -> f64 {
  arg + 1.0
}
