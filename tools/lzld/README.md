# `lzld`

This tools implements an alternative of the (deprecated) `ld -lazy_framework` to
lazy load frameworks as needed. Symbols used are manually added to `lzld.m`.

The purpose of this ld wrapper is to improve startup time on Mac. Because Deno
includes WebGPU, it needs to link to Metal and QuartzCore. We've observed that
loading frameworks during startup can cost as much as 8ms of startup time.

## Adding a new symbol binding

Add a binding for the used symbol in `lzld.m`, eg:

```diff
void *(*MTLCopyAllDevices_)(void) = 0;
+void *(*MTLSomethingSomething_)(void) = 0;

void loadMetalFramework() {
    void *handle = dlopen("/System/Library/Frameworks/Metal.framework/Metal", RTLD_LAZY);
    if (handle) {
        MTLCopyAllDevices_ = dlsym(handle, "MTLCopyAllDevices");
+       MTLSomethingSomething_ = dlsym(handle, "MTLSomethingSomething");
    }
}


+extern void *MTLSomethingSomething(void) {
+   if (MTLSomethingSomething_ == 0) {
+       loadMetalFramework();
+   }
+
+   return MTLSomethingSomething_();
+}

extern void *MTLCopyAllDevices(void) {
```

then build the static library with `make liblzld_arm64.a`.

## Usage

```toml
[target.aarch64-apple-darwin]
rustflags = [
  "-C",
  "linker=/path/to/lzld/lzld",
  "-C",
  "link-args=-L/path/to/lzld -llzld",
]
```

### Usage without `lzld` wrapper

1. `rustc -Z link-native-libraries=no -L/path/to/lzld -llzld`: Requires nightly
   but doesn't need a wrapper linker.

2. Manaully source modification: Remove `#[link]` attributes from all
   dependencies and link to `liblzld.a`.

## Design

It's pretty simple. Drop in `lzld` as the linker. It strips out `-framework`
arguments and links a static library (`liblzld.a`) that will lazy load the
framework via `dlopen` when needed.

Rest of the arguments are passed as-is to `lld`.
