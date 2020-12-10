## Overview of TypeScript in Deno

One of the benefits of Deno is that it treats TypeScript as a first class
language, just like JavaScript or Web Assembly, when running code in Deno. What
that means is you can run or import TypeScript without installing anything more
than the Deno CLI.

_But wait a minute, does Deno really run TypeScript?_ you might be asking
yourself. Well, depends on what you mean by run. One could argue that in a
browser you don't actually _run_ JavaScript either. The JavaScript engine in the
browser translates the JavaScript to a series of operation codes, which it then
executes in a sandbox. So it translates JavaScript to something close to
assembly. Even Web Assembly goes through a similar translation, in that Web
Assembly is architecture agnostic while it needs to be translated into the
machine specific operation codes needed for the particular platform architecture
it is running on. So when we say TypeScript is a first class language in Deno,
we mean that we try to make the user experience in authoring and running
TypeScript as easy and straightforward as JavaScript and Web Assembly.

Behind the scenes, we use a combination of technologies, in Rust and JavaScript,
to provide that experience.

### How does it work?

At a high level, Deno converts TypeScript (as well as TSX and JSX) into
JavaScript. It does this via a combination of the
[TypeScript compiler](https://github.com/microsoft/TypeScript), which we build
into Deno, and a Rust library called [swc](https://swc.rs/). When the code has
been type checked and transformed, it is stored in a cache, ready for the next
run without the need to convert it from its source to JavaScript again.

You can see this cache location by running `deno info`:

```shell
> deno info
DENO_DIR location: "/path/to/cache/deno"
Remote modules cache: "/path/to/cache/deno/deps"
TypeScript compiler cache: "/path/to/cache/deno/gen"
```

If you were to look in that cache, you would see a directory structure that
mimics that source directory structure and individual `.js` and `.meta` files
(also potentially `.map` files). The `.js` file is the transformed source file
while the `.meta` file contains meta data we want to cache about the file, which
at the moment contains a _hash_ of the source module that helps us manage cache
invalidation. You might also see a `.buildinfo` file as well, which is a
TypeScript compiler incremental build information file, which we cache to help
speed up type checking.

### Type Checking

One of the main advantages of TypeScript is that you can make code more type
safe, so that what would be syntactically valid JavaScript becomes TypeScript
with warnings about being "unsafe".

In Deno we handle TypeScript in two major ways. We can type check TypeScript,
the default, or you can opt into skipping that checking using the `--no-check`
flag. For example if you had a program you wanted to run, normally you would do
something like this:

```
deno run --allow-net my_server.ts
```

But if you wanted to skip the type checking, you would do something like this:

```
deno run --allow-net --no-check my_server.ts
```

Type checking can take a significant amount of time, especially if you are
working on a code base where you are making a lot of changes. We have tried to
optimise the type checking, but it still comes at a cost. If you just want to
hack at some code, or if you are working in an IDE which is type checking your
code as you author it, using `--no-check` can certainly speed up the process of
running TypeScript in Deno.

### Determining the type of file

Since Deno supports JavaScript, TypeScript, JSX, TSX modules, Deno has to make a
decision about how to treat each of these kinds of files. For local modules,
Deno makes this determination based fully on the extension. When the extension
is absent in a local file, it is assumed to be JavaScript.

For remote modules, the media type (mime-type) is used to determine the type of
the module, where the path of the module is used to help influence the file
type, when it is ambiguous what type of file it is.

For example, a `.d.ts` file and a `.ts` file have different semantics in
TypeScript as well as have different ways they need to be handled in Deno. While
we expect to convert a `.ts` file into JavaScript, a `.d.ts` file contains no
"runnable" code, and is simply describing types (often of "plain" JavaScript).
So when we fetch a remote module, the media type for a `.ts.` and `.d.ts` file
looks the same. So we look at the path, and if we see something that has a path
that ends with `.d.ts` we treat it as a type definition only file instead of
"runnable" TypeScript.

### Strict by default

Deno type checks TypeScript in _strict_ mode by default, and the TypeScript core
team recommends _strict_ mode as a sensible default. This mode generally enables
features of TypeScript that probably should have been there from the start, but
as TypeScript continued to evolve, would be breaking changes for existing code.

### Mixing JavaScript and TypeScript

By default, Deno does not type check JavaScript. This can be changed, and is
discussed further in [Configuring TypeScript in Deno](./configuration.md). Deno
does support JavaScript importing TypeScript and TypeScript import JavaScript,
in complex scenarios.

An important note though is that when type checking TypeScript, by default Deno
will "read" all the JavaScript in order to be able to evaluate how it might have
an impact on the TypeScript types. The type checker will do the best it can to
figure out what the types are of the JavaScript you import into TypeScript,
including reading any JSDoc comments. Details of this are discussed in detail in
the [Types and type declarations](./types.md) section.

### Diagnostics are terminal

While `tsc` by default will still emit JavaScript when run while encountering
diagnostic (type checking) issues, Deno currently treats them as terminal. It
will halt on these warnings, not cache any of the emitted files, and exit the
process.

In order to avoid this, you will either need to resolve the issue, utilise the
`// @ts-ignore` or `// @ts-expect-error` pragmas, or utilise `--no-check` to
bypass type checking all together.

### Type resolution

One of the core design principles of Deno is to avoid "magical" resolution, and
this applies to type resolution as well. If you want to utilise JavaScript that
has type definitions (e.g. a `.d.ts` file), you have to explicitly tell Deno
about this. The details of how this is accomplished are covered in the
[Types and type declarations](./types.md) section.
