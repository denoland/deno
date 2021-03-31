## FAQs about TypeScript in Deno

### Can I use TypeScript not written for Deno?

Maybe. That is the best answer, we are afraid. For lots of reasons, Deno has
chosen to have fully qualified module specifiers. In part this is because it
treats TypeScript as a first class language. Also, Deno uses explicit module
resolution, with no _magic_. This is effectively the same way browsers
themselves work, though they don't obviously support TypeScript directly. If
the TypeScript modules use imports that don't have these design decisions in
mind, they may not work under Deno.

Also, in recent versions of Deno (starting with 1.5), we have started to use a
Rust library to do transformations of TypeScript to JavaScript in certain
scenarios. Because of this, there are certain situations in TypeScript where
type information is required, and therefore those are not supported under Deno.
If you are using `tsc` as stand-alone, the setting to use is `"isolatedModules"`
and setting it to `true` to help ensure that your code can be properly handled
by Deno.

One of the ways to deal with the extension and the lack of _magical_ resolution
is to use [import maps](../linking_to_external_code/import_maps.md) which would
allow you to specify "packages" of bare specifiers which then Deno could resolve
and load.

### What version(s) of TypeScript does Deno support?

Deno is built with a specific version of TypeScript. To find out what this is,
type the following on the command line:

```shell
> deno --version
```

The TypeScript version (along with the version of Deno and v8) will be printed.
Deno tries to keep up to date with general releases of TypeScript, providing
them in the next patch or minor release of Deno.

### There was a breaking change in the version of TypeScript that Deno uses, why did you break my program?

We do not consider changes in behavior or breaking changes in TypeScript
releases as breaking changes for Deno. TypeScript is a generally mature language
and breaking changes in TypeScript are almost always "good things" making code
more sound, and it is best that we all keep our code sound. If there is a
blocking change in the version of TypeScript and it isn't suitable to use an
older release of Deno until the problem can be resolved, then you should be able
to use `--no-check` to skip type checking all together.

In addition you can utilize `@ts-ignore` to _ignore_ a specific error in code
that you control. You can also replace whole dependencies, using
[import maps](../linking_to_external_code/import_maps), for situations where a
dependency of a dependency isn't being maintained or has some sort of breaking
change you want to bypass while waiting for it to be updated.

### Why are you forcing me to use isolated modules, why can't I use const enums with Deno, why do I need to do export type?

As of Deno 1.5 we defaulted to _isolatedModules_ to `true` and in Deno 1.6 we
removed the options to set it back to `false` via a configuration file. The
_isolatedModules_ option forces the TypeScript compiler to check and emit
TypeScript as if each module would stand on its own. TypeScript has a few _type
directed emits_ in the language at the moment. While not allowing type directed
emits into the language was a design goal for TypeScript, it has happened
anyways. This means that the TypeScript compiler needs to understand the
erasable types in the code to determine what to emit, which when you are trying
to make a fully erasable type system on top of JavaScript, that becomes a
problem.

When people started transpiling TypeScript without `tsc`, these type directed
emits became a problem, since the likes of Babel simply try to erase the types
without needing to understand the types to direct the emit. In the internals of
Deno we have started to use a Rust based emitter which allows us to optionally
skip type checking and generates the bundles for things like `deno bundle`. Like
all transpilers, it doesn't care about the types, it just tries to erase them.
This means in certain situations we cannot support those type directed emits.

So instead of trying to get every user to understand when and how we could
support the type directed emits, we made the decision to disable the use of them
by forcing the _isolatedModules_ option to `true`. This means that even when we
are using the TypeScript compiler to emit the code, it will follow the same
"rules" that the Rust based emitter follows.

This means that certain language features are not supportable. Those features
are:

- Re-exporting of types is ambigious and requires to know if the source module
  is exporting runtime code or just type information. Therefore, it is
  recommended that you use `import type` and `export type` for type only imports
  and exports. This will help ensure that when the code is emitted, that all the
  types are erased.
- `const enum` is not supported. `const enum`s require type information to
  direct the emit, as `const enum`s get written out as hard coded values.
  Especially when `const enum`s get exported, they are a type system only
  construct.
- `export =` and `import =` are legacy TypeScript syntax which we do not
  support.
- Only `declare namespace` is support. Runtime `namespace` is legacy TypeScript
  syntax that is not supported.

### Why don't you support language service plugins or transformer plugins?

While `tsc` supports language service plugins, Deno does not. Deno does not
always use the built in TypeScript compiler to do what it does, and the
complexity of adding support for a language service plugin is not feasible.
TypeScript does not support emitter plugins, but there are a few community
projects which _hack_ emitter plugins into TypeScript. First, we wouldn't want
to support something that TypeScript doesn't support, plus we do not always use
the TypeScript compiler for the emit, which would mean we would need to ensure
we supported it in all modes, and the other emitter is written in Rust, meaning
that any emitter plugin for TypeScript wouldn't be available for the Rust
emitter.

The TypeScript in Deno isn't intended to be a fully flexible TypeScript
compiler. Its main purpose is to ensure that TypeScript and JavaScript can run
under Deno. The secondary ability to do TypeScript and JavaScript emitting via
the runtime API `Deno.emit()` is intended to be simple and straight forward and
support a certain set of use cases.
