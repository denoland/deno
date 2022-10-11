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

- arguments: integers, bool, `&mut OpState`, `&[u8]`,
  `&mut [u8]`,`&[u32]`,`&mut [u32]`
- return_type: integers, bool

The `#[op(fast)]` attribute should be used to enforce fast call generation at
compile time.

### Async fast calls

Async ops annotated with `#[op(fast)]` that return `()` or `Result<(), Error>`
are elligible for fast call scheduling. They should be called from JS using

```js
const { core } = Deno;
const { ops } = core;

const buf = new Uint8Array(1024);
const rid = 0; // You'd get this from another op.
const call = (i, p) => ops.op_read(i, p, rid, buf);

await core.opFastAsync(call);
```
