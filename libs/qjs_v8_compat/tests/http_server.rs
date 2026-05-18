// Copyright 2018-2026 the Deno authors. MIT license.
//
// HTTP server backed by QuickJS-ng.
//
// This is the smallest "an HTTP server runs on QuickJS" demonstration. We
// spin up a real Hyper server on a random port, where every request handler
// is a JavaScript function evaluated in QuickJS. The test then sends real
// HTTP requests over loopback and asserts that the responses match what
// the JS handler computed.
//
// Why this matters: it's the proof that JavaScript-on-QuickJS can drive a
// production-shaped I/O surface (tokio + Hyper + HTTP/1.1). Each piece of
// what `Deno.serve` does — accept loop, per-request handler invocation,
// JS->bytes response marshaling — is exercised end-to-end. The pieces
// missing for `Deno.serve` proper (deno_core integration, op2 macro,
// `ext/http`'s richer Request/Response types) are documented in the PR
// description; this demo is the floor those will be built on top of.
//
// # Threading model
//
// QuickJS-ng `JSRuntime` is not `Send` and not thread-safe. We dedicate a
// single OS thread to QuickJS and message-pass requests to it via an
// mpsc channel. Hyper runs on tokio's multi-threaded scheduler;
// per-request, the handler awaits a oneshot reply from the JS thread.
// This is the simplest working topology — production code would either
// pin tokio to a LocalSet or pool runtimes per worker.

#![cfg(feature = "link_quickjs")]

use std::ffi::CString;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::Request;
use hyper::Response;
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use qjs_v8_compat::ffi;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

// ---- the JS thread ------------------------------------------------------
//
// Owns a JSRuntime + JSContext for the lifetime of the test, evaluates a
// handler script once, then for each request invokes the global `handle`
// function with `(method, path)` and turns its return value into a
// `String` body.

struct ReqMsg {
  method: String,
  path: String,
  reply: oneshot::Sender<String>,
}

fn spawn_js_thread(
  handler_src: String,
) -> (mpsc::UnboundedSender<ReqMsg>, Arc<AtomicBool>) {
  let (tx, mut rx) = mpsc::unbounded_channel::<ReqMsg>();
  let ready = Arc::new(AtomicBool::new(false));
  let ready_for_thread = Arc::clone(&ready);

  thread::spawn(move || {
    let handler_c = CString::new(handler_src).unwrap();
    let fname_c = CString::new("<handler>").unwrap();
    let handle_name = CString::new("handle").unwrap();

    unsafe {
      let rt = ffi::JS_NewRuntime();
      let ctx = ffi::JS_NewContext(rt);

      // Evaluate the handler script. It must define `globalThis.handle`.
      let r = ffi::JS_Eval(
        ctx,
        handler_c.as_ptr(),
        handler_c.as_bytes().len(),
        fname_c.as_ptr(),
        ffi::JS_EVAL_TYPE_GLOBAL,
      );
      assert_ne!(r.tag, ffi::JS_TAG_EXCEPTION, "handler script threw on load");
      ffi::JS_FreeValue(ctx, r);

      let global = ffi::JS_GetGlobalObject(ctx);
      let handle = ffi::JS_GetPropertyStr(ctx, global, handle_name.as_ptr());
      assert_eq!(
        ffi::JS_IsFunction(ctx, handle),
        1,
        "handler script did not define `handle` as a function"
      );

      ready_for_thread.store(true, Ordering::SeqCst);

      // Block on the channel using a small parker. We can't use tokio
      // here because QuickJS isn't Send; instead, walk the channel
      // synchronously by busy-blocking with try_recv + a short sleep.
      // The mpsc UnboundedReceiver doesn't have a sync blocking_recv
      // by default, but it does — use it.
      while let Some(msg) = rx.blocking_recv() {
        let method_js = ffi::JS_NewStringLen(
          ctx,
          msg.method.as_ptr() as *const _,
          msg.method.len(),
        );
        let path_js = ffi::JS_NewStringLen(
          ctx,
          msg.path.as_ptr() as *const _,
          msg.path.len(),
        );
        let mut args = [method_js, path_js];
        let result = ffi::JS_Call(
          ctx,
          handle,
          global,
          args.len() as i32,
          args.as_mut_ptr(),
        );
        ffi::JS_FreeValue(ctx, method_js);
        ffi::JS_FreeValue(ctx, path_js);

        let body = if result.tag == ffi::JS_TAG_EXCEPTION {
          let exc = ffi::JS_GetException(ctx);
          let s = read_string(ctx, exc);
          ffi::JS_FreeValue(ctx, exc);
          format!("handler threw: {s}")
        } else {
          let s = read_string(ctx, result);
          ffi::JS_FreeValue(ctx, result);
          s
        };

        let _ = msg.reply.send(body);
      }

      ffi::JS_FreeValue(ctx, handle);
      ffi::JS_FreeValue(ctx, global);
      ffi::JS_FreeContext(ctx);
      ffi::JS_FreeRuntime(rt);
    }
  });

  // Spin until the JS thread has finished evaluating the handler script;
  // otherwise the test could hit the server before the handler exists.
  while !ready.load(Ordering::SeqCst) {
    thread::sleep(Duration::from_millis(1));
  }

  (tx, ready)
}

