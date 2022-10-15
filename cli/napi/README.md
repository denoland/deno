# napi

This directory contains source for Deno's Node-API implementation. It depends on
`napi_sym` and `deno_napi`.

- [`async.rs`](./async.rs) - Asyncronous work related functions.
- [`env.rs`](./env.rs) - Enviornment related functions.
- [`js_native_api.rs`](./js_native_api.rs) - V8/JS related functions.
- [`thread_safe_function.rs`](./threadsafe_functions.rs) - Thread safe function
  related functions.

## Adding a new function

Add the symbol name to
[`cli/napi_sym/symbol_exports.json`](../napi_sym/symbol_exports.json).

```diff
{
  "symbols": [
    ...
    "napi_get_undefined",
-   "napi_get_null"
+   "napi_get_null",
+   "napi_get_boolean"
  ]
}
```

Determine where to place the implementation. `napi_get_boolean` is related to JS
values so we will place it in `js_native_api.rs`. If something is not clear,
just create a new file module.

See [`napi_sym`](../napi_sym/) for writing the implementation:

```rust
#[napi_sym::napi_sym]
pub fn napi_get_boolean(
  env: *mut Env,
  value: bool,
  result: *mut napi_value,
) -> Result {
  // ...
  Ok(())
}
```

Update the generated symbol lists using the script:

```
deno run --allow-write tools/napi/generate_symbols_lists.js
```

Add a test in [`/test_napi`](../../test_napi/). You can also refer to Node.js
test suite for Node-API.

```js
// test_napi/boolean_test.js
import { assertEquals, loadTestLibrary } from "./common.js";
const lib = loadTestLibrary();
Deno.test("napi get boolean", function () {
  assertEquals(lib.test_get_boolean(true), true);
  assertEquals(lib.test_get_boolean(false), false);
});
```

```rust
// test_napi/src/boolean.rs

use napi_sys::Status::napi_ok;
use napi_sys::ValueType::napi_boolean;
use napi_sys::*;

extern "C" fn test_boolean(
  env: napi_env,
  info: napi_callback_info,
) -> napi_value {
  let (args, argc, _) = crate::get_callback_info!(env, info, 1);
  assert_eq!(argc, 1);

  let mut ty = -1;
  assert!(unsafe { napi_typeof(env, args[0], &mut ty) } == napi_ok);
  assert_eq!(ty, napi_boolean);

  // Use napi_get_boolean here...

  value
}

pub fn init(env: napi_env, exports: napi_value) {
  let properties = &[crate::new_property!(env, "test_boolean\0", test_boolean)];

  unsafe {
    napi_define_properties(env, exports, properties.len(), properties.as_ptr())
  };
}
```

```diff
// test_napi/src/lib.rs

+ mod boolean;

...

#[no_mangle]
unsafe extern "C" fn napi_register_module_v1(
  env: napi_env,
  exports: napi_value,
) -> napi_value {
  ...
+ boolean::init(env, exports);

  exports
}
```

Run the test using `cargo test -p test_napi`.
