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
