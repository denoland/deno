# deno_crypto

**This crate implements the Web Cryptography API.**

Spec: https://www.w3.org/TR/WebCryptoAPI/

## Usage Example

From javascript, include the extension's source, and assign `CryptoKey`,
`crypto`, `Crypto`, and `SubtleCrypto` to the global scope:

```javascript
import { core } from "ext:core/mod.js";

const crypto = core.loadExtScript("ext:deno_crypto/00_crypto.js");

Object.defineProperty(globalThis, "CryptoKey", {
  value: crypto.CryptoKey,
  enumerable: false,
  configurable: true,
  writable: true,
});

Object.defineProperty(globalThis, "crypto", {
  value: crypto.crypto,
  enumerable: false,
  configurable: true,
  writable: false,
});

Object.defineProperty(globalThis, "Crypto", {
  value: crypto.Crypto,
  enumerable: false,
  configurable: true,
  writable: true,
});

Object.defineProperty(globalThis, "SubtleCrypto", {
  value: crypto.SubtleCrypto,
  enumerable: false,
  configurable: true,
  writable: true,
});
```

Then from rust, provide: `deno_crypto::deno_crypto::init(Option<u64>)` in the
`extensions` field of your `RuntimeOptions`

Where the `Option<u64>` represents an optional seed for initialization.

## Dependencies

- **deno_webidl**: Provided by the `deno_webidl` crate
- **deno_web**: Provided by the `deno_web` crate

## Surface

All WebCrypto entry points are implemented as methods on the `Crypto` /
`SubtleCrypto` / `CryptoKey` cppgc-wrapped classes registered under the
extension's `objects = [...]` list. There are no standalone ops; the
per-algorithm helpers in `subtle_*.rs` are pure-Rust and called directly from
those method bodies.
