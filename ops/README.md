# deno_ops

`proc_macro` for generating highly optimized V8 functions from Deno ops.

```rust
// Declare an op.
#[op]
pub fn op_add(_: &mut OpState, a: i32, b: i32) -> Result<i32, AnyError> {
  Ok(a + b)
}

// Register with an extension.
Extension::builder()
  .ops(vec![op_add::decl()])
  .build();
```

## Peformance

The macro can optimize away code, short circuit fast paths and generate a Fast
API impl.

Cases where code is optimized away:

- `-> ()` skips serde_v8 and `rv.set` calls.
- `-> Result<(), E>` skips serde_v8 and `rv.set` calls for `Ok()` branch.
- `-> ResourceId` or `-> [int]` types will use specialized method like
  `v8::ReturnValue::set_uint32`. A fast path for SMI.
- `-> Result<ResourceId, E>` or `-> Result<[int], E>` types will be optimized
  like above for the `Ok()` branch.

### fast API calls

```rust
impl fast_api::FastFunction for #name {
  type Signature = ();
  fn function(&self) -> Self::Signature {}

  fn raw(&self) -> *const std::ffi::c_void {
    #raw_block
  }
  fn args(&self) -> &'static [fast_api::Type] {
    &[ #args ]
  }
  fn return_type(&self) -> fast_api::CType {
    #ret
  }
}
```
