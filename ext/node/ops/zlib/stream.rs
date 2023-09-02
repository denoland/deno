// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use super::mode::Flush;
use super::mode::Mode;
use libz_sys as sys;
use std::ffi::c_int;
use std::ops::Deref;
use std::ops::DerefMut;

pub struct StreamWrapper {
  pub strm: sys::z_stream,
}

impl Default for StreamWrapper {
  fn default() -> Self {
    Self {
      strm: sys::z_stream {
        next_in: std::ptr::null_mut(),
        avail_in: 0,
        total_in: 0,
        next_out: std::ptr::null_mut(),
        avail_out: 0,
        total_out: 0,
        msg: std::ptr::null_mut(),
        state: std::ptr::null_mut(),
        zalloc: super::alloc::zalloc,
        zfree: super::alloc::zfree,
        opaque: 0 as sys::voidpf,
        data_type: 0,
        adler: 0,
        reserved: 0,
      },
    }
  }
}

impl Deref for StreamWrapper {
  type Target = sys::z_stream;

  fn deref(&self) -> &Self::Target {
    &self.strm
  }
}

impl DerefMut for StreamWrapper {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.strm
  }
}

impl StreamWrapper {
  pub fn reset(&mut self, mode: Mode) -> c_int {
    // SAFETY: `self.strm` is an initialized `z_stream`.
    unsafe {
      match mode {
        Mode::Deflate | Mode::Gzip | Mode::DeflateRaw => {
          sys::deflateReset(&mut self.strm)
        }
        Mode::Inflate | Mode::Gunzip | Mode::InflateRaw | Mode::Unzip => {
          sys::inflateReset(&mut self.strm)
        }
        Mode::None => unreachable!(),
      }
    }
  }

  pub fn end(&mut self, mode: Mode) {
    // SAFETY: `self.strm` is an initialized `z_stream`.
    unsafe {
      match mode {
        Mode::Deflate | Mode::Gzip | Mode::DeflateRaw => {
          sys::deflateEnd(&mut self.strm);
        }
        Mode::Inflate | Mode::Gunzip | Mode::InflateRaw | Mode::Unzip => {
          sys::inflateEnd(&mut self.strm);
        }
        Mode::None => {}
      }
    }
  }

  pub fn deflate_init(
    &mut self,
    level: c_int,
    window_bits: c_int,
    mem_level: c_int,
    strategy: c_int,
  ) -> c_int {
    // SAFETY: `self.strm` is an initialized `z_stream`.
    unsafe {
      sys::deflateInit2_(
        &mut self.strm,
        level,
        sys::Z_DEFLATED,
        window_bits,
        mem_level,
        strategy,
        sys::zlibVersion(),
        std::mem::size_of::<sys::z_stream>() as i32,
      )
    }
  }

  pub fn inflate_init(&mut self, window_bits: c_int) -> c_int {
    // SAFETY: `self.strm` is an initialized `z_stream`.
    unsafe {
      sys::inflateInit2_(
        &mut self.strm,
        window_bits,
        sys::zlibVersion(),
        std::mem::size_of::<sys::z_stream>() as i32,
      )
    }
  }

  pub fn deflate(&mut self, flush: Flush) -> c_int {
    // SAFETY: `self.strm` is an initialized `z_stream`.
    unsafe { sys::deflate(&mut self.strm, flush as _) }
  }

  pub fn inflate(&mut self, flush: Flush) -> c_int {
    // SAFETY: `self.strm` is an initialized `z_stream`.
    unsafe { sys::inflate(&mut self.strm, flush as _) }
  }

  pub fn inflate_set_dictionary(&mut self, dictionary: &[u8]) -> c_int {
    // SAFETY: `self.strm` is an initialized `z_stream`.
    unsafe {
      sys::inflateSetDictionary(
        &mut self.strm,
        dictionary.as_ptr() as *const _,
        dictionary.len() as _,
      )
    }
  }
}