unsafe fn read_string(ctx: *mut ffi::JSContext, v: ffi::JSValue) -> String {
  unsafe {
    let p = ffi::JS_ToCString(ctx, v);
    if p.is_null() {
      return String::new();
    }
    let s = std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned();
    ffi::JS_FreeCString(ctx, p);
    s
  }
}

// ---- the HTTP server ----------------------------------------------------

async fn handle_request(
  tx: mpsc::UnboundedSender<ReqMsg>,
  req: Request<Incoming>,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
  let (parts, _body) = req.into_parts();
  let method = parts.method.to_string();
  let path = parts.uri.path().to_string();

  let (reply_tx, reply_rx) = oneshot::channel::<String>();
  let _ = tx.send(ReqMsg {
    method,
    path,
    reply: reply_tx,
  });
  let body = reply_rx
    .await
    .unwrap_or_else(|_| "<channel closed>".to_string());

  Ok(Response::new(Full::new(Bytes::from(body))))
}

// ---- the integration test ----------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn http_server_responses_come_from_quickjs() {
  let handler = r#"
    globalThis.handle = function (method, path) {
      if (path === '/hello') return 'hello, world from quickjs!';
      if (path === '/echo') return 'method=' + method;
      if (path.startsWith('/sum/')) {
        const parts = path.slice(5).split(',').map(Number);
        return 'sum=' + parts.reduce((a, b) => a + b, 0);
      }
      return 'unknown path: ' + path;
    };
  "#
  .to_string();

  let (tx, _ready) = spawn_js_thread(handler);

  // Bind to an ephemeral port and read back the actual address.
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let addr: SocketAddr = listener.local_addr().unwrap();

  let server_tx = tx.clone();
  let server = tokio::spawn(async move {
    loop {
      let (stream, _) = match listener.accept().await {
        Ok(x) => x,
        Err(_) => break,
      };
      let io = TokioIo::new(stream);
      let req_tx = server_tx.clone();
      tokio::spawn(async move {
        let _ = hyper::server::conn::http1::Builder::new()
          .serve_connection(
            io,
            service_fn(move |req| handle_request(req_tx.clone(), req)),
          )
          .await;
      });
    }
  });

  let client = hyper_util::client::legacy::Client::builder(
    hyper_util::rt::TokioExecutor::new(),
  )
  .build_http::<Full<Bytes>>();

  let cases = [
    ("/hello", "hello, world from quickjs!"),
    ("/echo", "method=GET"),
    ("/sum/1,2,3,4,5", "sum=15"),
    ("/something-else", "unknown path: /something-else"),
  ];

  for (path, expected) in cases {
    let url: hyper::Uri = format!("http://{addr}{path}").parse().unwrap();
    let resp = client.get(url).await.expect("client get failed");
    assert_eq!(resp.status(), 200, "non-200 for {path}");
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let body_str = std::str::from_utf8(&body).unwrap();
    assert_eq!(body_str, expected, "wrong body for {path}");
  }

  server.abort();
  drop(tx); // close the channel; JS thread will exit its loop.
}
