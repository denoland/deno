# Contributing Guidelines

This is the main repository that provides the `deno` CLI.

If you want to fix a bug or add a new feature to `deno` this is the repository
to contribute to.

Some systems, including a large part of the Node.js compatibility layer are
implemented in JavaScript and TypeScript modules. These are a good place to
start if you are looking to make your first contribution.

[Here](https://node-test-viewer.deno.dev/results/latest) is a list of Node.js
test cases, including both successful and failing ones. Reviewing these can
provide valuable insight into how the compatibility layer works in practice, and
where improvements might be needed. They can also serve as a useful guide for
identifying areas where contributions are most impactful.

While iterating on such modules it is recommended to include `--features hmr` in
your `cargo` flags. This is a special development mode where the JS/TS sources
are not included in the binary but read at runtime, meaning the binary will not
have to be rebuilt if they are changed.

To use the commands below, you need to first install the necessary tools on your
system as described [here](#building-from-source).

```sh
# cargo build
cargo build --features hmr

# cargo run -- run hello.ts
cargo run --features hmr -- run hello.ts

# cargo test integration::node_unit_tests::os_test
cargo test --features hmr integration::node_unit_tests::os_test
```

Also remember to reference this feature flag in your editor settings. For VSCode
users, combine the following into your workspace file:

```jsonc
{
  "settings": {
    "rust-analyzer.cargo.features": ["hmr"],
    // Adds support for resolving internal `ext:*` modules
    "deno.importMap": "tools/core_import_map.json"
  }
}
```

To use a development version of the LSP in VSCode:

1. Install and enable the
   [Deno VSCode extension](https://marketplace.visualstudio.com/items?itemName=denoland.vscode-deno)
2. Update your VSCode settings and point `deno.path` to your development binary:

```jsonc
// .vscode/settings.json
{
  "deno.path": "/path/to/your/deno/target/debug/deno"
}
```

## Submitting a PR

In addition to the above make sure that:

> To use the commands below, you need to first install the necessary tools on
> your system as described [here](building_from_source).

1. `cargo test` passes - this will run full test suite for `deno` including unit
   tests, integration tests and Web Platform Tests

1. Run `./tools/format.js` - this will format all of the code to adhere to the
   consistent style in the repository

1. Run `./tools/lint.js` - this will check Rust and JavaScript code for common
   mistakes and errors using `clippy` (for Rust) and `dlint` (for JavaScript)

## Building from source

Below are instructions on how to build Deno from source. If you just want to use
Deno you can download a prebuilt executable (more information in the
[`Getting Started`](/runtime/getting_started/installation/) chapter).

### Cloning the Repository

> Deno uses submodules, so you must remember to clone using
> `--recurse-submodules`.

**Linux(Debian)**/**Mac**/**WSL**:

```shell
git clone --recurse-submodules https://github.com/denoland/deno.git
```

**Windows**:

1. [Enable "Developer Mode"](https://www.google.com/search?q=windows+enable+developer+mode)
   (otherwise symlinks would require administrator privileges).
2. Make sure you are using git version 2.19.2.windows.1 or newer.
3. Set `core.symlinks=true` before the checkout:

   ```shell
   git config --global core.symlinks true
   git clone --recurse-submodules https://github.com/denoland/deno.git
   ```

### Prerequisites

#### Rust

> Deno requires a specific release of Rust. Deno may not support building on
> other versions, or on the Rust Nightly Releases. The version of Rust required
> for a particular release is specified in the `rust-toolchain.toml` file.

[Update or Install Rust](https://www.rust-lang.org/tools/install). Check that
Rust installed/updated correctly:

```console
rustc -V
cargo -V
```

#### Native Compilers and Linkers

Many components of Deno require a native compiler to build optimized native
functions.

##### Linux(Debian)/WSL

```shell
wget https://apt.llvm.org/llvm.sh
chmod +x llvm.sh
./llvm.sh 17
apt install --install-recommends -y cmake libglib2.0-dev
```

##### Mac

Mac users must have the _XCode Command Line Tools_ installed.
([XCode](https://developer.apple.com/xcode/) already includes the _XCode Command
Line Tools_. Run `xcode-select --install` to install it without XCode.)

[CMake](https://cmake.org/) is also required, but does not ship with the
_Command Line Tools_.

```console
brew install cmake
```

##### Mac M1/M2

For Apple aarch64 users `lld` must be installed.

```console
brew install llvm lld
# Add /opt/homebrew/opt/llvm/bin/ to $PATH
```

##### Windows

1. Get [VS Community 2019](https://www.visualstudio.com/downloads/) with the
   "Desktop development with C++" toolkit and make sure to select the following
   required tools listed below along with all C++ tools.

   - Visual C++ tools for CMake
   - Windows 10 SDK (10.0.17763.0)
   - Testing tools core features - Build Tools
   - Visual C++ ATL for x86 and x64
   - Visual C++ MFC for x86 and x64
   - C++/CLI support
   - VC++ 2015.3 v14.00 (v140) toolset for desktop

2. Enable "Debugging Tools for Windows".
   - Go to "Control Panel" → "Programs" → "Programs and Features"
   - Select "Windows Software Development Kit - Windows 10"
   - → "Change" → "Change" → Check "Debugging Tools For Windows" → "Change"
     →"Finish".
   - Or use:
     [Debugging Tools for Windows](https://docs.microsoft.com/en-us/windows-hardware/drivers/debugger/)
     (Notice: it will download the files, you should install
     `X64 Debuggers And Tools-x64_en-us.msi` file manually.)

#### Protobuf Compiler

Building Deno requires the
[Protocol Buffers compiler](https://grpc.io/docs/protoc-installation/).

##### Linux(Debian)/WSL

```sh
apt install -y protobuf-compiler
protoc --version  # Ensure compiler version is 3+
```

##### Mac

```sh
brew install protobuf
protoc --version  # Ensure compiler version is 3+
```

##### Windows

Windows users can download the latest binary release from
[GitHub](https://github.com/protocolbuffers/protobuf/releases/latest).

### Python 3

> Deno requires [Python 3](https://www.python.org/downloads) for running WPT
> tests. Ensure that a suffix-less `python`/`python.exe` exists in your `PATH`
> and it refers to Python 3.

### Building Deno

The easiest way to build Deno is by using a precompiled version of V8.

_For WSL make sure you have sufficient memory allocated in `.wslconfig`. It is
recommended that you allocate at least 16GB._

```console
cargo build -vv
```

However, you may also want to build Deno and V8 from source code if you are
doing lower-level V8 development, or using a platform that does not have
precompiled versions of V8:

```console
V8_FROM_SOURCE=1 cargo build -vv
```

When building V8 from source, there may be more dependencies. See
[rusty_v8's README](https://github.com/denoland/rusty_v8) for more details about
the V8 build.

### Building

Build with Cargo:

```shell
# Build:
cargo build -vv

# Build errors?  Ensure you have latest main and try building again, or if that doesn't work try:
cargo clean && cargo build -vv

# Run:
./target/debug/deno run tests/testdata/run/002_hello.ts
```

### Running the Tests

Deno has a comprehensive test suite written in both Rust and TypeScript. The
Rust tests can be run during the build process using:

```shell
cargo test -vv
```

The TypeScript tests can be run using:

```shell
# Run all unit/tests:
target/debug/deno test -A --unstable --lock=tools/deno.lock.json --config tests/config/deno.json tests/unit

# Run a specific test:
target/debug/deno test -A --unstable --lock=tools/deno.lock.json --config tests/config/deno.json tests/unit/os_test.ts
```

### Working with Multiple Crates

If a change-set spans multiple Deno crates, you may want to build multiple
crates together. It's suggested that you checkout all the required crates next
to one another. For example:

```shell
- denoland/
  - deno/
  - deno_core/
  - deno_ast/
  - ...
```

Then you can use
[Cargo's patch feature](https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html)
to override the default dependency paths:

```shell
cargo build --config 'patch.crates-io.deno_ast.path="../deno_ast"'
```

If you are working on a change-set for few days, you may prefer to add the patch
to your `Cargo.toml` file (just make sure you remove this before staging your
changes):

```sh
[patch.crates-io]
deno_ast = { path = "../deno_ast" }
```

This will build the `deno_ast` crate from the local path and link against that
version instead of fetching it from `crates.io`.

**Note**: It's important that the version of the dependencies in the
`Cargo.toml` match the version of the dependencies you have on disk.

Use `cargo search <dependency_name>` to inspect the versions.
