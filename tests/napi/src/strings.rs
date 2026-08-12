// Copyright 2018-2026 the Deno authors. MIT license.

use std::ffi::c_char;
use std::ffi::c_void;
use std::sync::LazyLock;
use std::sync::Mutex;

use napi_sys::ValueType::napi_string;
use napi_sys::*;

use crate::assert_napi_ok;
use crate::napi_get_callback_info;
use crate::napi_new_property;

extern "C" fn test_utf8(env: napi_env, info: napi_callback_info) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut ty = -1;
  assert_napi_ok!(napi_typeof(env, args[0], &mut ty));
  assert_eq!(ty, napi_string);

  args[0]
}

extern "C" fn test_utf16(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut ty = -1;
  assert_napi_ok!(napi_typeof(env, args[0], &mut ty));
  assert_eq!(ty, napi_string);

  args[0]
}

extern "C" fn test_utf8_roundtrip(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut len: usize = 0;
  assert_napi_ok!(napi_get_value_string_utf8(
    env,
    args[0],
    std::ptr::null_mut(),
    0,
    &mut len
  ));

  let mut buf: Vec<u8> = vec![0; 1024];
  let mut copied: usize = 0;
  assert_napi_ok!(napi_get_value_string_utf8(
    env,
    args[0],
    buf.as_mut_ptr() as *mut std::ffi::c_char,
    buf.len(),
    &mut copied
  ));

  assert_eq!(copied, len);

  let mut result: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    buf.as_ptr() as *const std::ffi::c_char,
    copied,
    &mut result
  ));

  result
}

extern "C" fn test_property_key_latin1(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (_args, argc, _) = napi_get_callback_info!(env, info, 0);
  assert_eq!(argc, 0);

  // Create a property key from latin1 string "hello"
  let latin1_str = b"hello\0";
  let mut key: napi_value = std::ptr::null_mut();
  assert_napi_ok!(node_api_create_property_key_latin1(
    env,
    latin1_str.as_ptr() as *const c_char,
    5,
    &mut key,
  ));

  // Create an object and set a property using the key
  let mut obj: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_create_object(env, &mut obj));

  let mut value: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_create_int32(env, 42, &mut value));
  assert_napi_ok!(napi_set_property(env, obj, key, value));

  // Verify the property can be retrieved using a regular string key
  let mut key2: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    b"hello\0".as_ptr() as *const c_char,
    5,
    &mut key2,
  ));

  let mut result: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_get_property(env, obj, key2, &mut result));

  result
}

extern "C" fn test_property_key_utf8(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (_args, argc, _) = napi_get_callback_info!(env, info, 0);
  assert_eq!(argc, 0);

  // Create a property key from utf8 string "hello"
  let utf8_str = b"hello\0";
  let mut key: napi_value = std::ptr::null_mut();
  assert_napi_ok!(node_api_create_property_key_utf8(
    env,
    utf8_str.as_ptr() as *const c_char,
    5,
    &mut key,
  ));

  // Create an object and set a property using the key
  let mut obj: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_create_object(env, &mut obj));

  let mut value: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_create_int32(env, 42, &mut value));
  assert_napi_ok!(napi_set_property(env, obj, key, value));

  // Verify the property can be retrieved using a regular string key
  let mut key2: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    b"hello\0".as_ptr() as *const c_char,
    5,
    &mut key2,
  ));

  let mut result: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_get_property(env, obj, key2, &mut result));

  result
}

extern "C" fn test_property_key_utf16(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (_args, argc, _) = napi_get_callback_info!(env, info, 0);
  assert_eq!(argc, 0);

  // Create a property key from utf16 string "hello"
  let utf16_str: [u16; 6] = [
    'h' as u16, 'e' as u16, 'l' as u16, 'l' as u16, 'o' as u16, 0,
  ];
  let mut key: napi_value = std::ptr::null_mut();
  assert_napi_ok!(node_api_create_property_key_utf16(
    env,
    utf16_str.as_ptr(),
    5,
    &mut key,
  ));

  // Create an object and set a property using the key
  let mut obj: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_create_object(env, &mut obj));

  let mut value: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_create_int32(env, 42, &mut value));
  assert_napi_ok!(napi_set_property(env, obj, key, value));

  // Verify the property can be retrieved using a regular string key
  let mut key2: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf8(
    env,
    b"hello\0".as_ptr() as *const c_char,
    5,
    &mut key2,
  ));

  let mut result: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_get_property(env, obj, key2, &mut result));

  result
}

