# deno_console

**This crate implements the Console API.**

Spec: https://console.spec.whatwg.org/

## Usage Example

From javascript, include the extension's source, and assign a console to the
global scope:

```javascript
import * as console from "ext:deno_console/01_console.js";
Object.defineProperty(globalThis, "console", {
  value: new console.Console((msg, level) =>
    globalThis.Deno.core.print(msg, level > 1)
  ),
  enumerable: false,
  configurable: true,
  writable: true,
});
```

Then from rust, provide `deno_console::deno_console::init_ops_and_esm()` in the
`extensions` field of your `RuntimeOptions`

## Provided ops

Following ops are provided, which can be accessed through `Deno.ops`:

- op_preview_entries
