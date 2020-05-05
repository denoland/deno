## Built-in Deno Utilities / Commands

<!-- prettier-ignore-start -->
<!-- prettier incorrectly moves the coming soon links to new lines -->

- dependency inspector (`deno info`)
- code formatter (`deno fmt`)
- bundling (`deno bundle`)
- runtime type info (`deno types`)
- test runner (`deno test`)
- command-line debugger (`--debug`)
- linter (`deno lint`) [coming soon](https://github.com/denoland/deno/issues/1880)

<!-- prettier-ignore-end -->

## API reference

### `deno types`

To get an exact reference of deno's runtime API, run the following in the
command line:

```shell
$ deno types
```

The output is the concatenation of three library files that are built into Deno:

- [lib.deno.ns.d.ts](https://github.com/denoland/deno/blob/master/cli/js/lib.deno.ns.d.ts)
- [lib.deno.shared_globals.d.ts](https://github.com/denoland/deno/blob/master/cli/js/lib.deno.shared_globals.d.ts)
- [lib.deno.window.d.ts](https://github.com/denoland/deno/blob/master/cli/js/lib.deno.window.d.ts)

### Reference websites

[TypeScript Deno API](https://deno.land/typedoc/index.html).

If you are embedding deno in a Rust program, see
[Rust Deno API](https://docs.rs/deno).

The Deno crate is hosted on [crates.io](https://crates.io/crates/deno).

## Examples

<!-- Should this be part of examples? Probably fits better into 'Linking to external code' -->

### Reload specific modules

Sometimes we want to upgrade only some modules. You can control it by passing an
argument to a `--reload` flag.

To reload everything

`--reload`

To reload all standard modules

`--reload=https://deno.land/std`

To reload specific modules (in this example - colors and file system utils) use
a comma to separate URLs

`--reload=https://deno.land/std/fs/utils.ts,https://deno.land/std/fmt/colors.ts`

<!-- Should this be part of examples? -->

### Permissions whitelist

Deno also provides permissions whitelist.

This is an example to restrict file system access by whitelist.

```shell
$ deno --allow-read=/usr https://deno.land/std/examples/cat.ts /etc/passwd
error: Uncaught PermissionDenied: read access to "/etc/passwd", run again with the --allow-read flag
► $deno$/dispatch_json.ts:40:11
    at DenoError ($deno$/errors.ts:20:5)
    ...
```

You can grant read permission under `/etc` dir

```shell
$ deno --allow-read=/etc https://deno.land/std/examples/cat.ts /etc/passwd
```

`--allow-write` works same as `--allow-read`.

This is an example to restrict host.

```ts
const result = await fetch("https://deno.land/");
```

```shell
$ deno --allow-net=deno.land https://deno.land/std/examples/curl.ts https://deno.land/
```

<!-- Part of Using Typescript -->

### Using external type definitions

Deno supports both JavaScript and TypeScript as first class languages at
runtime. This means it requires fully qualified module names, including the
extension (or a server providing the correct media type). In addition, Deno has
no "magical" module resolution.

The out of the box TypeScript compiler though relies on both extension-less
modules and the Node.js module resolution logic to apply types to JavaScript
modules.

In order to bridge this gap, Deno supports three ways of referencing type
definition files without having to resort to "magic" resolution.

#### Compiler hint

If you are importing a JavaScript module, and you know where the type definition
for that module is located, you can specify the type definition at import. This
takes the form of a compiler hint. Compiler hints inform Deno the location of
`.d.ts` files and the JavaScript code that is imported that they relate to. The
hint is `@deno-types` and when specified the value will be used in the compiler
instead of the JavaScript module. For example, if you had `foo.js`, but you know
that along side of it was `foo.d.ts` which was the types for the file, the code
would look like this:

```ts
// @deno-types="./foo.d.ts"
import * as foo from "./foo.js";
```

The value follows the same resolution logic as importing a module, meaning the
file needs to have an extension and is relative to the current module. Remote
specifiers are also allowed.

The hint affects the next `import` statement (or `export ... from` statement)
where the value of the `@deno-types` will be substituted at compile time instead
of the specified module. Like in the above example, the Deno compiler will load
`./foo.d.ts` instead of `./foo.js`. Deno will still load `./foo.js` when it runs
the program.

#### Triple-slash reference directive in JavaScript files

If you are hosting modules which you want to be consumed by Deno, and you want
to inform Deno about the location of the type definitions, you can utilise a
triple-slash directive in the actual code. For example, if you have a JavaScript
module and you would like to provide Deno with the location of the type
definitions which happen to be alongside that file, your JavaScript module named
`foo.js` might look like this:

```js
/// <reference types="./foo.d.ts" />
export const foo = "foo";
```

Deno will see this, and the compiler will use `foo.d.ts` when type checking the
file, though `foo.js` will be loaded at runtime. The resolution of the value of
the directive follows the same resolution logic as importing a module, meaning
the file needs to have an extension and is relative to the current file. Remote
specifiers are also allowed.

#### X-TypeScript-Types custom header

If you are hosting modules which you want to be consumed by Deno, and you want
to inform Deno the location of the type definitions, you can use a custom HTTP
header of `X-TypeScript-Types` to inform Deno of the location of that file.

The header works in the same way as the triple-slash reference mentioned above,
it just means that the content of the JavaScript file itself does not need to be
modified, and the location of the type definitions can be determined by the
server itself.

**Not all type definitions are supported.**

Deno will use the compiler hint to load the indicated `.d.ts` files, but some
`.d.ts` files contain unsupported features. Specifically, some `.d.ts` files
expect to be able to load or reference type definitions from other packages
using the module resolution logic. For example a type reference directive to
include `node`, expecting to resolve to some path like
`./node_modules/@types/node/index.d.ts`. Since this depends on non-relative
"magical" resolution, Deno cannot resolve this.

**Why not use the triple-slash type reference in TypeScript files?**

The TypeScript compiler supports triple-slash directives, including a type
reference directive. If Deno used this, it would interfere with the behavior of
the TypeScript compiler. Deno only looks for the directive in JavaScript (and
JSX) files.

<!-- Not really part of examples right? -->

### Referencing TypeScript library files

When you use `deno run`, or other Deno commands which type check TypeScript,
that code is evaluated against custom libraries which describe the environment
that Deno supports. By default, the compiler runtime APIs which type check
TypeScript also use these libraries (`Deno.compile()` and `Deno.bundle()`).

But if you want to compile or bundle TypeScript for some other runtime, you may
want to override the default libraries. To do this, the runtime APIs support the
`lib` property in the compiler options. For example, if you had TypeScript code
that is destined for the browser, you would want to use the TypeScript `"dom"`
library:

```ts
const [errors, emitted] = await Deno.compile(
  "main.ts",
  {
    "main.ts": `document.getElementById("foo");\n`,
  },
  {
    lib: ["dom", "esnext"],
  }
);
```

For a list of all the libraries that TypeScript supports, see the
[`lib` compiler option](https://www.typescriptlang.org/docs/handbook/compiler-options.html)
documentation.

**Don't forget to include the JavaScript library**

Just like `tsc`, when you supply a `lib` compiler option, it overrides the
default ones, which means that the basic JavaScript library won't be included
and you should include the one that best represents your target runtime (e.g.
`es5`, `es2015`, `es2016`, `es2017`, `es2018`, `es2019`, `es2020` or `esnext`).

#### Including the `Deno` namespace

In addition to the libraries that are provided by TypeScript, there are four
libraries that are built into Deno that can be referenced:

- `deno.ns` - Provides the `Deno` namespace.
- `deno.shared_globals` - Provides global interfaces and variables which Deno
  supports at runtime that are then exposed by the final runtime library.
- `deno.window` - Exposes the global variables plus the Deno namespace that are
  available in the Deno main worker and is the default for the runtime compiler
  APIs.
- `deno.worker` - Exposes the global variables that are available in workers
  under Deno.

So to add the Deno namespace to a compilation, you would include the `deno.ns`
lib in the array. For example:

```ts
const [errors, emitted] = await Deno.compile(
  "main.ts",
  {
    "main.ts": `document.getElementById("foo");\n`,
  },
  {
    lib: ["dom", "esnext", "deno.ns"],
  }
);
```

**Note** that the Deno namespace expects a runtime environment that is at least
ES2018 or later. This means if you use a lib "lower" than ES2018 you will get
errors logged as part of the compilation.

#### Using the triple slash reference

You do not have to specify the `lib` in the compiler options. Deno also supports
[the triple-slash reference to a lib](https://www.typescriptlang.org/docs/handbook/triple-slash-directives.html#-reference-lib-).
which can be embedded in the contents of the file. For example, if you have a
`main.ts` like:

```ts
/// <reference lib="dom" />

document.getElementById("foo");
```

It would compile without errors like this:

```ts
const [errors, emitted] = await Deno.compile("./main.ts", undefined, {
  lib: ["esnext"],
});
```

**Note** that the `dom` library conflicts with some of the default globals that
are defined in the default type library for Deno. To avoid this, you need to
specify a `lib` option in the compiler options to the runtime compiler APIs.

## Command line interface

### Flags

Use `deno help` to see help text documenting Deno's flags and usage. Use
`deno help <subcommand>` for subcommand-specific flags.

### Environmental variables

There are several env vars that control how Deno behaves:

`DENO_DIR` defaults to `$HOME/.deno` but can be set to any path to control where
generated and cached source code is written and read to.

`NO_COLOR` will turn off color output if set. See https://no-color.org/. User
code can test if `NO_COLOR` was set without having `--allow-env` by using the
boolean constant `Deno.noColor`.

### Shell completion

You can generate completion script for your shell using the
`deno completions <shell>` command. The command outputs to stdout so you should
redirect it to an appropriate file.

The supported shells are:

- zsh
- bash
- fish
- powershell
- elvish

Example:

```shell
deno completions bash > /usr/local/etc/bash_completion.d/deno.bash
source /usr/local/etc/bash_completion.d/deno.bash
```

### V8 flags

V8 has many many internal command-line flags.

```shell
# list available v8 flags
$ deno --v8-flags=--help

#  example for applying multiple flags
$ deno --v8-flags=--expose-gc,--use-strict
```

Particularly useful ones:

```
--async-stack-trace
```

### Bundling

`deno bundle [URL]` will output a single JavaScript file, which includes all
dependencies of the specified input. For example:

```
> deno bundle https://deno.land/std/examples/colors.ts colors.bundle.js
Bundling "colors.bundle.js"
Emitting bundle to "colors.bundle.js"
9.2 kB emitted.
```

If you omit the out file, the bundle will be sent to `stdout`.

The bundle can just be run as any other module in Deno would:

```
deno colors.bundle.js
```

The output is a self contained ES Module, where any exports from the main module
supplied on the command line will be available. For example, if the main module
looked something like this:

```ts
export { foo } from "./foo.js";

export const bar = "bar";
```

It could be imported like this:

```ts
import { foo, bar } from "./lib.bundle.js";
```

Bundles can also be loaded in the web browser. The bundle is a self-contained ES
module, and so the attribute of `type` must be set to `"module"`. For example:

```html
<script type="module" src="website.bundle.js"></script>
```

Or you could import it into another ES module to consume:

```html
<script type="module">
  import * as website from "website.bundle.js";
</script>
```

### Installing executable scripts

Deno provides `deno install` to easily install and distribute executable code.

`deno install [FLAGS...] [EXE_NAME] [URL] [SCRIPT_ARGS...]` will install the
script available at `URL` under the name `EXE_NAME`.

This command creates a thin, executable shell script which invokes `deno` using
the specified CLI flags and main module. It is place in the installation root's
`bin` directory.

Example:

```shell
$ deno install --allow-net --allow-read file_server https://deno.land/std/http/file_server.ts
[1/1] Compiling https://deno.land/std/http/file_server.ts

✅ Successfully installed file_server.
/Users/deno/.deno/bin/file_server
```

To change the installation root, use `--root`:

```shell
$ deno install --allow-net --allow-read --root /usr/local file_server https://deno.land/std/http/file_server.ts
```

The installation root is determined, in order of precedence:

- `--root` option
- `DENO_INSTALL_ROOT` environment variable
- `$HOME/.deno`

These must be added to the path manually if required.

```shell
$ echo 'export PATH="$HOME/.deno/bin:$PATH"' >> ~/.bashrc
```

You must specify permissions that will be used to run the script at installation
time.

```shell
$ deno install --allow-net --allow-read file_server https://deno.land/std/http/file_server.ts 8080
```

The above command creates an executable called `file_server` that runs with
write and read permissions and binds to port 8080.

For good practice, use the
[`import.meta.main`](#testing-if-current-file-is-the-main-program) idiom to
specify the entry point in an executable script.

Example:

```ts
// https://example.com/awesome/cli.ts
async function myAwesomeCli(): Promise<void> {
  -- snip --
}

if (import.meta.main) {
  myAwesomeCli();
}
```

When you create an executable script make sure to let users know by adding an
example installation command to your repository:

```shell
# Install using deno install

$ deno install awesome_cli https://example.com/awesome/cli.ts
```

## Compiler API

Deno supports runtime access to the built-in TypeScript compiler. There are
three methods in the `Deno` namespace that provide this access.

### `Deno.compile()`

This works similar to `deno cache` in that it can fetch and cache the code,
compile it, but not run it. It takes up to three arguments, the `rootName`,
optionally `sources`, and optionally `options`. The `rootName` is the root
module which will be used to generate the resulting program. This is like the
module name you would pass on the command line in
`deno --reload run example.ts`. The `sources` is a hash where the key is the
fully qualified module name, and the value is the text source of the module. If
`sources` is passed, Deno will resolve all the modules from within that hash and
not attempt to resolve them outside of Deno. If `sources` are not provided, Deno
will resolve modules as if the root module had been passed on the command line.
Deno will also cache any of these resources. The `options` argument is a set of
options of type `Deno.CompilerOptions`, which is a subset of the TypeScript
compiler options containing the ones supported by Deno.

The method resolves with a tuple. The first argument contains any diagnostics
(syntax or type errors) related to the code. The second argument is a map where
the keys are the output filenames and the values are the content.

An example of providing sources:

```ts
const [diagnostics, emitMap] = await Deno.compile("/foo.ts", {
  "/foo.ts": `import * as bar from "./bar.ts";\nconsole.log(bar);\n`,
  "/bar.ts": `export const bar = "bar";\n`,
});

assert(diagnostics == null); // ensuring no diagnostics are returned
console.log(emitMap);
```

We would expect map to contain 4 "files", named `/foo.js.map`, `/foo.js`,
`/bar.js.map`, and `/bar.js`.

When not supplying resources, you can use local or remote modules, just like you
could do on the command line. So you could do something like this:

```ts
const [diagnostics, emitMap] = await Deno.compile(
  "https://deno.land/std/examples/welcome.ts"
);
```

In this case `emitMap` will contain a simple `console.log()` statement.

### `Deno.bundle()`

This works a lot like `deno bundle` does on the command line. It is also like
`Deno.compile()`, except instead of returning a map of files, it returns a
single string, which is a self-contained JavaScript ES module which will include
all of the code that was provided or resolved as well as exports of all the
exports of the root module that was provided. It takes up to three arguments,
the `rootName`, optionally `sources`, and optionally `options`. The `rootName`
is the root module which will be used to generate the resulting program. This is
like module name you would pass on the command line in `deno bundle example.ts`.
The `sources` is a hash where the key is the fully qualified module name, and
the value is the text source of the module. If `sources` is passed, Deno will
resolve all the modules from within that hash and not attempt to resolve them
outside of Deno. If `sources` are not provided, Deno will resolve modules as if
the root module had been passed on the command line. Deno will also cache any of
these resources. The `options` argument is a set of options of type
`Deno.CompilerOptions`, which is a subset of the TypeScript compiler options
containing the ones supported by Deno.

An example of providing sources:

```ts
const [diagnostics, emit] = await Deno.bundle("/foo.ts", {
  "/foo.ts": `import * as bar from "./bar.ts";\nconsole.log(bar);\n`,
  "/bar.ts": `export const bar = "bar";\n`,
});

assert(diagnostics == null); // ensuring no diagnostics are returned
console.log(emit);
```

We would expect `emit` to be the text for an ES module, which would contain the
output sources for both modules.

When not supplying resources, you can use local or remote modules, just like you
could do on the command line. So you could do something like this:

```ts
const [diagnostics, emit] = await Deno.bundle(
  "https://deno.land/std/http/server.ts"
);
```

In this case `emit` will be a self contained JavaScript ES module with all of
its dependencies resolved and exporting the same exports as the source module.

### `Deno.transpileOnly()`

This is based off of the TypeScript function `transpileModule()`. All this does
is "erase" any types from the modules and emit JavaScript. There is no type
checking and no resolution of dependencies. It accepts up to two arguments, the
first is a hash where the key is the module name and the value is the content.
The only purpose of the module name is when putting information into a source
map, of what the source file name was. The second argument contains optional
`options` of the type `Deno.CompilerOptions`. The function resolves with a map
where the key is the source module name supplied, and the value is an object
with a property of `source` and optionally `map`. The first is the output
contents of the module. The `map` property is the source map. Source maps are
provided by default, but can be turned off via the `options` argument.

An example:

```ts
const result = await Deno.transpileOnly({
  "/foo.ts": `enum Foo { Foo, Bar, Baz };\n`,
});

console.log(result["/foo.ts"].source);
console.log(result["/foo.ts"].map);
```

We would expect the `enum` would be rewritten to an IIFE which constructs the
enumerable, and the map to be defined.

## TypeScript Compiler Options

In the Deno ecosystem, all strict flags are enabled in order to comply with
TypeScript's ideal of being `strict` by default. However, in order to provide a
way to support customization a configuration file such as `tsconfig.json` might
be provided to Deno on program execution.

You need to explicitly tell Deno where to look for this configuration by setting
the `-c` argument when executing your application.

```bash
deno -c tsconfig.json mod.ts
```

Following are the currently allowed settings and their default values in Deno:

```json
{
  "compilerOptions": {
    "allowJs": false,
    "allowUmdGlobalAccess": false,
    "allowUnreachableCode": false,
    "allowUnusedLabels": false,
    "alwaysStrict": true,
    "assumeChangesOnlyAffectDirectDependencies": false,
    "checkJs": false,
    "disableSizeLimit": false,
    "generateCpuProfile": "profile.cpuprofile",
    "jsx": "react",
    "jsxFactory": "React.createElement",
    "lib": [],
    "noFallthroughCasesInSwitch": false,
    "noImplicitAny": true,
    "noImplicitReturns": true,
    "noImplicitThis": true,
    "noImplicitUseStrict": false,
    "noStrictGenericChecks": false,
    "noUnusedLocals": false,
    "noUnusedParameters": false,
    "preserveConstEnums": false,
    "removeComments": false,
    "resolveJsonModule": true,
    "strict": true,
    "strictBindCallApply": true,
    "strictFunctionTypes": true,
    "strictNullChecks": true,
    "strictPropertyInitialization": true,
    "suppressExcessPropertyErrors": false,
    "suppressImplicitAnyIndexErrors": false,
    "useDefineForClassFields": false
  }
}
```

For documentation on allowed values and use cases please visit the
[typescript docs](https://www.typescriptlang.org/docs/handbook/compiler-options.html).

**Note**: Any options not listed above are either not supported by Deno or are
listed as deprecated/experimental in the TypeScript documentation.

## Program lifecycle

Deno supports browser compatible lifecycle events: `load` and `unload`. You can
use these events to provide setup and cleanup code in your program.

Listener for `load` events can be asynchronous and will be awaited. Listener for
`unload` events need to be synchronous. Both events cannot be cancelled.

Example:

```typescript
// main.ts
import "./imported.ts";

const handler = (e: Event): void => {
  console.log(`got ${e.type} event in event handler (main)`);
};

window.addEventListener("load", handler);

window.addEventListener("unload", handler);

window.onload = (e: Event): void => {
  console.log(`got ${e.type} event in onload function (main)`);
};

window.onunload = (e: Event): void => {
  console.log(`got ${e.type} event in onunload function (main)`);
};

// imported.ts
const handler = (e: Event): void => {
  console.log(`got ${e.type} event in event handler (imported)`);
};

window.addEventListener("load", handler);
window.addEventListener("unload", handler);

window.onload = (e: Event): void => {
  console.log(`got ${e.type} event in onload function (imported)`);
};

window.onunload = (e: Event): void => {
  console.log(`got ${e.type} event in onunload function (imported)`);
};

console.log("log from imported script");
```

Note that you can use both `window.addEventListener` and
`window.onload`/`window.onunload` to define handlers for events. There is a
major difference between them, let's run example:

```shell
$ deno main.ts
log from imported script
log from main script
got load event in onload function (main)
got load event in event handler (imported)
got load event in event handler (main)
got unload event in onunload function (main)
got unload event in event handler (imported)
got unload event in event handler (main)
```

All listeners added using `window.addEventListener` were run, but
`window.onload` and `window.onunload` defined in `main.ts` overridden handlers
defined in `imported.ts`.

## Internal details

### Profiling

To start profiling,

```sh
# Make sure we're only building release.
# Build deno and V8's d8.
ninja -C target/release d8

# Start the program we want to benchmark with --prof
./target/release/deno tests/http_bench.ts --allow-net --v8-flags=--prof &

# Exercise it.
third_party/wrk/linux/wrk http://localhost:4500/
kill `pgrep deno`
```

V8 will write a file in the current directory that looks like this:
`isolate-0x7fad98242400-v8.log`. To examine this file:

```sh
D8_PATH=target/release/ ./third_party/v8/tools/linux-tick-processor
isolate-0x7fad98242400-v8.log > prof.log
# on macOS, use ./third_party/v8/tools/mac-tick-processor instead
```

`prof.log` will contain information about tick distribution of different calls.

To view the log with Web UI, generate JSON file of the log:

```sh
D8_PATH=target/release/ ./third_party/v8/tools/linux-tick-processor
isolate-0x7fad98242400-v8.log --preprocess > prof.json
```

Open `third_party/v8/tools/profview/index.html` in your browser, and select
`prof.json` to view the distribution graphically.

Useful V8 flags during profiling:

- --prof
- --log-internal-timer-events
- --log-timer-events
- --track-gc
- --log-source-code
- --track-gc-object-stats

To learn more about `d8` and profiling, check out the following links:

- [https://v8.dev/docs/d8](https://v8.dev/docs/d8)
- [https://v8.dev/docs/profile](https://v8.dev/docs/profile)

### Debugging with LLDB

We can use LLDB to debug Deno.

```shell
$ lldb -- target/debug/deno run tests/worker.js
> run
> bt
> up
> up
> l
```

To debug Rust code, we can use `rust-lldb`. It should come with `rustc` and is a
wrapper around LLDB.

```shell
$ rust-lldb -- ./target/debug/deno run --allow-net tests/http_bench.ts
# On macOS, you might get warnings like
# `ImportError: cannot import name _remove_dead_weakref`
# In that case, use system python by setting PATH, e.g.
# PATH=/System/Library/Frameworks/Python.framework/Versions/2.7/bin:$PATH
(lldb) command script import "/Users/kevinqian/.rustup/toolchains/1.36.0-x86_64-apple-darwin/lib/rustlib/etc/lldb_rust_formatters.py"
(lldb) type summary add --no-value --python-function lldb_rust_formatters.print_val -x ".*" --category Rust
(lldb) type category enable Rust
(lldb) target create "../deno/target/debug/deno"
Current executable set to '../deno/target/debug/deno' (x86_64).
(lldb) settings set -- target.run-args  "tests/http_bench.ts" "--allow-net"
(lldb) b op_start
(lldb) r
```

### Deno Core

The core binding layer for Deno. It is released as a
[standalone crate](https://crates.io/crates/deno). Inside of core is V8 itself,
with a binding API called "libdeno". See the crate documentation for more
details.

### Continuous Benchmarks

See our benchmarks [over here](https://deno.land/benchmarks.html)

The benchmark chart supposes `//website/data.json` has the type
`BenchmarkData[]` where `BenchmarkData` is defined like the below:

```ts
interface ExecTimeData {
  mean: number;
  stddev: number;
  user: number;
  system: number;
  min: number;
  max: number;
}

interface BenchmarkData {
  created_at: string;
  sha1: string;
  benchmark: {
    [key: string]: ExecTimeData;
  };
  binarySizeData: {
    [key: string]: number;
  };
  threadCountData: {
    [key: string]: number;
  };
  syscallCountData: {
    [key: string]: number;
  };
}
```

### Logos

These Deno logos, like the Deno software, are distributed under the MIT license
(public domain and free for use)

- [A hand drawn one by @ry](https://deno.land/images/deno_logo.png)

- [An animated one by @hashrock](https://github.com/denolib/animated-deno-logo/)

- [A high resolution SVG one by @kevinkassimo](https://github.com/denolib/high-res-deno-logo)

- [A pixelated animation one by @tanakaworld](https://deno.land/images/deno_logo_4.gif)
