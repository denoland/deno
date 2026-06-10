# deno_webidl

**This crate implements WebIDL for Deno. It consists of infrastructure to do
ECMA -> WebIDL conversions.**

Spec: https://webidl.spec.whatwg.org/

## Usage Example

From javascript, include the extension's source, and assign the following to the
global scope:

```javascript
import { core } from "ext:core/mod.js";

const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
Object.defineProperty(globalThis, webidl.brand, {
  value: webidl.brand,
  enumerable: false,
  configurable: true,
  writable: true,
});
```

Then from rust, provide `deno_webidl::deno_webidl::init()` in the `extensions`
field of your `RuntimeOptions`
