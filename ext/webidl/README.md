# deno_webidl

**This crate implements WebIDL for Deno. It consists of infrastructure to do
ECMA -> WebIDL conversions.**

Spec: https://webidl.spec.whatwg.org/

## Usage Example

From javascript, include the extension's source, and assign the following to the
global scope:

```javascript
import * as webidl from "ext:deno_webidl/00_webidl.js";
Object.defineProperty(globalThis, webidl.brand, {
  value: webidl.brand,
  enumerable: false,
  configurable: true,
  writable: true,
});
```

Then from rust, provide `init_webidl::init_webidl::init_ops_and_esm()` in the
`extensions` field of your `RuntimeOptions`
