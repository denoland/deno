// Copyright 2018-2026 the Deno authors. MIT license.

use std::cell::Cell;
use std::mem::MaybeUninit;
use std::ptr::addr_of_mut;
use std::sync::OnceLock;
use std::time::Instant;

use deno_core::parking_lot::Mutex;
use deno_core::uv_compat;

use crate::util::SendPtr;
use crate::*;

fn assert_ok(res: c_int) -> c_int {
  if res != 0 {
    log::error!("bad result in uv polyfill: {res}");
    // don't panic because that might unwind into
    // c/c++
    std::process::abort();
  }
  res
}

use std::ffi::c_int;

use js_native_api::napi_create_string_utf8;
use node_api::napi_create_async_work;
use node_api::napi_delete_async_work;

const UV_MUTEX_SIZE: usize = {
  #[cfg(unix)]
  {
    std::mem::size_of::<libc::pthread_mutex_t>()
  }
  #[cfg(windows)]
  {
    std::mem::size_of::<windows_sys::Win32::System::Threading::CRITICAL_SECTION>(
    )
  }
};

#[repr(C)]
struct uv_mutex_t {
  mutex: Mutex<()>,
  _padding: [MaybeUninit<usize>; const {
    (UV_MUTEX_SIZE - size_of::<Mutex<()>>()) / size_of::<usize>()
  }],
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_mutex_init(lock: *mut uv_mutex_t) -> c_int {
  unsafe {
    addr_of_mut!((*lock).mutex).write(Mutex::new(()));
    0
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_mutex_lock(lock: *mut uv_mutex_t) {
  unsafe {
    let guard = (*lock).mutex.lock();
    // forget the guard so it doesn't unlock when it goes out of scope.
    // we're going to unlock it manually
    std::mem::forget(guard);
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_mutex_unlock(lock: *mut uv_mutex_t) {
  unsafe {
    (*lock).mutex.force_unlock();
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_mutex_destroy(_lock: *mut uv_mutex_t) {
  // no cleanup required
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
#[allow(dead_code, reason = "variants represent libuv enum values")]
enum uv_handle_type {
  UV_UNKNOWN_HANDLE = 0,
  UV_ASYNC,
  UV_CHECK,
  UV_FS_EVENT,
  UV_FS_POLL,
  UV_HANDLE,
  UV_IDLE,
  UV_NAMED_PIPE,
  UV_POLL,
  UV_PREPARE,
  UV_PROCESS,
  UV_STREAM,
  UV_TCP,
  UV_TIMER,
  UV_TTY,
  UV_UDP,
  UV_SIGNAL,
  UV_FILE,
  UV_HANDLE_TYPE_MAX,
}

const UV_HANDLE_SIZE: usize = 96;

#[repr(C)]
struct uv_handle_t {
  // public members
  pub data: *mut c_void,
  pub r#loop: *mut uv_loop_t,
  pub r#type: uv_handle_type,

  _padding: [MaybeUninit<usize>; const {
    (UV_HANDLE_SIZE
      - size_of::<*mut c_void>()
      - size_of::<*mut uv_loop_t>()
      - size_of::<uv_handle_type>())
      / size_of::<usize>()
  }],
}

#[cfg(unix)]
const UV_ASYNC_SIZE: usize = 128;

#[cfg(windows)]
const UV_ASYNC_SIZE: usize = 224;

#[repr(C)]
struct uv_async_t {
  // public members
  pub data: *mut c_void,
  pub r#loop: *mut uv_loop_t,
  pub r#type: uv_handle_type,
  // private
  async_cb: uv_async_cb,
  work: napi_async_work,
  _padding: [MaybeUninit<usize>; const {
    (UV_ASYNC_SIZE
      - size_of::<*mut c_void>()
      - size_of::<*mut uv_loop_t>()
      - size_of::<uv_handle_type>()
      - size_of::<uv_async_cb>()
      - size_of::<napi_async_work>())
      / size_of::<usize>()
  }],
}

type uv_loop_t = Env;
type uv_async_cb = extern "C" fn(handle: *mut uv_async_t);
#[unsafe(export_name = "uv_async_init")]
unsafe extern "C" fn _napi_uv_async_init(
  r#loop: *mut uv_loop_t,
  // probably uninitialized
  r#async: *mut uv_async_t,
  async_cb: uv_async_cb,
) -> c_int {
  unsafe {
    addr_of_mut!((*r#async).r#loop).write(r#loop);
    addr_of_mut!((*r#async).r#type).write(uv_handle_type::UV_ASYNC);
    addr_of_mut!((*r#async).async_cb).write(async_cb);

    let mut resource_name: MaybeUninit<napi_value> = MaybeUninit::uninit();
    assert_ok(napi_create_string_utf8(
      r#loop,
      c"uv_async".as_ptr(),
      usize::MAX,
      resource_name.as_mut_ptr(),
    ));
    let resource_name = resource_name.assume_init();

    let res = napi_create_async_work(
      r#loop,
      None::<v8::Local<'static, v8::Value>>.into(),
      resource_name,
      Some(async_exec_wrap),
      None,
      r#async.cast(),
      addr_of_mut!((*r#async).work),
    );

    // In libuv, uv_async_init starts the handle and keeps the event loop
    // alive until uv_close is called. Ref the event loop to match this.
    let env = &mut *r#loop;
    env.external_ops_tracker.ref_op();

    -res
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_async_send(handle: *mut uv_async_t) -> c_int {
  // Dispatch directly to the main thread. Unlike napi_queue_async_work (which
  // runs `execute` on a worker thread), uv_async callbacks need V8 access so
  // they must run on the main thread.
  unsafe {
    let env = &mut *(*handle).r#loop;
    let handle = SendPtr(handle as *const uv_async_t);
    env.async_work_sender.spawn(move |_| {
      let handle = handle.take() as *mut uv_async_t;
      ((*handle).async_cb)(handle);
    });
  }
  0
}

type uv_close_cb = unsafe extern "C" fn(*mut uv_handle_t);

#[unsafe(export_name = "uv_close")]
unsafe extern "C" fn _napi_uv_close(
  handle: *mut uv_handle_t,
  close: Option<uv_close_cb>,
) {
  unsafe {
    if handle.is_null() {
      if let Some(close) = close {
        close(handle);
      }
      return;
    }
    match (*handle).r#type {
      uv_handle_type::UV_ASYNC => {
        let handle: *mut uv_async_t = handle.cast();
        napi_delete_async_work((*handle).r#loop, (*handle).work);
        // Unref the event loop to match the ref in uv_async_init.
        let env = &mut *(*handle).r#loop;
        env.external_ops_tracker.unref_op();
      }
      uv_handle_type::UV_TIMER => {
        let handle: *mut uv_timer_t = handle.cast();
        if timer_close(handle, close) {
          // The uv_compat close callback will run the user close_cb on its
          // own schedule; don't double-fire it from here.
          return;
        }
      }
      _ => {}
    }
    if let Some(close) = close {
      close(handle);
    }
  }
}

// ---------- uv timer / cpu_info / misc polyfills ----------
//
// The Sentry profiling-node native addon (and a handful of other native
// addons that link against libuv directly) reaches into libuv for handle
// types beyond `uv_async_t` and `uv_mutex_t`. Deno does not run on libuv,
// so we satisfy these symbols with lightweight polyfills:
//
// * `uv_hrtime` returns a monotonic timestamp.
// * `uv_handle_set_data`, `uv_ref`, `uv_unref`, `uv_is_closing` mirror the
//   trivial libuv behavior.
// * `uv_cpu_info` returns an error so callers degrade gracefully (the
//   profiler skips per-tick CPU stats but still produces a valid profile).
// * `uv_timer_*` is bridged onto deno_core's libuv-compat layer (see
//   `deno_core::uv_compat`). That layer is the same one driving Node's
//   timer/idle/check/prepare handles on top of tokio, so timers scheduled
//   by a NAPI addon (e.g. the Sentry profiler's measurement ticker) fire
//   on the Deno event loop. We keep our own libuv-ABI `uv_timer_t` so the
//   addon-allocated struct layout matches what its compiler saw in
//   `<uv.h>`, and stash a pointer to a heap-allocated bridge containing
//   the matching `uv_compat::uv_timer_t` plus user callbacks in the
//   private padding area.
//
// We still return null from `uv_default_loop()` (the napi-style loop is
// the Env pointer, which addons get from `napi_get_uv_event_loop`).
// Addons that use `uv_default_loop()` purely to pass through to
// `uv_timer_init` (as Sentry's profiling-node addon does) are unaffected
// — our `uv_timer_init` ignores the supplied loop and resolves the
// real backing loop from the thread-local.

#[cfg(unix)]
const UV_TIMER_SIZE: usize = 152;

#[cfg(windows)]
const UV_TIMER_SIZE: usize = 160;

type uv_timer_cb = Option<unsafe extern "C" fn(handle: *mut uv_timer_t)>;

// The deno_core uv_compat loop the current JsRuntime is using. Populated by
// `register_default_uv_loop` on each `op_napi_open` call so that
// `uv_default_loop()` (called from native addons) and `uv_timer_init` with
// a null loop fall back to a real, tokio-backed loop. Per-thread because
// each JsRuntime is pinned to a thread.
thread_local! {
  static UV_DEFAULT_LOOP: Cell<*mut uv_compat::uv_loop_t> = const {
    Cell::new(std::ptr::null_mut())
  };
}

pub(crate) fn register_default_uv_loop(loop_ptr: *mut uv_compat::uv_loop_t) {
  UV_DEFAULT_LOOP.with(|cell| cell.set(loop_ptr));
}

fn current_uv_compat_loop() -> *mut uv_compat::uv_loop_t {
  UV_DEFAULT_LOOP.with(|cell| cell.get())
}

// Heap-allocated bridge between a libuv-ABI `uv_timer_t` exposed to the
// NAPI addon and a `uv_compat::uv_timer_t` driven by the Deno event loop.
//
// `inner` is the first field so `*mut NapiTimerBridge` and
// `*mut uv_compat::uv_timer_t` share an address — the trampoline
// callbacks cast between them. The bridge box is freed in the
// uv_compat close callback so we don't drop state while it is still
// queued in the closing-handles list.
#[repr(C)]
struct NapiTimerBridge {
  inner: uv_compat::uv_timer_t,
  napi_handle: *mut uv_timer_t,
  user_cb: Option<unsafe extern "C" fn(handle: *mut uv_timer_t)>,
  user_close_cb: Option<unsafe extern "C" fn(handle: *mut uv_handle_t)>,
}

#[repr(C)]
struct uv_timer_t {
  // public members (must match libuv layout)
  pub data: *mut c_void,
  pub r#loop: *mut uv_loop_t,
  pub r#type: uv_handle_type,

  // Pointer to the heap-allocated bridge. Null if the timer was
  // initialized without a uv_compat loop available (in which case all
  // timer operations are silent no-ops, matching the old behavior).
  bridge: *mut NapiTimerBridge,

  _padding: [MaybeUninit<usize>; const {
    (UV_TIMER_SIZE
      - size_of::<*mut c_void>()
      - size_of::<*mut uv_loop_t>()
      - size_of::<uv_handle_type>()
      - size_of::<*mut NapiTimerBridge>())
      / size_of::<usize>()
  }],
}

// Called by uv_compat when the timer fires. The handle pointer is the
// `inner` field of `NapiTimerBridge`, so we can read user_cb/napi_handle
// from there and deliver the callback with the addon-facing handle.
unsafe extern "C" fn timer_cb_trampoline(handle: *mut uv_compat::uv_timer_t) {
  unsafe {
    let bridge = handle as *mut NapiTimerBridge;
    let napi_handle = (*bridge).napi_handle;
    if let Some(cb) = (*bridge).user_cb {
      cb(napi_handle);
    }
  }
}

// Close callback for the uv_compat timer. Runs after uv_compat finishes
// closing the handle, so it is safe to free the bridge box here.
// Note: per libuv's contract the addon's `uv_timer_t` is invalid after
// uv_close fires the close callback (the addon may free or stack-pop it
// inside the callback), so we don't touch napi_handle after dispatching.
unsafe extern "C" fn timer_close_trampoline(
  handle: *mut uv_compat::uv_handle_t,
) {
  unsafe {
    let bridge_ptr = handle as *mut NapiTimerBridge;
    let napi_handle = (*bridge_ptr).napi_handle;
    let close_cb = (*bridge_ptr).user_close_cb;
    drop(Box::from_raw(bridge_ptr));
    if let Some(cb) = close_cb {
      cb(napi_handle.cast());
    }
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_timer_init(
  r#loop: *mut uv_loop_t,
  timer: *mut uv_timer_t,
) -> c_int {
  unsafe {
    addr_of_mut!((*timer).data).write(std::ptr::null_mut());
    addr_of_mut!((*timer).r#loop).write(r#loop);
    addr_of_mut!((*timer).r#type).write(uv_handle_type::UV_TIMER);
    addr_of_mut!((*timer).bridge).write(std::ptr::null_mut());

    // Pick up the active uv_compat loop. We ignore the addon-supplied
    // `loop` (which is the napi Env pointer in our world) and use the
    // thread-local instead — see register_default_uv_loop.
    let compat_loop = current_uv_compat_loop();
    if compat_loop.is_null() {
      // No active runtime / loop. Leave the bridge null; subsequent
      // uv_timer_* calls degrade to no-ops, matching the old behavior.
      return 0;
    }

    // Allocate a zero-initialized bridge. `inner` is then initialized by
    // uv_compat::uv_timer_init.
    let mut bridge_box: Box<MaybeUninit<NapiTimerBridge>> =
      Box::new(MaybeUninit::zeroed());
    let bridge_ptr = bridge_box.as_mut_ptr();
    // SAFETY: bridge_ptr points to zeroed (valid for the underlying
    // primitive fields) and writable memory.
    uv_compat::uv_timer_init(compat_loop, addr_of_mut!((*bridge_ptr).inner));
    addr_of_mut!((*bridge_ptr).napi_handle).write(timer);
    addr_of_mut!((*bridge_ptr).user_cb).write(None);
    addr_of_mut!((*bridge_ptr).user_close_cb).write(None);

    addr_of_mut!((*timer).bridge).write(bridge_ptr);
    // Keep the Box alive until uv_close fires the close trampoline.
    let _ = Box::into_raw(bridge_box);
  }
  0
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_timer_start(
  handle: *mut uv_timer_t,
  cb: uv_timer_cb,
  timeout_ms: u64,
  repeat_ms: u64,
) -> c_int {
  unsafe {
    let bridge = (*handle).bridge;
    if bridge.is_null() {
      return 0;
    }
    (*bridge).user_cb = cb;
    uv_compat::uv_timer_start(
      addr_of_mut!((*bridge).inner),
      timer_cb_trampoline,
      timeout_ms,
      repeat_ms,
    )
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_timer_stop(handle: *mut uv_timer_t) -> c_int {
  unsafe {
    let bridge = (*handle).bridge;
    if bridge.is_null() {
      return 0;
    }
    uv_compat::uv_timer_stop(addr_of_mut!((*bridge).inner))
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_timer_set_repeat(
  handle: *mut uv_timer_t,
  repeat_ms: u64,
) {
  unsafe {
    let bridge = (*handle).bridge;
    if bridge.is_null() {
      return;
    }
    uv_compat::uv_timer_set_repeat(addr_of_mut!((*bridge).inner), repeat_ms);
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_timer_get_repeat(handle: *const uv_timer_t) -> u64 {
  unsafe {
    let bridge = (*handle).bridge;
    if bridge.is_null() {
      return 0;
    }
    uv_compat::uv_timer_get_repeat(addr_of_mut!((*bridge).inner))
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_timer_again(handle: *mut uv_timer_t) -> c_int {
  unsafe {
    let bridge = (*handle).bridge;
    if bridge.is_null() {
      return 0;
    }
    uv_compat::uv_timer_again(addr_of_mut!((*bridge).inner))
  }
}

unsafe fn timer_close(
  handle: *mut uv_timer_t,
  close: Option<uv_close_cb>,
) -> bool {
  unsafe {
    let bridge = (*handle).bridge;
    if bridge.is_null() {
      return false;
    }
    (*bridge).user_close_cb = close;
    uv_compat::uv_close(
      addr_of_mut!((*bridge).inner) as *mut uv_compat::uv_handle_t,
      Some(timer_close_trampoline),
    );
    true
  }
}

// uv_hrtime returns nanoseconds since an arbitrary monotonic origin. We
// peg the origin to the first call.
#[unsafe(no_mangle)]
unsafe extern "C" fn uv_hrtime() -> u64 {
  static START: OnceLock<Instant> = OnceLock::new();
  let start = START.get_or_init(Instant::now);
  start.elapsed().as_nanos() as u64
}

// Many native addons reach for `uv_default_loop()` because they predate
// `napi_get_uv_event_loop`. We return null — our uv_timer_* polyfills
// ignore the supplied loop pointer and resolve the real uv_compat loop
// from the thread-local registered at `op_napi_open` time, and our
// uv_async_* polyfills require an Env loop (the napi-style loop is the
// Env pointer, which addons get from `napi_get_uv_event_loop` instead).
#[unsafe(no_mangle)]
unsafe extern "C" fn uv_default_loop() -> *mut uv_loop_t {
  std::ptr::null_mut()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_is_closing(_handle: *const uv_handle_t) -> c_int {
  0
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_is_active(_handle: *const uv_handle_t) -> c_int {
  0
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_ref(_handle: *mut uv_handle_t) {}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_unref(_handle: *mut uv_handle_t) {}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_has_ref(_handle: *const uv_handle_t) -> c_int {
  0
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_handle_set_data(
  handle: *mut uv_handle_t,
  data: *mut c_void,
) {
  if handle.is_null() {
    return;
  }
  unsafe {
    (*handle).data = data;
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_handle_get_data(
  handle: *const uv_handle_t,
) -> *mut c_void {
  if handle.is_null() {
    return std::ptr::null_mut();
  }
  unsafe { (*handle).data }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_handle_get_loop(
  handle: *const uv_handle_t,
) -> *mut uv_loop_t {
  if handle.is_null() {
    return std::ptr::null_mut();
  }
  unsafe { (*handle).r#loop }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_handle_get_type(handle: *const uv_handle_t) -> c_int {
  if handle.is_null() {
    return uv_handle_type::UV_UNKNOWN_HANDLE as c_int;
  }
  unsafe { (*handle).r#type as c_int }
}

// uv_cpu_info: report no available CPU info. Callers (e.g. Sentry's
// profiler) treat this as a non-fatal degradation.
#[unsafe(no_mangle)]
unsafe extern "C" fn uv_cpu_info(
  _cpu_infos: *mut *mut c_void,
  count: *mut c_int,
) -> c_int {
  if !count.is_null() {
    unsafe { *count = 0 };
  }
  // UV_ENOSYS (-libc::ENOSYS on unix); -4093 matches libuv's numbering for
  // ENOSYS on Linux. Any non-zero return signals "unsupported" to the addon.
  -4093
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_free_cpu_info(_cpu_infos: *mut c_void, _count: c_int) {
  // uv_cpu_info never allocates in our polyfill.
}

// ---------- uv thread / semaphore polyfills ----------
//
// Native addons that link against libuv directly frequently use libuv's
// portable threading and synchronization primitives (`uv_thread_*`,
// `uv_sem_*`) for their own background work instead of the raw OS APIs —
// e.g. a worker thread that signals readiness through a counting
// semaphore. Deno does not run on libuv, but these primitives are
// self-contained (they never touch the event loop), so we back them with
// Rust's std threading and a parking_lot-based counting semaphore.
//
// libuv lets the addon allocate the opaque handle structs itself, and
// their sizes are platform specific (`uv_sem_t` is a 4-byte mach
// `semaphore_t` on macOS but a 32-byte `sem_t` on Linux). To avoid
// depending on those layouts we store only a small integer token in the
// addon-provided struct and keep the real state in a process-global
// registry: a `u32` token for semaphores (fits `uv_sem_t`'s 4-byte
// minimum) and a `u64` token for threads (`uv_thread_t` is always
// pointer-sized).

type uv_thread_cb = unsafe extern "C" fn(arg: *mut c_void);

struct SemInner {
  count: Mutex<i64>,
  cond: deno_core::parking_lot::Condvar,
}

static SEMS: OnceLock<
  Mutex<std::collections::HashMap<u32, std::sync::Arc<SemInner>>>,
> = OnceLock::new();
static SEM_NEXT: std::sync::atomic::AtomicU32 =
  std::sync::atomic::AtomicU32::new(1);

fn sems()
-> &'static Mutex<std::collections::HashMap<u32, std::sync::Arc<SemInner>>> {
  SEMS.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn sem_lookup(sem: *const u32) -> Option<std::sync::Arc<SemInner>> {
  if sem.is_null() {
    return None;
  }
  let id = unsafe { *sem };
  sems().lock().get(&id).cloned()
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_sem_init(
  sem: *mut u32,
  value: std::ffi::c_uint,
) -> c_int {
  if sem.is_null() {
    return -1;
  }
  let id = SEM_NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
  let inner = std::sync::Arc::new(SemInner {
    count: Mutex::new(value as i64),
    cond: deno_core::parking_lot::Condvar::new(),
  });
  sems().lock().insert(id, inner);
  unsafe {
    *sem = id;
  }
  0
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_sem_destroy(sem: *mut u32) {
  if sem.is_null() {
    return;
  }
  let id = unsafe { *sem };
  sems().lock().remove(&id);
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_sem_post(sem: *mut u32) {
  if let Some(inner) = sem_lookup(sem) {
    let mut count = inner.count.lock();
    *count += 1;
    inner.cond.notify_one();
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_sem_wait(sem: *mut u32) {
  if let Some(inner) = sem_lookup(sem) {
    let mut count = inner.count.lock();
    while *count == 0 {
      inner.cond.wait(&mut count);
    }
    *count -= 1;
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_sem_trywait(sem: *mut u32) -> c_int {
  match sem_lookup(sem) {
    Some(inner) => {
      let mut count = inner.count.lock();
      if *count > 0 {
        *count -= 1;
        0
      } else {
        // UV_EAGAIN (-EAGAIN on Linux). Any non-zero return tells the
        // caller the semaphore could not be decremented without blocking.
        -11
      }
    }
    None => -1,
  }
}

static THREADS: OnceLock<
  Mutex<std::collections::HashMap<u64, std::thread::JoinHandle<()>>>,
> = OnceLock::new();
static THREAD_NEXT: std::sync::atomic::AtomicU64 =
  std::sync::atomic::AtomicU64::new(1);

fn threads()
-> &'static Mutex<std::collections::HashMap<u64, std::thread::JoinHandle<()>>> {
  THREADS.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

thread_local! {
  // The libuv thread id of the currently running thread, or 0 for threads
  // not created through `uv_thread_create` (e.g. the main thread).
  static CURRENT_UV_THREAD_ID: Cell<u64> = const { Cell::new(0) };
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_thread_create(
  tid: *mut u64,
  entry: uv_thread_cb,
  arg: *mut c_void,
) -> c_int {
  if tid.is_null() {
    return -1;
  }
  let id = THREAD_NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
  let arg = SendPtr(arg as *const c_void);
  let spawned = std::thread::Builder::new().spawn(move || {
    CURRENT_UV_THREAD_ID.with(|c| c.set(id));
    let arg = arg.take() as *mut c_void;
    // SAFETY: `entry` is a valid C callback supplied by the addon.
    unsafe {
      entry(arg);
    }
  });
  match spawned {
    Ok(handle) => {
      threads().lock().insert(id, handle);
      unsafe {
        *tid = id;
      }
      0
    }
    Err(_) => -1,
  }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_thread_join(tid: *mut u64) -> c_int {
  if tid.is_null() {
    return -1;
  }
  let id = unsafe { *tid };
  let handle = threads().lock().remove(&id);
  if let Some(handle) = handle {
    let _ = handle.join();
  }
  0
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_thread_self() -> u64 {
  CURRENT_UV_THREAD_ID.with(|c| c.get())
}

#[unsafe(no_mangle)]
unsafe extern "C" fn uv_thread_equal(t1: *const u64, t2: *const u64) -> c_int {
  if t1.is_null() || t2.is_null() {
    return 0;
  }
  (unsafe { *t1 == *t2 }) as c_int
}

unsafe extern "C" fn async_exec_wrap(_env: napi_env, data: *mut c_void) {
  let data: *mut uv_async_t = data.cast();
  unsafe {
    ((*data).async_cb)(data);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn sizes() {
    assert_eq!(
      std::mem::size_of::<libuv_sys_lite::uv_mutex_t>(),
      UV_MUTEX_SIZE
    );
    assert_eq!(
      std::mem::size_of::<libuv_sys_lite::uv_handle_t>(),
      UV_HANDLE_SIZE
    );
    assert_eq!(
      std::mem::size_of::<libuv_sys_lite::uv_async_t>(),
      UV_ASYNC_SIZE
    );
    assert_eq!(std::mem::size_of::<uv_mutex_t>(), UV_MUTEX_SIZE);
    assert_eq!(std::mem::size_of::<uv_handle_t>(), UV_HANDLE_SIZE);
    assert_eq!(std::mem::size_of::<uv_async_t>(), UV_ASYNC_SIZE);
    assert_eq!(
      std::mem::size_of::<libuv_sys_lite::uv_timer_t>(),
      UV_TIMER_SIZE
    );
    assert_eq!(std::mem::size_of::<uv_timer_t>(), UV_TIMER_SIZE);
  }

  // Drives the uv_sem_* / uv_thread_* polyfills the way a native addon
  // would: a worker thread increments a counter and posts a counting
  // semaphore three times while the main thread drains the semaphore and
  // joins the worker.
  #[test]
  fn thread_and_semaphore() {
    struct Shared {
      sem: u32,
      counter: i32,
    }

    unsafe extern "C" fn worker(arg: *mut c_void) {
      let shared = arg as *mut Shared;
      unsafe {
        for _ in 0..3 {
          (*shared).counter += 1;
          uv_sem_post(addr_of_mut!((*shared).sem));
        }
      }
    }

    unsafe {
      let mut shared = Shared { sem: 0, counter: 0 };
      let shared_ptr: *mut Shared = &mut shared;
      let sem_ptr: *mut u32 = addr_of_mut!((*shared_ptr).sem);
      assert_eq!(uv_sem_init(sem_ptr, 0), 0);

      let mut tid: u64 = 0;
      let tid_ptr: *mut u64 = &mut tid;
      assert_eq!(uv_thread_create(tid_ptr, worker, shared_ptr.cast()), 0);

      // Blocks until the worker has posted three times.
      for _ in 0..3 {
        uv_sem_wait(sem_ptr);
      }
      assert_eq!(uv_thread_join(tid_ptr), 0);
      assert_eq!((*shared_ptr).counter, 3);

      // The count is back to zero, so a non-blocking wait must fail.
      assert_ne!(uv_sem_trywait(sem_ptr), 0);
      assert_ne!(uv_thread_equal(tid_ptr, tid_ptr), 0);
      uv_sem_destroy(sem_ptr);
    }
  }
}
