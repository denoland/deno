## Compiler API

> This is an unstable Deno feature. Learn more about
> [unstable features](./stability.md).

Deno supports runtime access to the built-in TypeScript compiler. There are
three methods in the `Deno` namespace that provide this access.

### `Deno.compile()`

This works similar to `deno cache` in that it can fetch and cache the code,
compile it, but not run it. It takes up to three arguments, the `rootName`,
optionally `sources`, and optionally `options`. The `rootName` is the root
module which will be used to generate the resulting program. This is like the
module name you would pass on the command line in
`deno run --reload example.ts`. The `sources` is a hash where the key is the
fully qualified module name, and the value is the text source of the module. If
`sources` is passed, Deno will resolve all the modules from within that hash and
not attempt to resolve them outside of Deno. If `sources` are not provided, Deno
will resolve modules as if the root module had been passed on the command line.
Deno will also cache any of these resources. All resolved resources are treated
as dynamic imports and require read or net permissions depending on if they're
local or remote. The `options` argument is a set of options of type
`Deno.CompilerOptions`, which is a subset of the TypeScript compiler options
containing the ones supported by Deno.

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

In this case `emitMap` will contain a `console.log()` statement.

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
the root module had been passed on the command line. All resolved resources are
treated as dynamic imports and require read or net permissions depending if
they're local or remote. Deno will also cache any of these resources. The
`options` argument is a set of options of type `Deno.CompilerOptions`, which is
a subset of the TypeScript compiler options containing the ones supported by
Deno.

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
