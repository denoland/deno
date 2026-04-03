// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::rc::Rc;

use deno_core::uv_compat::uv_buf_t;
use deno_core::uv_compat::uv_stream_t;
use deno_core::v8;

#[derive(Clone, Copy)]
pub(crate) struct ReadInterceptor {
  pub ptr: *mut c_void,
  pub callback:
    unsafe fn(*mut c_void, *mut uv_stream_t, isize, *const uv_buf_t),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ReadCallbackKey {
  index: u32,
  generation: u32,
}

pub(crate) struct ReadCallbackState {
  pub isolate: v8::UnsafeRawIsolatePtr,
  pub onread: Option<v8::Global<v8::Function>>,
  pub stream_base_state: Option<v8::Global<v8::Int32Array>>,
  pub handle: Option<v8::Global<v8::Object>>,
  pub bytes_read: Rc<Cell<u64>>,
  pub read_interceptor: Option<ReadInterceptor>,
}

#[derive(Clone)]
pub(crate) struct ReadCallbackSnapshot {
  pub isolate: v8::UnsafeRawIsolatePtr,
  pub onread: Option<v8::Global<v8::Function>>,
  pub stream_base_state: Option<v8::Global<v8::Int32Array>>,
  pub handle: Option<v8::Global<v8::Object>>,
  pub bytes_read: Rc<Cell<u64>>,
  pub read_interceptor: Option<ReadInterceptor>,
}

impl ReadCallbackState {
  fn snapshot(&self) -> ReadCallbackSnapshot {
    ReadCallbackSnapshot {
      isolate: self.isolate,
      onread: self.onread.clone(),
      stream_base_state: self.stream_base_state.clone(),
      handle: self.handle.clone(),
      bytes_read: self.bytes_read.clone(),
      read_interceptor: self.read_interceptor,
    }
  }
}

#[derive(Default)]
pub(crate) struct ReadCallbackRegistry {
  slots: Vec<ReadCallbackSlot>,
  free_head: Option<u32>,
}

struct ReadCallbackSlot {
  generation: u32,
  state: Option<ReadCallbackState>,
  next_free: Option<u32>,
}

impl ReadCallbackRegistry {
  pub fn insert(&mut self, state: ReadCallbackState) -> ReadCallbackKey {
    if let Some(index) = self.free_head {
      let slot = &mut self.slots[index as usize];
      self.free_head = slot.next_free.take();
      slot.state = Some(state);
      return ReadCallbackKey {
        index,
        generation: slot.generation,
      };
    }

    let index = self.slots.len() as u32;
    self.slots.push(ReadCallbackSlot {
      generation: 0,
      state: Some(state),
      next_free: None,
    });
    ReadCallbackKey {
      index,
      generation: 0,
    }
  }

  pub fn snapshot(&self, key: ReadCallbackKey) -> Option<ReadCallbackSnapshot> {
    let slot = self.slots.get(key.index as usize)?;
    if slot.generation != key.generation {
      return None;
    }
    slot.state.as_ref().map(ReadCallbackState::snapshot)
  }

  #[allow(dead_code, reason = "used by upcoming TLSWrap")]
  pub fn update_interceptor(
    &mut self,
    key: ReadCallbackKey,
    interceptor: Option<ReadInterceptor>,
  ) -> bool {
    let Some(slot) = self.slots.get_mut(key.index as usize) else {
      return false;
    };
    if slot.generation != key.generation {
      return false;
    }
    let Some(state) = slot.state.as_mut() else {
      return false;
    };
    state.read_interceptor = interceptor;
    true
  }

  pub fn remove(&mut self, key: ReadCallbackKey) -> Option<ReadCallbackState> {
    let slot = self.slots.get_mut(key.index as usize)?;
    if slot.generation != key.generation {
      return None;
    }
    let state = slot.state.take()?;
    slot.generation = slot.generation.wrapping_add(1);
    slot.next_free = self.free_head;
    self.free_head = Some(key.index);
    Some(state)
  }
}

pub(crate) struct WriteRequestCallbackState {
  pub isolate: v8::UnsafeRawIsolatePtr,
  pub req_wrap_obj: v8::Global<v8::Object>,
  pub stream_handle: v8::Global<v8::Object>,
  pub stream_base_state: v8::Global<v8::Int32Array>,
  pub bytes: usize,
}

pub(crate) struct ShutdownRequestCallbackState {
  pub isolate: v8::UnsafeRawIsolatePtr,
  pub req_wrap_obj: v8::Global<v8::Object>,
  pub stream_handle: v8::Global<v8::Object>,
}

pub(crate) enum RequestCallbackState {
  Write(WriteRequestCallbackState),
  Shutdown(ShutdownRequestCallbackState),
}

#[derive(Default)]
pub(crate) struct RequestCallbackRegistry {
  next_id: usize,
  states: HashMap<usize, RequestCallbackState>,
}

impl RequestCallbackRegistry {
  pub fn insert(&mut self, state: RequestCallbackState) -> *mut c_void {
    let id = self.next_id.checked_add(1).unwrap_or(1);
    self.next_id = id;
    self.states.insert(id, state);
    id as *mut c_void
  }

  pub fn take(&mut self, ptr: *mut c_void) -> Option<RequestCallbackState> {
    let id = ptr as usize;
    if id == 0 {
      return None;
    }
    self.states.remove(&id)
  }
}
