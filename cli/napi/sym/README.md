# napi_sym

A proc_macro for Deno's Node-API implementation. It does the following things:

- Marks the symbol as `#[no_mangle]` and rewrites it as `pub extern "C" $name`.
- Asserts that the function symbol is present in
  [`symbol_exports.json`](./symbol_exports.json).
- Maps `deno_napi::Result` to raw `napi_result`.

```rust
use deno_napi::{napi_value, Env, Error, Result};

#[napi_sym::napi_sym]
fn napi_get_boolean(
  env: *mut Env,
  value: bool,
  result: *mut napi_value,
) -> Result {
  let _env: &mut Env = env.as_mut().ok_or(Error::InvalidArg)?;
  // *result = ...
  Ok(())
}
```

### `symbol_exports.json`

A file containing the symbols that need to be put into the executable's dynamic
symbol table at link-time.

This is done using `/DEF:` on Windows, `-exported_symbol,_` on macOS and
`--export-dynamic-symbol=` on Linux. See [`cli/build.rs`](../build.rs).

On Windows, you need to generate the `.def` file by running
[`tools/napi/generate_symbols_lists.js`](../../tools/napi/generate_symbols_lists.js).
