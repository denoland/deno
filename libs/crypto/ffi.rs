// Copyright 2018-2025 the Deno authors. MIT license.

pub struct PKey(pub *mut aws_lc_sys::EVP_PKEY);

impl PKey {
  pub fn from_ptr(ptr: *mut aws_lc_sys::EVP_PKEY) -> Option<Self> {
    if ptr.is_null() {
      None
    } else {
      Some(Self(ptr))
    }
  }

  pub fn as_ptr(&self) -> *mut aws_lc_sys::EVP_PKEY {
    self.0
  }
}

impl Drop for PKey {
  fn drop(&mut self) {
    unsafe {
      if self.0.is_null() {
        return;
      }
      aws_lc_sys::EVP_PKEY_free(self.0);
    }
  }
}

impl std::ops::Deref for PKey {
  type Target = *mut aws_lc_sys::EVP_PKEY;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

pub struct Bio(pub *mut aws_lc_sys::BIO);

impl Drop for Bio {
  fn drop(&mut self) {
    unsafe {
      if self.0.is_null() {
        return;
      }
      aws_lc_sys::BIO_free(self.0);
    }
  }
}

impl Bio {
  pub fn new_memory() -> Result<Self, &'static str> {
    unsafe {
      let bio = aws_lc_sys::BIO_new(aws_lc_sys::BIO_s_mem());
      if bio.is_null() {
        return Err("Failed to create memory BIO");
      }
      Ok(Bio(bio))
    }
  }

  pub fn get_contents(&self) -> Result<Vec<u8>, &'static str> {
    unsafe {
      let mut len = 0;
      let mut content_ptr = std::ptr::null();
      aws_lc_sys::BIO_mem_contents(self.0, &mut content_ptr, &mut len);

      if content_ptr.is_null() || len == 0 {
        return Err("No content in BIO");
      }

      let data =
        std::slice::from_raw_parts(content_ptr as *const u8, len as usize);
      Ok(data.to_vec())
    }
  }

  pub fn as_ptr(&self) -> *mut aws_lc_sys::BIO {
    self.0
  }
}

impl std::ops::Deref for Bio {
  type Target = *mut aws_lc_sys::BIO;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}
