`std/_crypto_wasm` is only for internal use, such as by `std/crypto`. Its
interface may not be stable between releases and it should not be imported
directly.

## Overview

This folder contains Rust code that we use via Wasm. It allows us to take
advantage of existing Rust implementations of crypto algorithms such as SHA-1
and use them here in deno_std.

## How to Build

```sh
deno task build:crypto
```

This will regenerate the files in the `./lib/` folder from the Rust source.
