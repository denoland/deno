use std::cell::Cell;
use std::ffi::c_void;
use std::future::poll_fn;
use std::rc::Rc;
use std::task::Poll;

use super::tcp::AF_INET;
use super::tcp::sockaddr_in;
use crate::JsRuntime;
use crate::PollEventLoopOptions;
use crate::uv_compat::*;

fn assert_ok(status: i32) {
  assert_eq!(status, 0);
}

async fn run_test(f: impl AsyncFnOnce(&mut JsRuntime, *mut uv_loop_t)) {
  let mut runtime = JsRuntime::new(Default::default());
  let uv_loop = Box::into_raw(Box::<uv_loop_t>::new_uninit());
  let uv_loop = unsafe {
    assert_ok(uv_loop_init(uv_loop.cast()));
    uv_loop.cast()
  };

  unsafe {
    runtime.register_uv_loop(uv_loop);
  }

  f(&mut runtime, uv_loop).await;

  unsafe {
    uv_loop_close(uv_loop);
  }
  drop(runtime);
  let _ = unsafe { Box::from_raw(uv_loop) };
}

/// Tick the event loop once.
fn tick(runtime: &mut JsRuntime) {
  let _ = runtime.poll_event_loop(
    &mut std::task::Context::from_waker(std::task::Waker::noop()),
    PollEventLoopOptions::default(),
  );
}

// ========== Loop lifecycle ==========

#[tokio::test(flavor = "current_thread")]
async fn loop_init_and_close() {
  let uv_loop = Box::into_raw(Box::<uv_loop_t>::new_uninit());
  unsafe {
    assert_ok(uv_loop_init(uv_loop.cast()));
    assert_ok(uv_loop_close(uv_loop.cast()));
    let _ = Box::from_raw(uv_loop);
  }
}

#[tokio::test(flavor = "current_thread")]
async fn uv_now_returns_nonzero_after_delay() {
  run_test(async |_runtime, uv_loop| {
    let t1 = unsafe { uv_now(uv_loop) };
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    let t2 = unsafe { uv_now(uv_loop) };
    assert!(t2 >= t1);
  })
  .await;
}

// ========== Timer tests ==========