extern "C" fn test_latin1_roundtrip(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  // Get length
  let mut len: usize = 0;
  assert_napi_ok!(napi_get_value_string_latin1(
    env,
    args[0],
    std::ptr::null_mut(),
    0,
    &mut len
  ));

  // Get string content
  let mut buf: Vec<u8> = vec![0; len + 1];
  let mut copied: usize = 0;
  assert_napi_ok!(napi_get_value_string_latin1(
    env,
    args[0],
    buf.as_mut_ptr() as *mut c_char,
    buf.len(),
    &mut copied
  ));
  assert_eq!(copied, len);

  // Create string from latin1 bytes
  let mut result: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_create_string_latin1(
    env,
    buf.as_ptr() as *const c_char,
    copied,
    &mut result
  ));

  result
}

extern "C" fn test_utf16_roundtrip(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = napi_get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  // Get length
  let mut len: usize = 0;
  assert_napi_ok!(napi_get_value_string_utf16(
    env,
    args[0],
    std::ptr::null_mut(),
    0,
    &mut len
  ));

  // Get string content
  let mut buf: Vec<u16> = vec![0; len + 1];
  let mut copied: usize = 0;
  assert_napi_ok!(napi_get_value_string_utf16(
    env,
    args[0],
    buf.as_mut_ptr(),
    buf.len(),
    &mut copied
  ));
  assert_eq!(copied, len);

  // Create string from utf16
  let mut result: napi_value = std::ptr::null_mut();
  assert_napi_ok!(napi_create_string_utf16(
    env,
    buf.as_ptr(),
    copied,
    &mut result
  ));

  result
}

// node_api_create_external_string_latin1 is declared in napi_sys

/// Release a latin1 buffer allocated via Vec<u8>.
unsafe extern "C" fn finalize_latin1(
  _env: napi_env,
  data: *mut c_void,
  hint: *mut c_void,
) {
  let len = hint as usize;
  unsafe { drop(Vec::from_raw_parts(data as *mut u8, len, len)) };
}

/// Test that node_api_create_external_string_latin1 creates a string
/// and reports whether the data was copied.
extern "C" fn test_external_latin1(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  // Allocate a buffer that the external string will reference
  let data = b"hello latin1".to_vec();
  let ptr = data.as_ptr();
  let len = data.len();
  std::mem::forget(data);

  let mut result: napi_value = std::ptr::null_mut();
  let mut copied = true; // Initialize to see if it changes
  let status = unsafe {
    node_api_create_external_string_latin1(
      env,
      ptr as *const c_char,
      len,
      Some(finalize_latin1),
      len as *mut c_void, // pass length as hint for deallocation
      &mut result,
      &mut copied,
    )
  };
  assert_eq!(status, 0); // napi_ok

  // Read back the string to verify content
  let mut buf: Vec<u8> = vec![0; 64];
  let mut out_len: usize = 0;
  assert_napi_ok!(napi_get_value_string_latin1(
    env,
    result,
    buf.as_mut_ptr() as *mut c_char,
    buf.len(),
    &mut out_len
  ));
  assert_eq!(&buf[..out_len], b"hello latin1");

  if copied {
    // V8 copied the data, so we still own the buffer and must free it.
    unsafe {
      drop(Vec::from_raw_parts(ptr as *mut u8, len, len));
    }
  }
  // If !copied (zero-copy), V8 owns the buffer and will call
  // finalize_latin1 when the string is garbage collected.

  let mut ret: napi_value = std::ptr::null_mut();
  // Return whether the string was copied (false = zero-copy, true = copied)
  assert_napi_ok!(napi_get_boolean(env, !copied, &mut ret));
  ret
}

/// Release a UTF-16 buffer allocated via Vec<u16>.
unsafe extern "C" fn finalize_utf16(
  _env: napi_env,
  data: *mut c_void,
  hint: *mut c_void,
) {
  let len = hint as usize;
  unsafe { drop(Vec::from_raw_parts(data as *mut u16, len, len)) };
}

