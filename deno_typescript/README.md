# Deno TypeScript Crate

[![crates](https://img.shields.io/crates/v/deno_typescript.svg)](https://crates.io/crates/deno_typescript)
[![docs](https://docs.rs/deno_typescript/badge.svg)](https://docs.rs/deno_typescript)

This crate provides utilities to compile typescript, bundle it up, and create a
V8 snapshot, all during build. Snapshots allow the executable to startup fast.

## `system_loader.js`

This is a minimalistic implementation of a
[System](https://github.com/systemjs/systemjs) module loader. It is specifically
designed to load modules that are emitted from TypeScript the module format is
`"system"` and a single `"outfile"` is supplied, which is commonly refereed to
as a bundle.

Because this loader becomes part of an emitted bundle under `Deno.bundle()` and
`deno bundle`, it has minimal comments and very terse and cryptic syntax, which
isn't very self documenting. Because of this, a guide to this file is provided
here.

A bundle of System modules expects a `System.register()` function to be in scope
for registering the modules. Modules that are emitted from TypeScript in a
single out file always pass 3 arguments, the module specifier, an array of
strings of modules specifiers that this module depends upon, and finally a
module factory.

The module factory requires two arguments to be passed, a function for exporting
values and a context object. We have to bind to some information in the
environment to provide these, so `gC` gets the context and `gE` gets the export
function to be passed to a factory. The context contains information like the
module specifier, a reference to the dynamic `import()` and the equivalent of
`import.meta`. The export function takes either two arguments of an named export
and its value, or an object record of keys of the named exports and the values
of the exports.

Currently, TypeScript does not re-write dynamic imports which resolve to static
strings (see
[microsoft/TypeScript#37429](https://github.com/microsoft/TypeScript/issues/37429)),
which means the import specifier for a dynamic import which has been
incorporated in the bundle does not automatically match a module specifier that
has been registered in the bundle. The `di()` function provides the capability
to try to identify relative import specifiers and resolve them to a specifier
inside the bundle. If it does this, it resolves with the exports of the module,
otherwise it simply passes the module specifier to `import()` and returns the
resulting promise.

The running of the factories is handled by `rF()`. When the factory is run, it
returns an object with two keys, `execute` and `setters`. `execute` is a
function which finalises that instantiation of the module, and `setters` which
is an array of functions that sets the value of the exports of the dependent
module.

The `gExp()` and `gExpA()` are the recursive functions which returns the exports
of a given module. It will determine if the module has been fully initialized,
and if not, it will gather the exports of the dependencies, set those exports in
the module via the `setters` and run the modules `execute()`. It will then
always return or resolve with the exports of the module.

As of TypeScript 3.8, top level await is supported when emitting ES or System
modules. When Deno creates a module bundle, it creates a valid, self-contained
ES module which exports the exports of the "main" module that was used when the
bundle was created. If a module in the bundle requires top-level-await, then the
`execute()` function is emitted as an async function, returning a promise. This
means that in order to export the values of the main module, the instantiation
needs to utilise top-level-await as well.

At the time of this writing, while V8 and other JavaScript engines have
implemented top-level-await, no browsers have it implemented, meaning that most
browsers could not consume modules that require top-level-await.

In order to facilitate this, there are two functions that are in the scope of
the module in addition to the `System.register()` method. `__instantiate(main)`
will bootstrap everything synchronously and `__instantiateAsync(main)` will do
so asynchronously. When emitting a bundle that contains a module that requires
top-level-await, Deno will detect this and utilise
`await __instantiateAsync(main)` instead.