#[tokio::test(flavor = "current_thread")]
async fn timer_init_sets_fields() {
  run_test(async |_runtime, uv_loop| {
    let mut timer = std::mem::MaybeUninit::<uv_timer_t>::uninit();
    unsafe {
      assert_ok(uv_timer_init(uv_loop, timer.as_mut_ptr()));
      let timer = timer.assume_init_ref();
      assert_eq!(timer.r#type, uv_handle_type::UV_TIMER);
      assert_eq!(timer.loop_, uv_loop);
      assert!(timer.data.is_null());
      assert_eq!(uv_is_active(timer as *const _ as *const uv_handle_t), 0);
    }
  })
  .await;
}

#[tokio::test(flavor = "current_thread")]
async fn timer_fires_callback() {
  run_test(async |runtime, uv_loop| {
    let fired = Rc::new(Cell::new(false));
    let fired_ptr = Rc::into_raw(fired.clone());

    unsafe extern "C" fn timer_cb(handle: *mut uv_timer_t) {
      let fired = unsafe { Rc::from_raw((*handle).data as *const Cell<bool>) };
      fired.set(true);
      // Re-leak so the Rc lives until the test checks it.
      let _ = Rc::into_raw(fired);
    }

    let mut timer = std::mem::MaybeUninit::<uv_timer_t>::uninit();
    unsafe {
      uv_timer_init(uv_loop, timer.as_mut_ptr());
      let timer = timer.as_mut_ptr();
      (*timer).data = fired_ptr as *mut c_void;
      uv_timer_start(timer, timer_cb, 0, 0);
      assert_eq!(uv_is_active(timer as *const uv_handle_t), 1);
    }

    // Tick the event loop so timers fire.
    tick(runtime);

    assert!(fired.get());

    // Clean up the leaked Rc.
    unsafe {
      Rc::from_raw(fired_ptr);
    }
  })
  .await;
}

#[tokio::test(flavor = "current_thread")]
async fn timer_repeat() {
  run_test(async |runtime, uv_loop| {
    let count = Rc::new(Cell::new(0u32));
    let count_ptr = Rc::into_raw(count.clone());

    unsafe extern "C" fn timer_cb(handle: *mut uv_timer_t) {
      let count = unsafe { Rc::from_raw((*handle).data as *const Cell<u32>) };
      count.set(count.get() + 1);
      let _ = Rc::into_raw(count);
    }

    let mut timer = std::mem::MaybeUninit::<uv_timer_t>::uninit();
    let timer_ptr = timer.as_mut_ptr();
    unsafe {
      uv_timer_init(uv_loop, timer_ptr);
      (*timer_ptr).data = count_ptr as *mut c_void;
      // repeat every 1ms, first fire at 0ms
      uv_timer_start(timer_ptr, timer_cb, 0, 1);
    }

    // Tick a few times with small delays.
    for _ in 0..5 {
      tick(runtime);
      tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    }

    let final_count = count.get();
    assert!(
      final_count >= 2,
      "Expected repeat timer to fire at least twice, got {final_count}"
    );

    unsafe {
      uv_timer_stop(timer_ptr);
    }

    // Clean up.
    unsafe {
      Rc::from_raw(count_ptr);
    }
  })
  .await;
}

#[tokio::test(flavor = "current_thread")]
async fn timer_stop_prevents_firing() {
  run_test(async |runtime, uv_loop| {
    let fired = Rc::new(Cell::new(false));
    let fired_ptr = Rc::into_raw(fired.clone());

    unsafe extern "C" fn timer_cb(handle: *mut uv_timer_t) {
      let fired = unsafe { Rc::from_raw((*handle).data as *const Cell<bool>) };
      fired.set(true);
      let _ = Rc::into_raw(fired);
    }

    let mut timer = std::mem::MaybeUninit::<uv_timer_t>::uninit();
    let timer_ptr = timer.as_mut_ptr();
    unsafe {
      uv_timer_init(uv_loop, timer_ptr);
      (*timer_ptr).data = fired_ptr as *mut c_void;
      uv_timer_start(timer_ptr, timer_cb, 10, 0);
      uv_timer_stop(timer_ptr);
      assert_eq!(uv_is_active(timer_ptr as *const uv_handle_t), 0);
    }

    tick(runtime);
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    tick(runtime);

    assert!(!fired.get());

    unsafe {
      Rc::from_raw(fired_ptr);
    }
  })
  .await;
}

#[tokio::test(flavor = "current_thread")]
async fn timer_again_requires_repeat() {
  run_test(async |_runtime, uv_loop| {
    unsafe extern "C" fn noop_cb(_: *mut uv_timer_t) {}

    let mut timer = std::mem::MaybeUninit::<uv_timer_t>::uninit();
    let timer_ptr = timer.as_mut_ptr();
    unsafe {
      uv_timer_init(uv_loop, timer_ptr);

      // uv_timer_again on a never-started timer returns UV_EINVAL
      // (because cb is None).
      let status = uv_timer_again(timer_ptr);
      assert_eq!(status, UV_EINVAL);

      // Start with repeat = 0
      uv_timer_start(timer_ptr, noop_cb, 100, 0);
      // uv_timer_again with repeat=0 is a no-op (returns 0), matching libuv.
      let status = uv_timer_again(timer_ptr);
      assert_ok(status);

      // Set repeat, then again should succeed and restart the timer
      uv_timer_set_repeat(timer_ptr, 50);
      assert_eq!(uv_timer_get_repeat(timer_ptr), 50);
      let status = uv_timer_again(timer_ptr);
      assert_ok(status);

      uv_timer_stop(timer_ptr);
    }
  })
  .await;
}

#[tokio::test(flavor = "current_thread")]
async fn timer_get_set_repeat() {
  run_test(async |_runtime, uv_loop| {
    let mut timer = std::mem::MaybeUninit::<uv_timer_t>::uninit();
    let timer_ptr = timer.as_mut_ptr();
    unsafe {
      uv_timer_init(uv_loop, timer_ptr);
      assert_eq!(uv_timer_get_repeat(timer_ptr), 0);
      uv_timer_set_repeat(timer_ptr, 42);
      assert_eq!(uv_timer_get_repeat(timer_ptr), 42);
    }
  })
  .await;
}

// ========== Idle tests ==========

#[tokio::test(flavor = "current_thread")]
async fn idle_fires_callback() {
  run_test(async |runtime, uv_loop| {
    let fired = Rc::new(Cell::new(false));
    let fired_ptr = Rc::into_raw(fired.clone());

    unsafe extern "C" fn idle_cb(handle: *mut uv_idle_t) {
      let fired = unsafe { Rc::from_raw((*handle).data as *const Cell<bool>) };
      fired.set(true);
      let _ = Rc::into_raw(fired);
    }

    let mut idle = std::mem::MaybeUninit::<uv_idle_t>::uninit();
    let idle_ptr = idle.as_mut_ptr();
    unsafe {
      uv_idle_init(uv_loop, idle_ptr);
      (*idle_ptr).data = fired_ptr as *mut c_void;
      uv_idle_start(idle_ptr, idle_cb);
      assert_eq!(uv_is_active(idle_ptr as *const uv_handle_t), 1);
    }

    tick(runtime);
    assert!(fired.get());

    unsafe {
      uv_idle_stop(idle_ptr);
      assert_eq!(uv_is_active(idle_ptr as *const uv_handle_t), 0);
      Rc::from_raw(fired_ptr);
    }
  })
  .await;
}

#[tokio::test(flavor = "current_thread")]
async fn idle_stop_prevents_further_callbacks() {
  run_test(async |runtime, uv_loop| {
    let count = Rc::new(Cell::new(0u32));
    let count_ptr = Rc::into_raw(count.clone());

    unsafe extern "C" fn idle_cb(handle: *mut uv_idle_t) {
      let count = unsafe { Rc::from_raw((*handle).data as *const Cell<u32>) };
      count.set(count.get() + 1);
      let _ = Rc::into_raw(count);
    }

    let mut idle = std::mem::MaybeUninit::<uv_idle_t>::uninit();
    let idle_ptr = idle.as_mut_ptr();
    unsafe {
      uv_idle_init(uv_loop, idle_ptr);
      (*idle_ptr).data = count_ptr as *mut c_void;
      uv_idle_start(idle_ptr, idle_cb);
    }

    tick(runtime);
    let after_first = count.get();
    assert!(after_first >= 1);

    unsafe { uv_idle_stop(idle_ptr) };

    tick(runtime);
    assert_eq!(count.get(), after_first, "idle should not fire after stop");

    unsafe {
      Rc::from_raw(count_ptr);
    }
  })
  .await;
}

// ========== Prepare tests ==========

#[tokio::test(flavor = "current_thread")]
async fn prepare_fires_callback() {
  run_test(async |runtime, uv_loop| {
    let fired = Rc::new(Cell::new(false));
    let fired_ptr = Rc::into_raw(fired.clone());

    unsafe extern "C" fn prepare_cb(handle: *mut uv_prepare_t) {
      let fired = unsafe { Rc::from_raw((*handle).data as *const Cell<bool>) };
      fired.set(true);
      let _ = Rc::into_raw(fired);
    }

    let mut prepare = std::mem::MaybeUninit::<uv_prepare_t>::uninit();
    let prepare_ptr = prepare.as_mut_ptr();
    unsafe {
      uv_prepare_init(uv_loop, prepare_ptr);
      (*prepare_ptr).data = fired_ptr as *mut c_void;
      uv_prepare_start(prepare_ptr, prepare_cb);
      assert_eq!(uv_is_active(prepare_ptr as *const uv_handle_t), 1);
    }

    tick(runtime);
    assert!(fired.get());

    unsafe {
      uv_prepare_stop(prepare_ptr);
      assert_eq!(uv_is_active(prepare_ptr as *const uv_handle_t), 0);
      Rc::from_raw(fired_ptr);
    }
  })
  .await;
}

// ========== Check tests ==========

#[tokio::test(flavor = "current_thread")]
async fn check_fires_callback() {
  run_test(async |runtime, uv_loop| {
    let fired = Rc::new(Cell::new(false));
    let fired_ptr = Rc::into_raw(fired.clone());

    unsafe extern "C" fn check_cb(handle: *mut uv_check_t) {
      let fired = unsafe { Rc::from_raw((*handle).data as *const Cell<bool>) };
      fired.set(true);
      let _ = Rc::into_raw(fired);
    }

    let mut check = std::mem::MaybeUninit::<uv_check_t>::uninit();
    let check_ptr = check.as_mut_ptr();
    unsafe {
      uv_check_init(uv_loop, check_ptr);
      (*check_ptr).data = fired_ptr as *mut c_void;
      uv_check_start(check_ptr, check_cb);
      assert_eq!(uv_is_active(check_ptr as *const uv_handle_t), 1);
    }

    tick(runtime);
    assert!(fired.get());

    unsafe {
      uv_check_stop(check_ptr);
      assert_eq!(uv_is_active(check_ptr as *const uv_handle_t), 0);
      Rc::from_raw(fired_ptr);
    }
  })
  .await;
}

// ========== uv_close tests ==========

#[tokio::test(flavor = "current_thread")]
async fn close_fires_callback() {
  run_test(async |runtime, uv_loop| {
    let closed = Rc::new(Cell::new(false));
    let closed_ptr = Rc::into_raw(closed.clone());

    unsafe extern "C" fn close_cb(handle: *mut uv_handle_t) {
      let closed = unsafe { Rc::from_raw((*handle).data as *const Cell<bool>) };
      closed.set(true);
      let _ = Rc::into_raw(closed);
    }

    let mut timer = std::mem::MaybeUninit::<uv_timer_t>::uninit();
    let timer_ptr = timer.as_mut_ptr();
    unsafe {
      uv_timer_init(uv_loop, timer_ptr);
      (*timer_ptr).data = closed_ptr as *mut c_void;
      assert_eq!(uv_is_closing(timer_ptr as *const uv_handle_t), 0);
      uv_close(timer_ptr as *mut uv_handle_t, Some(close_cb));
      assert_eq!(uv_is_closing(timer_ptr as *const uv_handle_t), 1);
    }

    tick(runtime);
    assert!(closed.get());

    unsafe {
      Rc::from_raw(closed_ptr);
    }
  })
  .await;
}

#[tokio::test(flavor = "current_thread")]
async fn close_without_callback() {
  run_test(async |runtime, uv_loop| {
    let mut idle = std::mem::MaybeUninit::<uv_idle_t>::uninit();
    let idle_ptr = idle.as_mut_ptr();
    unsafe {
      uv_idle_init(uv_loop, idle_ptr);
      uv_close(idle_ptr as *mut uv_handle_t, None);
      assert_eq!(uv_is_closing(idle_ptr as *const uv_handle_t), 1);
    }
    // Should not crash.
    tick(runtime);
  })
  .await;
}

// ========== uv_ref / uv_unref ==========

#[tokio::test(flavor = "current_thread")]
async fn ref_unref_toggle() {
  run_test(async |_runtime, uv_loop| {
    let mut timer = std::mem::MaybeUninit::<uv_timer_t>::uninit();
    let timer_ptr = timer.as_mut_ptr();
    unsafe {
      uv_timer_init(uv_loop, timer_ptr);
      let handle = timer_ptr as *mut uv_handle_t;
      // Timer starts ref'd by default.
      assert_ne!((*handle).flags & 0x2, 0); // UV_HANDLE_REF

      uv_unref(handle);
      assert_eq!((*handle).flags & 0x2, 0);

      uv_ref(handle);
      assert_ne!((*handle).flags & 0x2, 0);
    }
  })
  .await;
}

// ========== uv_ip4_addr ==========

#[tokio::test(flavor = "current_thread")]
async fn ip4_addr_parses_correctly() {
  let mut addr = std::mem::MaybeUninit::<sockaddr_in>::uninit();
  let ip = std::ffi::CString::new("127.0.0.1").unwrap();
  unsafe {
    assert_ok(uv_ip4_addr(ip.as_ptr(), 8080, addr.as_mut_ptr()));
    let addr = addr.assume_init_ref();
    assert_eq!(addr.sin_family as i32, AF_INET);
    assert_eq!(u16::from_be(addr.sin_port), 8080);
    // 127.0.0.1 in network byte order
    let expected = u32::from(std::net::Ipv4Addr::new(127, 0, 0, 1)).to_be();
    assert_eq!(addr.sin_addr.s_addr, expected);
  }
}

#[tokio::test(flavor = "current_thread")]
async fn ip4_addr_invalid_string() {
  let mut addr = std::mem::MaybeUninit::<sockaddr_in>::uninit();
  let ip = std::ffi::CString::new("not-an-ip").unwrap();
  unsafe {
    let status = uv_ip4_addr(ip.as_ptr(), 0, addr.as_mut_ptr());
    assert_eq!(status, UV_EINVAL);
  }
}

// ========== TCP init ==========

#[tokio::test(flavor = "current_thread")]
async fn tcp_init_sets_fields() {
  run_test(async |_runtime, uv_loop| {
    let mut tcp = std::mem::MaybeUninit::<uv_tcp_t>::uninit();
    let tcp_ptr = tcp.as_mut_ptr();
    unsafe {
      assert_ok(uv_tcp_init(uv_loop, tcp_ptr));
      let tcp = tcp.assume_init_ref();
      assert_eq!(tcp.r#type, uv_handle_type::UV_TCP);
      assert_eq!(tcp.loop_, uv_loop);
      assert!(tcp.data.is_null());
    }
  })
  .await;
}

// ========== TCP bind / listen / accept ==========

#[tokio::test(flavor = "current_thread")]
async fn tcp_bind_and_listen() {
  run_test(async |_runtime, uv_loop| {
    let mut server = std::mem::MaybeUninit::<uv_tcp_t>::uninit();
    let server_ptr = server.as_mut_ptr();
    unsafe {
      uv_tcp_init(uv_loop, server_ptr);

      let mut addr = std::mem::MaybeUninit::<sockaddr_in>::uninit();
      let ip = std::ffi::CString::new("127.0.0.1").unwrap();
      uv_ip4_addr(ip.as_ptr(), 0, addr.as_mut_ptr());

      assert_ok(uv_tcp_bind(
        server_ptr,
        addr.as_ptr() as *const c_void,
        0,
        0,
      ));

      unsafe extern "C" fn on_connection(_: *mut uv_stream_t, _: i32) {}

      assert_ok(uv_listen(
        server_ptr as *mut uv_stream_t,
        128,
        Some(on_connection),
      ));

      assert_eq!(uv_is_active(server_ptr as *const uv_handle_t), 1);

      // Verify getsockname works after listen
      let mut name = std::mem::MaybeUninit::<sockaddr_in>::zeroed();
      let mut namelen = std::mem::size_of::<sockaddr_in>() as i32;
      assert_ok(uv_tcp_getsockname(
        server_ptr,
        name.as_mut_ptr() as *mut c_void,
        &mut namelen,
      ));
      let name = name.assume_init_ref();
      let port = u16::from_be(name.sin_port);
      assert!(port > 0, "Expected OS-assigned port > 0, got {port}");

      uv_close(server_ptr as *mut uv_handle_t, None);
    }
  })
  .await;
}

// ========== TCP connect + I/O ==========

#[tokio::test(flavor = "current_thread")]
async fn tcp_connect_and_echo() {
  run_test(async |runtime, uv_loop| {
    // --- Set up a server ---
    let mut server = std::mem::MaybeUninit::<uv_tcp_t>::uninit();
    let server_ptr = server.as_mut_ptr();

    unsafe extern "C" fn on_connection(server: *mut uv_stream_t, status: i32) {
      assert_eq!(status, 0);
      // We don't accept here; the test drives accept manually.
      let _ = server;
    }

    let server_port: u16;
    unsafe {
      uv_tcp_init(uv_loop, server_ptr);

      let mut addr = std::mem::MaybeUninit::<sockaddr_in>::uninit();
      let ip = std::ffi::CString::new("127.0.0.1").unwrap();
      uv_ip4_addr(ip.as_ptr(), 0, addr.as_mut_ptr());

      assert_ok(uv_tcp_bind(
        server_ptr,
        addr.as_ptr() as *const c_void,
        0,
        0,
      ));

      assert_ok(uv_listen(
        server_ptr as *mut uv_stream_t,
        128,
        Some(on_connection),
      ));

      let mut name = std::mem::MaybeUninit::<sockaddr_in>::zeroed();
      let mut namelen = std::mem::size_of::<sockaddr_in>() as i32;
      uv_tcp_getsockname(
        server_ptr,
        name.as_mut_ptr() as *mut c_void,
        &mut namelen,
      );
      server_port = u16::from_be(name.assume_init_ref().sin_port);
    }

    // --- Connect a client ---
    let connected = Rc::new(Cell::new(false));
    let connected_ptr = Rc::into_raw(connected.clone());

    unsafe extern "C" fn on_connect(req: *mut uv_connect_t, status: i32) {
      assert_eq!(status, 0);
      let connected = unsafe { Rc::from_raw((*req).data as *const Cell<bool>) };
      connected.set(true);
      let _ = Rc::into_raw(connected);
    }

    let mut client = std::mem::MaybeUninit::<uv_tcp_t>::uninit();
    let client_ptr = client.as_mut_ptr();
    let mut connect_req = std::mem::MaybeUninit::<uv_connect_t>::uninit();
    let connect_req_ptr = connect_req.as_mut_ptr();

    unsafe {
      uv_tcp_init(uv_loop, client_ptr);
      uv_tcp_nodelay(client_ptr, 1);

      (*connect_req_ptr).data = connected_ptr as *mut c_void;

      let mut addr = std::mem::MaybeUninit::<sockaddr_in>::uninit();
      let ip = std::ffi::CString::new("127.0.0.1").unwrap();
      uv_ip4_addr(ip.as_ptr(), server_port as i32, addr.as_mut_ptr());

      assert_ok(uv_tcp_connect(
        connect_req_ptr,
        client_ptr,
        addr.as_ptr() as *const c_void,
        Some(on_connect),
      ));
    }

    // Poll until connected.
    for _ in 0..100 {
      tick(runtime);
      if connected.get() {
        break;
      }
      tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
    assert!(connected.get(), "Client should have connected");

    // Clean up.
    unsafe {
      uv_close(client_ptr as *mut uv_handle_t, None);
      uv_close(server_ptr as *mut uv_handle_t, None);
      Rc::from_raw(connected_ptr);
    }
    tick(runtime);
  })
  .await;
}

// ========== TCP nodelay ==========

#[tokio::test(flavor = "current_thread")]
async fn tcp_nodelay() {
  run_test(async |_runtime, uv_loop| {
    let mut tcp = std::mem::MaybeUninit::<uv_tcp_t>::uninit();
    let tcp_ptr = tcp.as_mut_ptr();
    unsafe {
      uv_tcp_init(uv_loop, tcp_ptr);
      // Should not error even without a stream.
      assert_ok(uv_tcp_nodelay(tcp_ptr, 1));
      assert_ok(uv_tcp_nodelay(tcp_ptr, 0));
    }
  })
  .await;
}

// ========== TCP keepalive / simultaneous_accepts (no-ops) ==========

#[tokio::test(flavor = "current_thread")]
async fn tcp_keepalive_is_noop() {
  run_test(async |_runtime, uv_loop| {
    let mut tcp = std::mem::MaybeUninit::<uv_tcp_t>::uninit();
    let tcp_ptr = tcp.as_mut_ptr();
    unsafe {
      uv_tcp_init(uv_loop, tcp_ptr);
      assert_ok(uv_tcp_keepalive(tcp_ptr, 1, 60));
      assert_ok(uv_tcp_simultaneous_accepts(tcp_ptr, 1));
    }
  })
  .await;
}

// ========== uv_read_stop ==========

#[tokio::test(flavor = "current_thread")]
async fn read_stop_clears_reading() {
  run_test(async |_runtime, uv_loop| {
    let mut tcp = std::mem::MaybeUninit::<uv_tcp_t>::uninit();
    let tcp_ptr = tcp.as_mut_ptr();
    unsafe {
      uv_tcp_init(uv_loop, tcp_ptr);

      unsafe extern "C" fn alloc_cb(
        _: *mut uv_handle_t,
        _: usize,
        _: *mut uv_buf_t,
      ) {
      }
      unsafe extern "C" fn read_cb(
        _: *mut uv_stream_t,
        _: isize,
        _: *const uv_buf_t,
      ) {
      }

      uv_read_start(tcp_ptr as *mut uv_stream_t, Some(alloc_cb), Some(read_cb));
      assert_eq!(uv_is_active(tcp_ptr as *const uv_handle_t), 1);

      uv_read_stop(tcp_ptr as *mut uv_stream_t);
      assert_eq!(uv_is_active(tcp_ptr as *const uv_handle_t), 0);
    }
  })
  .await;
}

// ========== uv_try_write without stream ==========

#[tokio::test(flavor = "current_thread")]
async fn try_write_no_stream_returns_ebadf() {
  run_test(async |_runtime, uv_loop| {
    let mut tcp = std::mem::MaybeUninit::<uv_tcp_t>::uninit();
    let tcp_ptr = tcp.as_mut_ptr();
    unsafe {
      uv_tcp_init(uv_loop, tcp_ptr);
      let data = b"hello";
      let result = uv_try_write(tcp_ptr as *mut uv_stream_t, data);
      assert_eq!(result, UV_EBADF);
    }
  })
  .await;
}

// ========== new_* constructors ==========

#[test]
fn new_tcp_constructor() {
  let tcp = new_tcp();
  assert_eq!(tcp.r#type, uv_handle_type::UV_TCP);
  assert!(tcp.data.is_null());
  assert!(tcp.loop_.is_null());
}

#[test]
fn new_write_constructor() {
  let w = new_write();
  assert_eq!(w.r#type, 0);
  assert!(w.data.is_null());
  assert!(w.handle.is_null());
}

#[test]
fn new_connect_constructor() {
  let c = new_connect();
  assert_eq!(c.r#type, 0);
  assert!(c.data.is_null());
  assert!(c.handle.is_null());
}

#[test]
fn new_shutdown_constructor() {
  let s = new_shutdown();
  assert_eq!(s.r#type, 0);
  assert!(s.data.is_null());
  assert!(s.handle.is_null());
}

// ========== Phase ordering ==========

#[tokio::test(flavor = "current_thread")]
async fn phase_ordering_idle_prepare_check() {
  run_test(async |runtime, uv_loop| {
    // Verify that idle runs before prepare, and prepare before check,
    // by recording the order callbacks fire in.
    let order = Rc::new(RefCell::new(Vec::<&'static str>::new()));

    use std::cell::RefCell;

    let order_idle = Rc::into_raw(order.clone());
    let order_prepare = Rc::into_raw(order.clone());
    let order_check = Rc::into_raw(order.clone());

    unsafe extern "C" fn idle_cb(handle: *mut uv_idle_t) {
      let order = unsafe {
        Rc::from_raw((*handle).data as *const RefCell<Vec<&'static str>>)
      };
      order.borrow_mut().push("idle");
      let _ = Rc::into_raw(order);
    }
    unsafe extern "C" fn prepare_cb(handle: *mut uv_prepare_t) {
      let order = unsafe {
        Rc::from_raw((*handle).data as *const RefCell<Vec<&'static str>>)
      };
      order.borrow_mut().push("prepare");
      let _ = Rc::into_raw(order);
    }
    unsafe extern "C" fn check_cb(handle: *mut uv_check_t) {
      let order = unsafe {
        Rc::from_raw((*handle).data as *const RefCell<Vec<&'static str>>)
      };
      order.borrow_mut().push("check");
      let _ = Rc::into_raw(order);
    }

    let mut idle = std::mem::MaybeUninit::<uv_idle_t>::uninit();
    let mut prepare = std::mem::MaybeUninit::<uv_prepare_t>::uninit();
    let mut check = std::mem::MaybeUninit::<uv_check_t>::uninit();

    unsafe {
      uv_idle_init(uv_loop, idle.as_mut_ptr());
      (*idle.as_mut_ptr()).data = order_idle as *mut c_void;
      uv_idle_start(idle.as_mut_ptr(), idle_cb);

      uv_prepare_init(uv_loop, prepare.as_mut_ptr());
      (*prepare.as_mut_ptr()).data = order_prepare as *mut c_void;
      uv_prepare_start(prepare.as_mut_ptr(), prepare_cb);

      uv_check_init(uv_loop, check.as_mut_ptr());
      (*check.as_mut_ptr()).data = order_check as *mut c_void;
      uv_check_start(check.as_mut_ptr(), check_cb);
    }

    tick(runtime);

    let phases = order.borrow();
    // The runtime runs: timers -> idle -> prepare -> I/O -> check -> close
    // (matching libuv's phase ordering)
    assert!(
      phases.len() >= 3,
      "Expected at least 3 phases, got {:?}",
      *phases
    );

    // Find first occurrence of each.
    let idle_idx = phases.iter().position(|&s| s == "idle").unwrap();
    let prepare_idx = phases.iter().position(|&s| s == "prepare").unwrap();
    let check_idx = phases.iter().position(|&s| s == "check").unwrap();

    assert!(
      idle_idx < prepare_idx,
      "idle should run before prepare: {:?}",
      *phases
    );
    assert!(
      prepare_idx < check_idx,
      "prepare should run before check: {:?}",
      *phases
    );

    unsafe {
      uv_idle_stop(idle.as_mut_ptr());
      uv_prepare_stop(prepare.as_mut_ptr());
      uv_check_stop(check.as_mut_ptr());

      Rc::from_raw(order_idle);
      Rc::from_raw(order_prepare);
      Rc::from_raw(order_check);
    }
  })
  .await;
}

// ========== idle_start on already-active handle is no-op ==========

#[tokio::test(flavor = "current_thread")]
async fn idle_start_already_active_is_noop() {
  run_test(async |runtime, uv_loop| {
    let count = Rc::new(Cell::new(0u32));
    let count_ptr = Rc::into_raw(count.clone());

    unsafe extern "C" fn cb_a(handle: *mut uv_idle_t) {
      let c = unsafe { Rc::from_raw((*handle).data as *const Cell<u32>) };
      c.set(c.get() + 1);
      let _ = Rc::into_raw(c);
    }
    unsafe extern "C" fn cb_b(_handle: *mut uv_idle_t) {
      // This should never be called -- libuv ignores the new cb.
      panic!(
        "cb_b should not be called; uv_idle_start on active handle is a no-op"
      );
    }

    let mut idle = std::mem::MaybeUninit::<uv_idle_t>::uninit();
    let idle_ptr = idle.as_mut_ptr();
    unsafe {
      uv_idle_init(uv_loop, idle_ptr);
      (*idle_ptr).data = count_ptr as *mut c_void;
      uv_idle_start(idle_ptr, cb_a);
    }

    tick(runtime);
    assert!(count.get() >= 1);

    // Calling uv_idle_start on an already-active handle is a no-op in libuv.
    // The original callback (cb_a) should keep firing, NOT cb_b.
    let before = count.get();
    unsafe {
      uv_idle_start(idle_ptr, cb_b);
    }

    tick(runtime);
    // cb_a should still be firing (if cb_b fired, it would panic).
    assert!(count.get() > before, "original callback should keep firing");

    unsafe {
      uv_idle_stop(idle_ptr);
      Rc::from_raw(count_ptr);
    }
  })
  .await;
}

// ========== Idle stop is idempotent ==========

#[tokio::test(flavor = "current_thread")]
async fn idle_stop_when_not_active_is_noop() {
  run_test(async |_runtime, uv_loop| {
    let mut idle = std::mem::MaybeUninit::<uv_idle_t>::uninit();
    unsafe {
      uv_idle_init(uv_loop, idle.as_mut_ptr());
      // Stop without start should not crash.
      assert_ok(uv_idle_stop(idle.as_mut_ptr()));
    }
  })
  .await;
}

// ========== Event loop keeps running with alive handles ==========

#[tokio::test(flavor = "current_thread")]
async fn event_loop_pending_with_active_timer() {
  run_test(async |runtime, uv_loop| {
    unsafe extern "C" fn noop_cb(_: *mut uv_timer_t) {}

    let mut timer = std::mem::MaybeUninit::<uv_timer_t>::uninit();
    let timer_ptr = timer.as_mut_ptr();
    unsafe {
      uv_timer_init(uv_loop, timer_ptr);
      uv_timer_start(timer_ptr, noop_cb, 100_000, 0);
    }

    // With an active timer, poll_event_loop should return Pending.
    let result = poll_fn(|cx| {
      let poll = runtime.poll_event_loop(cx, PollEventLoopOptions::default());
      Poll::Ready(poll)
    })
    .await;

    assert!(result.is_pending(), "Should be pending with active timer");

    unsafe {
      uv_timer_stop(timer_ptr);
    }
  })
  .await;
}

// ========== Close on each handle type ==========

#[tokio::test(flavor = "current_thread")]
async fn close_idle_handle() {
  run_test(async |runtime, uv_loop| {
    let closed = Rc::new(Cell::new(false));
    let closed_ptr = Rc::into_raw(closed.clone());

    unsafe extern "C" fn close_cb(handle: *mut uv_handle_t) {
      let closed = unsafe { Rc::from_raw((*handle).data as *const Cell<bool>) };
      closed.set(true);
      let _ = Rc::into_raw(closed);
    }

    unsafe extern "C" fn idle_cb(_: *mut uv_idle_t) {}

    let mut idle = std::mem::MaybeUninit::<uv_idle_t>::uninit();
    let idle_ptr = idle.as_mut_ptr();
    unsafe {
      uv_idle_init(uv_loop, idle_ptr);
      (*idle_ptr).data = closed_ptr as *mut c_void;
      uv_idle_start(idle_ptr, idle_cb);
      uv_close(idle_ptr as *mut uv_handle_t, Some(close_cb));
    }

    tick(runtime);
    assert!(closed.get());

    unsafe {
      Rc::from_raw(closed_ptr);
    }
  })
  .await;
}

#[tokio::test(flavor = "current_thread")]
async fn close_prepare_handle() {
  run_test(async |runtime, uv_loop| {
    let closed = Rc::new(Cell::new(false));
    let closed_ptr = Rc::into_raw(closed.clone());

    unsafe extern "C" fn close_cb(handle: *mut uv_handle_t) {
      let closed = unsafe { Rc::from_raw((*handle).data as *const Cell<bool>) };
      closed.set(true);
      let _ = Rc::into_raw(closed);
    }

    unsafe extern "C" fn prepare_cb(_: *mut uv_prepare_t) {}

    let mut prepare = std::mem::MaybeUninit::<uv_prepare_t>::uninit();
    let prepare_ptr = prepare.as_mut_ptr();
    unsafe {
      uv_prepare_init(uv_loop, prepare_ptr);
      (*prepare_ptr).data = closed_ptr as *mut c_void;
      uv_prepare_start(prepare_ptr, prepare_cb);
      uv_close(prepare_ptr as *mut uv_handle_t, Some(close_cb));
    }

    tick(runtime);
    assert!(closed.get());

    unsafe {
      Rc::from_raw(closed_ptr);
    }
  })
  .await;
}

#[tokio::test(flavor = "current_thread")]
async fn close_check_handle() {
  run_test(async |runtime, uv_loop| {
    let closed = Rc::new(Cell::new(false));
    let closed_ptr = Rc::into_raw(closed.clone());

    unsafe extern "C" fn close_cb(handle: *mut uv_handle_t) {
      let closed = unsafe { Rc::from_raw((*handle).data as *const Cell<bool>) };
      closed.set(true);
      let _ = Rc::into_raw(closed);
    }

    unsafe extern "C" fn check_cb(_: *mut uv_check_t) {}

    let mut check = std::mem::MaybeUninit::<uv_check_t>::uninit();
    let check_ptr = check.as_mut_ptr();
    unsafe {
      uv_check_init(uv_loop, check_ptr);
      (*check_ptr).data = closed_ptr as *mut c_void;
      uv_check_start(check_ptr, check_cb);
      uv_close(check_ptr as *mut uv_handle_t, Some(close_cb));
    }

    tick(runtime);
    assert!(closed.get());

    unsafe {
      Rc::from_raw(closed_ptr);
    }
  })
  .await;
}

#[tokio::test(flavor = "current_thread")]
async fn close_tcp_handle() {
  run_test(async |runtime, uv_loop| {
    let closed = Rc::new(Cell::new(false));
    let closed_ptr = Rc::into_raw(closed.clone());

    unsafe extern "C" fn close_cb(handle: *mut uv_handle_t) {
      let closed = unsafe { Rc::from_raw((*handle).data as *const Cell<bool>) };
      closed.set(true);
      let _ = Rc::into_raw(closed);
    }

    let mut tcp = std::mem::MaybeUninit::<uv_tcp_t>::uninit();
    let tcp_ptr = tcp.as_mut_ptr();
    unsafe {
      uv_tcp_init(uv_loop, tcp_ptr);
      (*tcp_ptr).data = closed_ptr as *mut c_void;
      uv_close(tcp_ptr as *mut uv_handle_t, Some(close_cb));
    }

    tick(runtime);
    assert!(closed.get());

    unsafe {
      Rc::from_raw(closed_ptr);
    }
  })
  .await;
}

// ========== TCP getsockname / getpeername edge cases ==========

#[tokio::test(flavor = "current_thread")]
async fn tcp_getsockname_no_bind_returns_einval() {
  run_test(async |_runtime, uv_loop| {
    let mut tcp = std::mem::MaybeUninit::<uv_tcp_t>::uninit();
    let tcp_ptr = tcp.as_mut_ptr();
    unsafe {
      uv_tcp_init(uv_loop, tcp_ptr);
      let mut name = std::mem::MaybeUninit::<sockaddr_in>::zeroed();
      let mut namelen = std::mem::size_of::<sockaddr_in>() as i32;
      let status = uv_tcp_getsockname(
        tcp_ptr,
        name.as_mut_ptr() as *mut c_void,
        &mut namelen,
      );
      assert_eq!(status, UV_EINVAL);
    }
  })
  .await;
}

#[tokio::test(flavor = "current_thread")]
async fn tcp_getpeername_no_stream_returns_enotconn() {
  run_test(async |_runtime, uv_loop| {
    let mut tcp = std::mem::MaybeUninit::<uv_tcp_t>::uninit();
    let tcp_ptr = tcp.as_mut_ptr();
    unsafe {
      uv_tcp_init(uv_loop, tcp_ptr);
      let mut name = std::mem::MaybeUninit::<sockaddr_in>::zeroed();
      let mut namelen = std::mem::size_of::<sockaddr_in>() as i32;
      let status = uv_tcp_getpeername(
        tcp_ptr,
        name.as_mut_ptr() as *mut c_void,
        &mut namelen,
      );
      assert_eq!(status, UV_ENOTCONN);
    }
  })
  .await;
}

// ========== TCP shutdown drains write queue first ==========

#[tokio::test(flavor = "current_thread")]
async fn tcp_shutdown_waits_for_write_queue_to_drain() {
  run_test(async |runtime, uv_loop| {
    use std::cell::RefCell;

    // Track ordering of callbacks.
    let order = Rc::new(RefCell::new(Vec::<&'static str>::new()));

    // --- Server: bind + listen ---
    let mut server = std::mem::MaybeUninit::<uv_tcp_t>::uninit();
    let server_ptr = server.as_mut_ptr();

    unsafe extern "C" fn on_connection(_: *mut uv_stream_t, _: i32) {}

    let server_port: u16;
    unsafe {
      uv_tcp_init(uv_loop, server_ptr);

      let mut addr = std::mem::MaybeUninit::<sockaddr_in>::uninit();
      let ip = std::ffi::CString::new("127.0.0.1").unwrap();
      uv_ip4_addr(ip.as_ptr(), 0, addr.as_mut_ptr());

      assert_ok(uv_tcp_bind(
        server_ptr,
        addr.as_ptr() as *const c_void,
        0,
        0,
      ));

      assert_ok(uv_listen(
        server_ptr as *mut uv_stream_t,
        128,
        Some(on_connection),
      ));

      let mut name = std::mem::MaybeUninit::<sockaddr_in>::zeroed();
      let mut namelen = std::mem::size_of::<sockaddr_in>() as i32;
      uv_tcp_getsockname(
        server_ptr,
        name.as_mut_ptr() as *mut c_void,
        &mut namelen,
      );
      server_port = u16::from_be(name.assume_init_ref().sin_port);
    }

    // --- Client: connect ---
    let connected = Rc::new(Cell::new(false));
    let connected_ptr = Rc::into_raw(connected.clone());

    unsafe extern "C" fn on_connect(req: *mut uv_connect_t, status: i32) {
      assert_eq!(status, 0);
      let connected = unsafe { Rc::from_raw((*req).data as *const Cell<bool>) };
      connected.set(true);
      let _ = Rc::into_raw(connected);
    }

    let mut client = std::mem::MaybeUninit::<uv_tcp_t>::uninit();
    let client_ptr = client.as_mut_ptr();
    let mut connect_req = std::mem::MaybeUninit::<uv_connect_t>::uninit();
    let connect_req_ptr = connect_req.as_mut_ptr();

    unsafe {
      uv_tcp_init(uv_loop, client_ptr);
      (*connect_req_ptr).data = connected_ptr as *mut c_void;

      let mut addr = std::mem::MaybeUninit::<sockaddr_in>::uninit();
      let ip = std::ffi::CString::new("127.0.0.1").unwrap();
      uv_ip4_addr(ip.as_ptr(), server_port as i32, addr.as_mut_ptr());

      assert_ok(uv_tcp_connect(
        connect_req_ptr,
        client_ptr,
        addr.as_ptr() as *const c_void,
        Some(on_connect),
      ));
    }

    // Poll until connected.
    for _ in 0..100 {
      tick(runtime);
      if connected.get() {
        break;
      }
      tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
    assert!(connected.get(), "Client should have connected");

    // --- Accept on server side ---
    let mut accepted = std::mem::MaybeUninit::<uv_tcp_t>::uninit();
    let accepted_ptr = accepted.as_mut_ptr();
    unsafe {
      uv_tcp_init(uv_loop, accepted_ptr);
    }
    tick(runtime);
    unsafe {
      assert_ok(uv_accept(
        server_ptr as *mut uv_stream_t,
        accepted_ptr as *mut uv_stream_t,
      ));
    }

    // Start reading on accepted socket so the client's writes can drain.
    unsafe extern "C" fn alloc_cb(
      _: *mut uv_handle_t,
      size: usize,
      buf: *mut uv_buf_t,
    ) {
      let mut v = Vec::<u8>::with_capacity(size);
      unsafe {
        (*buf).base = v.as_mut_ptr().cast();
        (*buf).len = size;
      }
      std::mem::forget(v);
    }
    unsafe extern "C" fn drain_read_cb(
      _: *mut uv_stream_t,
      _nread: isize,
      buf: *const uv_buf_t,
    ) {
      unsafe {
        if !(*buf).base.is_null() && (*buf).len > 0 {
          drop(Vec::<u8>::from_raw_parts((*buf).base.cast(), 0, (*buf).len));
        }
      }
    }
    unsafe {
      uv_read_start(
        accepted_ptr as *mut uv_stream_t,
        Some(alloc_cb),
        Some(drain_read_cb),
      );
    }

    // --- Write large buffer + immediate shutdown ---
    let order_write = Rc::into_raw(order.clone());
    let order_shutdown = Rc::into_raw(order.clone());

    unsafe extern "C" fn write_cb(req: *mut uv_write_t, status: i32) {
      assert_eq!(status, 0);
      let order = unsafe {
        Rc::from_raw((*req).data as *const RefCell<Vec<&'static str>>)
      };
      order.borrow_mut().push("write");
      let _ = Rc::into_raw(order);
    }

    unsafe extern "C" fn shutdown_cb(req: *mut uv_shutdown_t, _status: i32) {
      let order = unsafe {
        Rc::from_raw((*req).data as *const RefCell<Vec<&'static str>>)
      };
      order.borrow_mut().push("shutdown");
      let _ = Rc::into_raw(order);
    }

    // 2 MB – large enough to exceed kernel TCP buffers, ensuring
    // the write is partially queued when uv_shutdown is called.
    let write_data = vec![0x42u8; 2 * 1024 * 1024];
    let mut write_req = std::mem::MaybeUninit::<uv_write_t>::uninit();
    let write_req_ptr = write_req.as_mut_ptr();
    let mut shutdown_req = std::mem::MaybeUninit::<uv_shutdown_t>::uninit();
    let shutdown_req_ptr = shutdown_req.as_mut_ptr();

    unsafe {
      (*write_req_ptr).data = order_write as *mut c_void;
      (*shutdown_req_ptr).data = order_shutdown as *mut c_void;

      let buf = uv_buf_t {
        base: write_data.as_ptr() as *mut _,
        len: write_data.len(),
      };

      assert_ok(uv_write(
        write_req_ptr,
        client_ptr as *mut uv_stream_t,
        &buf,
        1,
        Some(write_cb),
      ));

      // Shutdown while writes are (likely) still queued.
      assert_ok(uv_shutdown(
        shutdown_req_ptr,
        client_ptr as *mut uv_stream_t,
        Some(shutdown_cb),
      ));
    }

    // Tick until both callbacks fire.
    for _ in 0..2000 {
      tick(runtime);
      if order.borrow().len() >= 2 {
        break;
      }
      tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }

    let phases = order.borrow();
    assert!(
      phases.len() >= 2,
      "Expected both write and shutdown callbacks, got {:?}",
      *phases
    );

    let write_idx = phases
      .iter()
      .position(|&s| s == "write")
      .expect("write cb should have fired");
    let shutdown_idx = phases
      .iter()
      .position(|&s| s == "shutdown")
      .expect("shutdown cb should have fired");
    assert!(
      write_idx < shutdown_idx,
      "write should complete before shutdown: {:?}",
      *phases
    );
    drop(phases);

    // Clean up.
    unsafe {
      uv_close(client_ptr as *mut uv_handle_t, None);
      uv_close(server_ptr as *mut uv_handle_t, None);
      uv_close(accepted_ptr as *mut uv_handle_t, None);
      Rc::from_raw(connected_ptr);
      Rc::from_raw(order_write);
      Rc::from_raw(order_shutdown);
    }
    tick(runtime);
  })
  .await;
}

// ========== TCP shutdown without stream ==========

#[tokio::test(flavor = "current_thread")]
async fn tcp_shutdown_no_stream() {
  run_test(async |_runtime, uv_loop| {
    let mut tcp = std::mem::MaybeUninit::<uv_tcp_t>::uninit();
    let tcp_ptr = tcp.as_mut_ptr();
    let mut req = std::mem::MaybeUninit::<uv_shutdown_t>::uninit();
    let req_ptr = req.as_mut_ptr();
    unsafe {
      uv_tcp_init(uv_loop, tcp_ptr);
      // uv_shutdown returns UV_ENOTCONN when no stream is attached
      // (matching libuv which returns error code, not callback).
      let status = uv_shutdown(req_ptr, tcp_ptr as *mut uv_stream_t, None);
      assert_eq!(status, UV_ENOTCONN);
    }
  })
  .await;
}
