# Plugins

Short explanation showing how Deno Rust plugins can be used. _**!!! warning the
feature is still unstable!!!**_

Clone the [deno](https://github.com/denoland/deno.git) source & go to
`deno/test_plugin`

The repo is structure as following:

```
test_plugin
├── Cargo.toml
├── src
│   └── lib.rs
└── tests
    ├── ...
    └── test.js
```

It is a rust repo.

## In the `rust`

#### 1- add `deno_core` & specify your project as a crate of type `cdylib` to make the lib dynamically available.

In the cargo manifest

```
[package]
name = "test_plugin"
...

[lib]
crate-type = ["cdylib"]

[dependencies]
...

deno_core = { path = "../core" }
...
```

#### 2- wrap your custom lib functionalities in ops functions,

```rust
// in lib.rs
fn op_test_sync(
  _interface: &mut dyn Interface,
  zero_copy: &mut [ZeroCopyBuf],
) -> Op {

  ...

  Op::Sync(result_box)
}
```

#### 3- register then inside the plugin container `deno_plugin_init`.

```rust
// in lib.rs
#[no_mangle]
pub fn deno_plugin_init(interface: &mut dyn Interface) {
  interface.register_op("testSync", op_test_sync);
  ...
}
```

#### 4- build your lib with cargo : `cargo build -p <name of the crate>`

```shell
$ cargo build -p test_plugin --release
```

## In js

#### 5- open your lib with `Deno.openPlugin`. You had to specify the build target from (4)

```js
// in test.js
 const rid = Deno.openPlugin(<path to *.dylib>);
```

#### 6 - get your functionality from ops with `Deno.core.ops`

```js
// in test.js
const { testSync } = Deno.core.ops();
```

#### 7 - dispatch call to your functionality with ops using `Deno.core.dispatch`

```js
// in test.js
const response = Deno.core.dispatch(
  testSync,
  new Uint8Array([116, 101, 115, 116]),
  new Uint8Array([49, 50, 51]),
  new Uint8Array([99, 98, 97]),
);
```

async ops:

```js
// in test.js
Deno.core.setAsyncHandler(testAsync, (response) => {
  console.log(`Plugin Async Response: ${textDecoder.decode(response)}`);
});

const response = Deno.core.dispatch(
  testAsync,
  new Uint8Array([116, 101, 115, 116]),
  new Uint8Array([49, 50, 51]),
);
```

#### 8 - close the plugin

```
// in test.js
  Deno.close(rid);
```

#### 9 - run your script

```shell
$ deno run --allow-plugin --unstable tests/test.js release
```
