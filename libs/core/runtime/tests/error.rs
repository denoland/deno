// Copyright 2018-2026 the Deno authors. MIT license.

use std::future::poll_fn;
use std::task::Poll;

use deno_error::JsErrorBox;

use crate::JsRuntime;
use crate::RuntimeOptions;
use crate::op2;

#[tokio::test]
async fn test_error_builder() {
  #[op2(fast)]
  fn op_err() -> Result<(), JsErrorBox> {
    Err(JsErrorBox::new("DOMExceptionOperationError", "abc"))
  }

  deno_core::extension!(test_ext, ops = [op_err]);
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: vec![test_ext::init()],
    ..Default::default()
  });
  poll_fn(move |cx| {
    runtime
      .execute_script(
        "error_builder_test.js",
        include_str!("error_builder_test.js"),
      )
      .unwrap();
    if let Poll::Ready(Err(_)) = runtime.poll_event_loop(cx, Default::default())
    {
      unreachable!();
    }
    Poll::Ready(())
  })
  .await;
}

#[test]
fn syntax_error() {
  let mut runtime = JsRuntime::new(Default::default());
  let src = "hocuspocus(";
  let js_error = runtime.execute_script("i.js", src).unwrap_err();
  let frame = js_error.frames.first().unwrap();
  assert_eq!(frame.column_number, Some(12));
}

// `throw_js_error_class` is what the op2 fast-call error path uses. Unlike the
// slow path it cannot re-enter JS to call `buildCustomError`, so it rebuilds
// the exception natively. This verifies that the native rebuild still restores
// the registered error class (`instanceof`), name, message, the `code`
// additional property, and the round-trip key symbol.
#[test]
fn fast_call_error_preserves_class() {
  use std::borrow::Cow;

  use deno_error::JsErrorClass;
  use deno_error::PropertyValue;

  #[derive(Debug)]
  struct CustomError;

  impl std::fmt::Display for CustomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      write!(f, "custom message")
    }
  }

  impl std::error::Error for CustomError {}

  impl JsErrorClass for CustomError {
    fn get_class(&self) -> Cow<'static, str> {
      Cow::Borrowed("MyError")
    }

    fn get_message(&self) -> Cow<'static, str> {
      Cow::Borrowed("custom message")
    }

    fn get_additional_properties(&self) -> deno_error::AdditionalProperties {
      Box::new(std::iter::once((
        Cow::Borrowed("code"),
        PropertyValue::String(Cow::Borrowed("E_CUSTOM")),
      )))
    }

    fn get_ref(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
      self
    }
  }

  let mut runtime = JsRuntime::new(Default::default());
  runtime
    .execute_script(
      "register.js",
      r#"
      globalThis.MyError = class MyError extends Error {
        constructor(msg) {
          super(msg);
          this.name = "MyError";
        }
      };
      Deno.core.registerErrorClass("MyError", globalThis.MyError);
      "#,
    )
    .unwrap();

  // Build + throw the exception through the native path, then stash it on the
  // global so JS can inspect it.
  {
    deno_core::scope!(scope, runtime);
    let exception = {
      v8::tc_scope!(let tc_scope, scope);
      crate::error::throw_js_error_class(tc_scope, &CustomError);
      let exception = tc_scope
        .exception()
        .expect("throw_js_error_class should set an exception");
      v8::Global::new(tc_scope, exception)
    };
    let exception = v8::Local::new(scope, &exception);
    let context = scope.get_current_context();
    let global = context.global(scope);
    let key = v8::String::new(scope, "thrownError").unwrap();
    global.set(scope, key.into(), exception);
  }

  // Assertions throw on mismatch, failing `execute_script`.
  runtime
    .execute_script(
      "assert.js",
      r#"
      {
        const e = globalThis.thrownError;
        if (!(e instanceof globalThis.MyError)) {
          throw new Error("expected instanceof MyError, got " + e.name);
        }
        if (e.name !== "MyError") {
          throw new Error("expected name 'MyError', got " + e.name);
        }
        if (e.message !== "custom message") {
          throw new Error("expected message 'custom message', got " + e.message);
        }
        if (e.code !== "E_CUSTOM") {
          throw new Error("expected code 'E_CUSTOM', got " + e.code);
        }
        const keys = e[Symbol.for("errorAdditionalPropertyKeys")];
        if (!keys || keys.length !== 1 || keys[0] !== "code") {
          throw new Error("expected key ['code'], got " + keys);
        }
      }
      "#,
    )
    .unwrap();
}
