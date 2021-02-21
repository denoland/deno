## Building from source

Below are instructions on how to build Deno from source. If you just want to use
Deno you can download a prebuilt executable (more information in the
`Getting Started` chapter).

### Cloning the Repository

Clone on Linux or Mac:

```shell
git clone --recurse-submodules https://github.com/denoland/deno.git
```

Extra steps for Windows users:

1. [Enable "Developer Mode"](https://www.google.com/search?q=windows+enable+developer+mode)
   (otherwise symlinks would require administrator privileges).
2. Make sure you are using git version 2.19.2.windows.1 or newer.
3. Set `core.symlinks=true` before the checkout:
   ```shell
   git config --global core.symlinks true
   git clone --recurse-submodules https://github.com/denoland/deno.git
   ```

### Prerequisites

> Deno requires the progressively latest stable release of Rust. Deno does not
> support the Rust nightlies.

[Update or Install Rust](https://www.rust-lang.org/tools/install). Check that
Rust installed/updated correctly:

```
rustc -V
cargo -V
```

### Setup rust targets and components

```shell
rustup target add wasm32-unknown-unknown
rustup target add wasm32-wasi
```

### Building Deno

The easiest way to build Deno is by using a precompiled version of V8:

```
cargo build -vv
```

However if you want to build Deno and V8 from source code:

```
V8_FROM_SOURCE=1 cargo build -vv
```

When building V8 from source, there are more dependencies:

[Python 2](https://www.python.org/downloads). Ensure that a suffix-less
`python`/`python.exe` exists in your `PATH` and it refers to Python 2,
[not 3](https://github.com/denoland/deno/issues/464#issuecomment-411795578).

For Linux users glib-2.0 development files must also be installed. (On Ubuntu,
run `apt install libglib2.0-dev`.)

Mac users must have Command Line Tools installed.
([XCode](https://developer.apple.com/xcode/) already includes CLT. Run
`xcode-select --install` to install it without XCode.)

For Windows users:

1. Get [VS Community 2019](https://www.visualstudio.com/downloads/) with
   "Desktop development with C++" toolkit and make sure to select the following
   required tools listed below along with all C++ tools.

   - Visual C++ tools for CMake
   - Windows 10 SDK (10.0.17763.0)
   - Testing tools core features - Build Tools
   - Visual C++ ATL for x86 and x64
   - Visual C++ MFC for x86 and x64
   - C++/CLI support
   - VC++ 2015.3 v14.00 (v140) toolset for desktop

2. Enable "Debugging Tools for Windows". Go to "Control Panel" → "Programs" →
   "Programs and Features" → Select "Windows Software Development Kit - Windows
   10" → "Change" → "Change" → Check "Debugging Tools For Windows" → "Change" →
   "Finish". Or use:
   [Debugging Tools for Windows](https://docs.microsoft.com/en-us/windows-hardware/drivers/debugger/)
   (Notice: it will download the files, you should install
   `X64 Debuggers And Tools-x64_en-us.msi` file manually.)

See [rusty_v8's README](https://github.com/denoland/rusty_v8) for more details
about the V8 build.

### Building

Build with Cargo:

```shell
# Build:
cargo build -vv

# Build errors?  Ensure you have latest main and try building again, or if that doesn't work try:
cargo clean && cargo build -vv

# Run:
./target/debug/deno run cli/tests/002_hello.ts
```