/// Test that node_api_create_external_string_utf16 creates a string
/// and reports whether the data was copied.
extern "C" fn test_external_utf16(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  // Allocate a UTF-16 buffer: "hello utf16"
  let data: Vec<u16> = "hello utf16".encode_utf16().collect();
  let ptr = data.as_ptr();
  let len = data.len();
  std::mem::forget(data);

  let mut result: napi_value = std::ptr::null_mut();
  let mut copied = true; // Initialize to see if it changes
  let status = unsafe {
    node_api_create_external_string_utf16(
      env,
      ptr,
      len,
      Some(finalize_utf16),
      len as *mut c_void, // pass length as hint for deallocation
      &mut result,
      &mut copied,
    )
  };
  assert_eq!(status, 0); // napi_ok

  // Read back the string to verify content
  let mut buf: Vec<u16> = vec![0; 64];
  let mut out_len: usize = 0;
  assert_napi_ok!(napi_get_value_string_utf16(
    env,
    result,
    buf.as_mut_ptr(),
    buf.len(),
    &mut out_len
  ));
  let expected: Vec<u16> = "hello utf16".encode_utf16().collect();
  assert_eq!(&buf[..out_len], &expected[..]);

  if copied {
    // V8 copied the data, so we still own the buffer and must free it.
    unsafe {
      drop(Vec::from_raw_parts(ptr as *mut u16, len, len));
    }
  }
  // If !copied (zero-copy), V8 owns the buffer and will call
  // finalize_utf16 when the string is garbage collected.

  let mut ret: napi_value = std::ptr::null_mut();
  // Return whether the string was copied (false = zero-copy, true = copied)
  assert_napi_ok!(napi_get_boolean(env, !copied, &mut ret));
  ret
}

const SHARED_EXTERNAL_STRING_LEN: usize = 4096;
static SHARED_EXTERNAL_LATIN1: [u8; SHARED_EXTERNAL_STRING_LEN] =
  [b'x'; SHARED_EXTERNAL_STRING_LEN];
static SHARED_EXTERNAL_UTF16: [u16; SHARED_EXTERNAL_STRING_LEN] =
  [b'y' as u16; SHARED_EXTERNAL_STRING_LEN];

#[derive(Default)]
struct ExternalStringFinalizerProbe {
  creator_thread: Option<std::thread::ThreadId>,
  called: u32,
  hints: u32,
  wrong_thread: bool,
  null_env: bool,
}

static EXTERNAL_STRING_FINALIZER_PROBE: LazyLock<
  Mutex<ExternalStringFinalizerProbe>,
> = LazyLock::new(|| Mutex::new(ExternalStringFinalizerProbe::default()));

unsafe extern "C" fn record_external_string_finalizer(
  env: napi_env,
  _data: *mut c_void,
  hint: *mut c_void,
) {
  let mut probe = EXTERNAL_STRING_FINALIZER_PROBE.lock().unwrap();
  probe.called += 1;
  probe.hints |= 1 << hint as usize;
  probe.wrong_thread |=
    probe.creator_thread.as_ref() != Some(&std::thread::current().id());
  probe.null_env |= env.is_null();
}

extern "C" fn test_external_string_finalizer_reset(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  *EXTERNAL_STRING_FINALIZER_PROBE.lock().unwrap() =
    ExternalStringFinalizerProbe {
      creator_thread: Some(std::thread::current().id()),
      ..Default::default()
    };

  let mut result = std::ptr::null_mut();
  assert_napi_ok!(napi_get_undefined(env, &mut result));
  result
}

/// Create two one-byte and two two-byte strings from identical backing-store
/// addresses. The second string of each encoding must use the copy path so
/// every resource keeps the correct finalizer identity.
extern "C" fn test_external_string_finalizer_collisions(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut strings = [std::ptr::null_mut(); 4];
  let mut copied = [true; 4];

  for index in 0..2 {
    let status = unsafe {
      node_api_create_external_string_latin1(
        env,
        SHARED_EXTERNAL_LATIN1.as_ptr() as *const c_char,
        SHARED_EXTERNAL_LATIN1.len(),
        Some(record_external_string_finalizer),
        index as *mut c_void,
        &mut strings[index],
        &mut copied[index],
      )
    };
    assert_eq!(status, 0);
  }

  for index in 2..4 {
    let status = unsafe {
      node_api_create_external_string_utf16(
        env,
        SHARED_EXTERNAL_UTF16.as_ptr(),
        SHARED_EXTERNAL_UTF16.len(),
        Some(record_external_string_finalizer),
        index as *mut c_void,
        &mut strings[index],
        &mut copied[index],
      )
    };
    assert_eq!(status, 0);
  }

  let mut result = std::ptr::null_mut();
  assert_napi_ok!(napi_create_array_with_length(env, 8, &mut result));
  for (index, string) in strings.into_iter().enumerate() {
    assert_napi_ok!(napi_set_element(env, result, index as u32, string));
  }
  for (index, copied) in copied.into_iter().enumerate() {
    let mut value = std::ptr::null_mut();
    assert_napi_ok!(napi_get_boolean(env, !copied, &mut value));
    assert_napi_ok!(napi_set_element(env, result, (index + 4) as u32, value));
  }
  result
}

