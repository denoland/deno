use deno_core::plugin_api::DispatchOpFn;
use deno_core::plugin_api::Interface;
use deno_core::plugin_api::Op;
use deno_core::plugin_api::ZeroCopyBuf;
use futures::future::FutureExt;

#[no_mangle]
pub fn deno_plugin_init(interface: &mut dyn Interface) {
  interface.register_op("testSync", Box::new(op_test_sync));
  interface.register_op("testAsync", Box::new(op_test_async));
  interface.register_op("testWrapped", wrap_op(op_wrapped));
}

fn op_test_sync(
  _interface: &mut dyn Interface,
  zero_copy: &mut [ZeroCopyBuf],
) -> Op {
  if !zero_copy.is_empty() {
    println!("Hello from plugin.");
  }
  let zero_copy = zero_copy.to_vec();
  for (idx, buf) in zero_copy.iter().enumerate() {
    let buf_str = std::str::from_utf8(&buf[..]).unwrap();
    println!("zero_copy[{}]: {}", idx, buf_str);
  }
  let result = b"test";
  let result_box: Box<[u8]> = Box::new(*result);
  Op::Sync(result_box)
}

fn op_test_async(
  _interface: &mut dyn Interface,
  zero_copy: &mut [ZeroCopyBuf],
) -> Op {
  if !zero_copy.is_empty() {
    println!("Hello from plugin.");
  }
  let zero_copy = zero_copy.to_vec();
  let fut = async move {
    for (idx, buf) in zero_copy.iter().enumerate() {
      let buf_str = std::str::from_utf8(&buf[..]).unwrap();
      println!("zero_copy[{}]: {}", idx, buf_str);
    }
    let (tx, rx) = futures::channel::oneshot::channel::<Result<(), ()>>();
    std::thread::spawn(move || {
      std::thread::sleep(std::time::Duration::from_secs(1));
      tx.send(Ok(())).unwrap();
    });
    assert!(rx.await.is_ok());
    let result = b"test";
    let result_box: Box<[u8]> = Box::new(*result);
    result_box
  };

  Op::Async(fut.boxed())
}

fn wrap_op<D>(d: D) -> Box<DispatchOpFn>
where
  D: Fn(&mut dyn Interface, String, &mut [ZeroCopyBuf]) -> Op + 'static,
{
  Box::new(
    move |i: &mut dyn Interface, zero_copy: &mut [ZeroCopyBuf]| {
      let first_buf_str = std::str::from_utf8(&zero_copy[0][..]).unwrap();
      d(i, first_buf_str.to_string(), &mut zero_copy[1..])
    },
  )
}

fn op_wrapped(
  _interface: &mut dyn Interface,
  first_buf_str: String,
  zero_copy: &mut [ZeroCopyBuf],
) -> Op {
  if !zero_copy.is_empty() {
    println!("Hello from wrapped op.");
  }
  let zero_copy = zero_copy.to_vec();
  let fut = async move {
    println!("first_buf: {}", first_buf_str);
    for (idx, buf) in zero_copy.iter().enumerate() {
      let buf_str = std::str::from_utf8(&buf[..]).unwrap();
      println!("zero_copy[{}]: {}", idx, buf_str);
    }
    let (tx, rx) = futures::channel::oneshot::channel::<Result<(), ()>>();
    std::thread::spawn(move || {
      std::thread::sleep(std::time::Duration::from_secs(1));
      tx.send(Ok(())).unwrap();
    });
    assert!(rx.await.is_ok());
    let result = b"test";
    let result_box: Box<[u8]> = Box::new(*result);
    result_box
  };

  Op::Async(fut.boxed())
}
