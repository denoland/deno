# deno_io

**This crate provides IO primitives for other Deno extensions, this includes
stdio streams and abstraction over File System files.**

## Usage Example

From javascript, include the extension's source:

```javascript
import * as io from "ext:deno_io/12_io.js";
```

Then from rust, provide:
`deno_io::deno_io::init_ops_and_esm(Option<deno_io::Stdio>)` in the `extensions`
field of your `RuntimeOptions`

Where `deno_io::Stdio` implements `Default`, and can therefore be provided as
`Some(deno_io::Stdio::default())`

## Dependencies

- **deno_web**: Provided by the `deno_web` crate
- **deno_tty**: Provided in `deno/runtime/ops/tty.rs`
