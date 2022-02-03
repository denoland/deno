use deno_core::napi::*;

#[no_mangle]
pub unsafe extern "C" fn napi_fatal_error(
  location: *const c_char,
  location_len: isize,
  message: *const c_char,
  message_len: isize,
) -> ! {
  let location = if location.is_null() {
    None
  } else {
    Some(if location_len < 0 {
      std::ffi::CStr::from_ptr(location).to_str().unwrap()
    } else {
      let slice = std::slice::from_raw_parts(
        location as *const u8,
        location_len as usize,
      );
      std::str::from_utf8(slice).unwrap()
    })
  };
  let message = if message_len < 0 {
    std::ffi::CStr::from_ptr(message).to_str().unwrap()
  } else {
    let slice =
      std::slice::from_raw_parts(message as *const u8, message_len as usize);
    std::str::from_utf8(slice).unwrap()
  };
  panic!(
    "Fatal exception triggered by napi_fatal_error!\nLocation: {:?}\n{}",
    location, message
  );
}
