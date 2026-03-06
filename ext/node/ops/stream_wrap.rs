use deno_core::{
  CppgcBase, CppgcInherits, GarbageCollected, uv_compat::uv_stream_t,
};

use crate::ops::handle_wrap::HandleWrap;

#[derive(CppgcBase, CppgcInherits)]
#[repr(C)]
#[cppgc_inherits_from(HandleWrap)]
pub struct LibUvStreamWrap {
  base: HandleWrap,
  fd: i32,
  stream: *const uv_stream_t,
}

impl LibUvStreamWrap {
  pub fn new(base: HandleWrap, fd: i32, stream: *const uv_stream_t) -> Self {
    Self { base, fd, stream }
  }
}

unsafe impl GarbageCollected for LibUvStreamWrap {
  fn get_name(&self) -> &'static std::ffi::CStr {
    c"LibUvStreamWrap"
  }

  fn trace(&self, visitor: &mut deno_core::v8::cppgc::Visitor) {
    self.base.trace(visitor);
  }
}