/// V8 disposes zero-length external resources synchronously. Keep these on
/// the copy path so their callbacks run exactly once before the API returns.
extern "C" fn test_empty_external_string_finalizers(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let mut strings = [std::ptr::null_mut(); 2];
  let mut copied = [false; 2];

  let latin1_status = unsafe {
    node_api_create_external_string_latin1(
      env,
      SHARED_EXTERNAL_LATIN1.as_ptr() as *const c_char,
      0,
      Some(record_external_string_finalizer),
      4 as *mut c_void,
      &mut strings[0],
      &mut copied[0],
    )
  };
  assert_eq!(latin1_status, 0);

  let utf16_status = unsafe {
    node_api_create_external_string_utf16(
      env,
      SHARED_EXTERNAL_UTF16.as_ptr(),
      0,
      Some(record_external_string_finalizer),
      5 as *mut c_void,
      &mut strings[1],
      &mut copied[1],
    )
  };
  assert_eq!(utf16_status, 0);

  let mut result = std::ptr::null_mut();
  assert_napi_ok!(napi_create_array_with_length(env, 4, &mut result));
  for (index, string) in strings.into_iter().enumerate() {
    assert_napi_ok!(napi_set_element(env, result, index as u32, string));
  }
  for (index, copied) in copied.into_iter().enumerate() {
    let mut value = std::ptr::null_mut();
    assert_napi_ok!(napi_get_boolean(env, !copied, &mut value));
    assert_napi_ok!(napi_set_element(env, result, (index + 2) as u32, value));
  }
  result
}

extern "C" fn test_external_string_finalizer_status(
  env: napi_env,
  _info: napi_callback_info,
) -> napi_value {
  let probe = EXTERNAL_STRING_FINALIZER_PROBE.lock().unwrap();
  let mut result = std::ptr::null_mut();
  assert_napi_ok!(napi_create_array_with_length(env, 4, &mut result));

  let mut called = std::ptr::null_mut();
  assert_napi_ok!(napi_create_uint32(env, probe.called, &mut called));
  assert_napi_ok!(napi_set_element(env, result, 0, called));

  let mut wrong_thread = std::ptr::null_mut();
  assert_napi_ok!(napi_get_boolean(env, probe.wrong_thread, &mut wrong_thread));
  assert_napi_ok!(napi_set_element(env, result, 1, wrong_thread));

  let mut null_env = std::ptr::null_mut();
  assert_napi_ok!(napi_get_boolean(env, probe.null_env, &mut null_env));
  assert_napi_ok!(napi_set_element(env, result, 2, null_env));

  let mut hints = std::ptr::null_mut();
  assert_napi_ok!(napi_create_uint32(env, probe.hints, &mut hints));
  assert_napi_ok!(napi_set_element(env, result, 3, hints));
  result
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[
    napi_new_property!(env, "test_utf8", test_utf8),
    napi_new_property!(env, "test_utf16", test_utf16),
    napi_new_property!(env, "test_utf8_roundtrip", test_utf8_roundtrip),
    napi_new_property!(
      env,
      "test_property_key_latin1",
      test_property_key_latin1
    ),
    napi_new_property!(env, "test_property_key_utf8", test_property_key_utf8),
    napi_new_property!(env, "test_property_key_utf16", test_property_key_utf16),
    napi_new_property!(env, "test_latin1_roundtrip", test_latin1_roundtrip),
    napi_new_property!(env, "test_utf16_roundtrip", test_utf16_roundtrip),
    napi_new_property!(env, "test_external_latin1", test_external_latin1),
    napi_new_property!(env, "test_external_utf16", test_external_utf16),
    napi_new_property!(
      env,
      "test_external_string_finalizer_reset",
      test_external_string_finalizer_reset
    ),
    napi_new_property!(
      env,
      "test_external_string_finalizer_collisions",
      test_external_string_finalizer_collisions
    ),
    napi_new_property!(
      env,
      "test_empty_external_string_finalizers",
      test_empty_external_string_finalizers
    ),
    napi_new_property!(
      env,
      "test_external_string_finalizer_status",
      test_external_string_finalizer_status
    ),
  ];

  assert_napi_ok!(napi_define_properties(
    env,
    exports,
    properties.len(),
    properties.as_ptr()
  ));
}
