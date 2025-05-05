use std::borrow::Cow;
use std::cell::RefCell;

use deno_core::futures::channel::oneshot;
use deno_core::futures::FutureExt;
use deno_core::op2;
use deno_core::v8;
use deno_core::v8::Handle;
use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::Resource;
use deno_core::ResourceId;
use std::rc::Rc;
use tokio::io::AsyncRead;

pub struct JsStreamInner {
  onread: v8::Global<v8::Function>,
  onwrite: v8::Global<v8::Function>,
  op_state: Rc<RefCell<OpState>>,
}

pub struct JsStream {
  rid: ResourceId,
}

impl GarbageCollected for JsStream {}

#[op2]
impl JsStream {
  #[constructor]
  #[cppgc]
  pub fn new(
    op_state: Rc<RefCell<OpState>>,
    scope: &mut v8::HandleScope,
    stream: v8::Local<v8::Object>,
  ) -> JsStream {
    let onread_str = v8::String::new(scope, "onread").unwrap();
    let onread = stream.get(scope, onread_str.into()).unwrap();
    let onread_func = v8::Local::<v8::Function>::try_from(onread).unwrap();
    let v8_global_onread = v8::Global::new(scope, onread_func);

    let onwrite_str = v8::String::new(scope, "onwrite").unwrap();
    let onwrite = stream.get(scope, onwrite_str.into()).unwrap();
    let onwrite_func = v8::Local::<v8::Function>::try_from(onwrite).unwrap();
    let v8_global_onwrite = v8::Global::new(scope, onwrite_func);

    let stream = JsStreamInner {
      onread: v8_global_onread,
      onwrite: v8_global_onwrite,
      op_state: op_state.clone(),
    };
    let rid = op_state.borrow_mut().resource_table.add(stream);
    JsStream { rid }
  }

  #[fast]
  #[symbol("kStreamBaseField")]
  #[smi]
  fn get(&self) -> ResourceId {
    self.rid
  }

  #[fast]
  #[symbol("timeout")]
  #[smi]
  fn timeout(&self) -> i32 {
    10
  }
}

impl Resource for JsStreamInner {
  fn name(&self) -> Cow<str> {
    "js_stream".into()
  }

  fn write(
    self: Rc<Self>,
    buf: deno_core::BufView,
  ) -> deno_core::AsyncResult<deno_core::WriteOutcome> {
    let op_state = self.op_state.clone();
    let onwrite = self.onwrite.clone();
    let (tx, rx) = oneshot::channel();
    async move {
      op_state
        .borrow()
        .borrow::<deno_core::V8TaskSpawner>()
        .spawn(move |scope| {
          let func = onwrite.open(scope);
          let recv = v8::undefined(scope);

          let bs = v8::ArrayBuffer::new_backing_store_from_boxed_slice(
            buf.to_vec().into_boxed_slice(),
          );
          let ab =
            v8::ArrayBuffer::with_backing_store(scope, &bs.make_shared());
          let args = [ab.into()];
          let value = func.call(scope, recv.into(), &args).unwrap();
          let nwritten = v8::Local::<v8::Number>::try_from(value).unwrap();
          let _ = tx.send(deno_core::WriteOutcome::Full {
            nwritten: nwritten.value() as usize,
          });
        });
      match rx.await {
        Ok(buf) => Ok(buf),
        Err(_) => panic!("Failed to read from stream"),
      }
    }
    .boxed_local()
  }

  fn read(
    self: std::rc::Rc<Self>,
    limit: usize,
  ) -> deno_core::AsyncResult<deno_core::BufView> {
    let op_state = self.op_state.clone();
    let onread = self.onread.clone();
    let (tx, rx) = oneshot::channel();
    async move {
      op_state
        .borrow()
        .borrow::<deno_core::V8TaskSpawner>()
        .spawn(move |scope| {
          let func = onread.open(scope);
          let recv = v8::undefined(scope);
          let args = [v8::Number::new(scope, limit as f64).into()];
          let value = func.call(scope, recv.into(), &args).unwrap();
          let buf = v8::Local::<v8::ArrayBuffer>::try_from(value).unwrap();
          let data = unsafe {
            std::slice::from_raw_parts(
              buf.data().unwrap().as_ptr() as *const u8,
              buf.byte_length(),
            )
          };
          let _ = tx.send(data.to_vec().into());
        });
      match rx.await {
        Ok(buf) => Ok(buf),
        Err(_) => panic!("Failed to read from stream"),
      }
    }
    .boxed_local()
  }
}
