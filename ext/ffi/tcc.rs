// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::{
  ffi::CStr,
  marker::PhantomData,
  os::raw::{c_char, c_int, c_void},
  ptr::null_mut,
};

#[repr(C)]
#[derive(Debug)]
pub struct TCCState {
  _unused: [u8; 0],
}
pub const TCC_OUTPUT_MEMORY: i32 = 1;

extern "C" {
  pub fn tcc_new() -> *mut TCCState;
  pub fn tcc_delete(s: *mut TCCState);
  pub fn tcc_set_options(s: *mut TCCState, str: *const c_char);
  pub fn tcc_compile_string(s: *mut TCCState, buf: *const c_char) -> c_int;
  pub fn tcc_add_symbol(
    s: *mut TCCState,
    name: *const c_char,
    val: *const c_void,
  ) -> c_int;
  pub fn tcc_set_output_type(s: *mut TCCState, output_type: c_int) -> c_int;
  pub fn tcc_relocate(s1: *mut TCCState, ptr: *mut c_void) -> c_int;
  pub fn tcc_get_symbol(s: *mut TCCState, name: *const c_char) -> *mut c_void;
}

/// Compilation context.
pub struct Compiler {
  inner: *mut TCCState,
  _phantom: PhantomData<TCCState>,
  pub bin: Option<Vec<u8>>,
}

impl Compiler {
  pub fn new() -> Result<Self, ()> {
    // SAFETY: There is one context per thread.
    let inner = unsafe { tcc_new() };
    if inner.is_null() {
      Err(())
    } else {
      let ret =
        // SAFETY: set output to memory.
        unsafe { tcc_set_output_type(inner, TCC_OUTPUT_MEMORY as c_int) };
      assert_eq!(ret, 0);
      Ok(Self {
        inner,
        _phantom: PhantomData,
        bin: None,
      })
    }
  }

  pub fn set_options(&mut self, option: &CStr) -> &mut Self {
    // SAFETY: option is a null-terminated C string.
    unsafe {
      tcc_set_options(self.inner, option.as_ptr());
    }
    self
  }

  pub fn compile_string(&mut self, p: &CStr) -> Result<(), ()> {
    // SAFETY: p is a null-terminated C string.
    let ret = unsafe { tcc_compile_string(self.inner, p.as_ptr()) };
    if ret == 0 {
      Ok(())
    } else {
      Err(())
    }
  }

  /// # Safety
  /// Symbol need satisfy ABI requirement.
  pub unsafe fn add_symbol(&mut self, sym: &CStr, val: *const c_void) {
    // SAFETY: sym is a null-terminated C string.
    let ret = tcc_add_symbol(self.inner, sym.as_ptr(), val);
    assert_eq!(ret, 0);
  }

  pub fn relocate_and_get_symbol(
    &mut self,
    sym: &CStr,
  ) -> Result<*mut c_void, ()> {
    // SAFETY: pass null ptr to get required length
    let len = unsafe { tcc_relocate(self.inner, null_mut()) };
    if len == -1 {
      return Err(());
    };
    let mut bin = Vec::with_capacity(len as usize);
    let ret =
      // SAFETY: bin is allocated up to len.
      unsafe { tcc_relocate(self.inner, bin.as_mut_ptr() as *mut c_void) };
    if ret != 0 {
      return Err(());
    }
    // SAFETY: if ret == 0, bin is initialized.
    unsafe {
      bin.set_len(len as usize);
    }
    self.bin = Some(bin);
    // SAFETY: sym is a null-terminated C string.
    let addr = unsafe { tcc_get_symbol(self.inner, sym.as_ptr()) };
    Ok(addr)
  }
}

impl Drop for Compiler {
  fn drop(&mut self) {
    // SAFETY: delete state from tcc_new()
    unsafe { tcc_delete(self.inner) };
  }
}
