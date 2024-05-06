A `ld -lazy_framework` for macOS. Designed to build Deno.

## Usage

```toml
[target.aarch64-apple-darwin]
rustflags = [
  "-C",
  "linker=/path/to/lzld/lzld",
  "-C",
  "link-args=-L/path/to/lzld -llzld"
]
```

### Usage without `lzld` wrapper

1. `rustc -Z link-native-libraries=no -L/path/to/lzld -llzld`:
Requires nightly but doesn't need a wrapper linker.

2. Manaully source modification: Remove `#[link]` attributes
from all dependencies and link to `liblzld.a`.

## Design 

It's pretty simple. Drop in `lzld` as the linker.
It strips out `-framework` arguments and links a 
static library (`liblzld.a`) that will lazy load 
the framework via `dlopen` when needed. 

Rest of the arguments are passed as-is to `lld`.

<!--
Supported frameworks:
- QuartzCore
- CoreFoundation
- TODO
-->


