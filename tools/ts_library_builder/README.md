# ts_library_builder

This tool allows us to produce a single TypeScript declaration file that
describes the complete Deno runtime, including global variables and the built-in
`deno` module. The output of this tool, `lib.deno_runtime.d.ts`, serves several
purposes:

1. It is passed to the TypeScript compiler `js/compiler.ts`, so that TypeScript
   knows what types to expect and can validate code against the runtime
   environment.
2. It is outputted to stdout by `deno --types`, so that users can easily have
   access to the complete declaration file. Editors can use this in the future
   to perform type checking.
3. Because JSDocs are maintained, this serves as a simple documentation page for
   Deno. We will use this file to generate HTML docs in the future.

The tool depends upon a couple libraries:

- [`ts-node`](https://www.npmjs.com/package/ts-node) to provide just in time
  transpiling of TypeScript for the tool itself.
- [`ts-simple-ast`](https://www.npmjs.com/package/ts-simple-ast) which provides
  a more rational and functional interface to the TypeScript AST to make
  manipulations easier.
- [`prettier`](https://www.npmjs.com/package/prettier) and
  [`@types/prettier`](https://www.npmjs.com/package/@types/prettier) to format
  the output.

## Design

Ideally we wouldn't have to build this tool at all, and could simply use `tsc`
to output this declaration file. While, `--emitDeclarationsOnly`, `--outFile`
and `--module AMD` generates a single declaration file, it isn't clean. It was
never designed for a library generation, where what is available in a runtime
environment significantly differs from the code that creates that environment's
structure.

Therefore this tool injects some of the knowledge of what occurs in the Deno
runtime environment as well as ensures that the output file is more clean and
logical for an end user. In the deno runtime, code runs in a global scope that
is defined in `js/global.ts`. This contains global scope items that one
reasonably expects in a JavaScript runtime, like `console`. It also defines the
global scope on a self-reflective `window` variable. There is currently only one
module of Deno specific APIs which is available to the user. This is defined in
`js/deno.ts`.

This tool takes advantage of an experimental feature of TypeScript that items
that are not really intended to be part of the public API are marked with a
comment pragma of `@internal` and then are not emitted when generating type
definitions. In addition TypeScript will _tree-shake_ any dependencies tied to
that "hidden" API and elide them as well. This really helps keep the public API
clean and as minimal as needed.

In order to create the default type library, the process at a high-level looks
like this:

- We read in all of the runtime environment definition code into TypeScript AST
  parser "project".
- We emit the TypeScript type definitions only into another AST parser
  "project".
- We process the `deno` namespace/module, by "flattening" the type definition
  file.
  - We determine the exported symbols for `js/deno.ts`.
  - We create a custom extraction of the `gen/msg_generated.ts` which is
    generated during the build process and contains the type information related
    to flatbuffer structures that communicate between the privileged part of
    deno and the user land. Currently, the tool doesn't do full complex
    dependency analysis to be able to determine what is required out of this
    file, so we explicitly extract the type information we need.
  - We recurse over all imports/exports of the modules, only exporting those
    symbols which are finally exported by `js/deno.ts`.
  - We replace the import/export with the type information from the source file.
  - This process assumes that all the modules that feed `js/deno.ts` will have a
    public type API that does not have name conflicts.
- We process the `js/globals.ts` file to generate the global namespace.
  - We create a `Window` interface and a `global` scope augmentation namespace.
  - We iterate over augmentations to the `window` variable declared in the file,
    extract the type information and apply it to both a global variable
    declaration and a property on the `Window` interface.
  - We identify any type aliases in the module and declare them globally.
- We take each namespace import to `js/globals.ts`, we resolve the emitted
  declaration `.d.ts` file and create it as its own namespace within the global
  scope. It is unsafe to just flatten these, because there is a high risk of
  collisions, but also, it makes authoring the types easier within the generated
  interface and variable declarations.
- We then validate the resulting definition file and write it out to the
  appropriate build path.

## TODO

- The tool does not _tree-shake_ when flattening imports. This means there are
  extraneous types that get included that are not really needed and it means
  that `gen/msg_generated.ts` has to be explicitly carved down.
- Complete the tests... we have some coverage, but not a lot of what is in
  `ast_util_test` which is being tested implicitly.
