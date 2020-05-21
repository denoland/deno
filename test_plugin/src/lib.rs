use deno_core::plugin_api::Buf;
use deno_core::plugin_api::Interface;
use deno_core::plugin_api::Op;
use deno_core::plugin_api::ZeroCopyBuf;
use futures::future::FutureExt;

#[no_mangle]
pub fn deno_plugin_init(interface: &mut dyn Interface) {
  interface.register_op("testSync", op_test_sync);
  interface.register_op("testAsync", op_test_async);
  interface.register_op("testResources", op_test_resources);
}

fn op_test_sync(
  _interface: &mut dyn Interface,
  data: &[u8],
  zero_copy: Option<ZeroCopyBuf>,
) -> Op {
  if let Some(buf) = zero_copy {
    let data_str = std::str::from_utf8(&data[..]).unwrap();
    let buf_str = std::str::from_utf8(&buf[..]).unwrap();
    println!(
      "Hello from plugin. data: {} | zero_copy: {}",
      data_str, buf_str
    );
  }
  let result = b"test";
  let result_box: Buf = Box::new(*result);
  Op::Sync(result_box)
}

fn op_test_async(
  _interface: &mut dyn Interface,
  data: &[u8],
  zero_copy: Option<ZeroCopyBuf>,
) -> Op {
  let data_str = std::str::from_utf8(&data[..]).unwrap().to_string();
  let fut = async move {
    if let Some(buf) = zero_copy {
      let buf_str = std::str::from_utf8(&buf[..]).unwrap();
      println!(
        "Hello from plugin. data: {} | zero_copy: {}",
        data_str, buf_str
      );
    }
    let (tx, rx) = futures::channel::oneshot::channel::<Result<(), ()>>();
    std::thread::spawn(move || {
      std::thread::sleep(std::time::Duration::from_secs(1));
      tx.send(Ok(())).unwrap();
    });
    assert!(rx.await.is_ok());
    let result = b"test";
    let result_box: Buf = Box::new(*result);
    result_box
  };

  Op::Async(fut.boxed())
}

struct TestResource {
  noise: String,
}

fn op_test_resources(
  interface: &mut dyn Interface,
  _data: &[u8],
  _zero_copy: Option<ZeroCopyBuf>,
) -> Op {
  let rid = {
    // `add()`
    let rc = Box::new(TestResource {
      noise: "woof".to_owned(),
    });
    interface.resource_table().add("test_resource", rc)
  };
  {
    // `has()`
    let found = interface.resource_table().has(rid);
    assert!(found);
  }
  {
    // `get()`
    let rc = interface.resource_table().get(rid).unwrap();
    let rc = rc.downcast_ref::<TestResource>().unwrap();
    assert_eq!(&rc.noise, "woof");
  }
  {
    // `get_mut()`
    let rc = interface.resource_table().get_mut(rid).unwrap();
    let mut rc = rc.downcast_mut::<TestResource>().unwrap();
    assert_eq!(&rc.noise, "woof");
    rc.noise = "mooh".to_owned();
  }
  {
    // The resource's internal state should have changed.
    let rc = interface.resource_table().get(rid).unwrap();
    let rc = rc.downcast_ref::<TestResource>().unwrap();
    assert_eq!(&rc.noise, "mooh");
  }
  {
    // `close()`
    let found = interface.resource_table().close(rid).is_some();
    assert!(found);
  }
  {
    // After `close()` the resource should be gone.
    let found1 = interface.resource_table().has(rid);
    assert!(!found1);
    let found2 = interface.resource_table().close(rid).is_some();
    assert!(!found2);
  }
  Op::Sync(Default::default())
}
