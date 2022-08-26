# deno_ops

`proc_macro` for generating highly optimized V8 functions from Deno ops.

```rust
// Declare an op.
#[op(fast)]
pub fn op_add(_: &mut OpState, a: i32, b: i32) -> i32 {
  a + b
}

// Register with an extension.
Extension::builder()
  .ops(vec![op_add::decl()])
  .build();
```

## Performance

The macro can optimize away code, short circuit fast paths and generate a Fast
API impl.

Cases where code is optimized away:

- `-> ()` skips serde_v8 and `rv.set` calls.
- `-> Result<(), E>` skips serde_v8 and `rv.set` calls for `Ok()` branch.
- `-> ResourceId` or `-> [int]` types will use specialized method like
  `v8::ReturnValue::set_uint32`. A fast path for SMI.
- `-> Result<ResourceId, E>` or `-> Result<[int], E>` types will be optimized
  like above for the `Ok()` branch.

### Fast calls

The macro will infer and try to auto generate V8 fast API call trait impl for
`sync` ops with:

- arguments: integers / `&mut OpState`
- return_type: integers

The `#[op(fast)]` attribute shoukd be used to enforce fast call generation at
compile time.

Trait gen for `async` ops & a ZeroCopyBuf equivalent type is planned and will be
added soon.
