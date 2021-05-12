## Types and Type Declarations

One of the design principles of Deno is no _magical_ resolution. When TypeScript
is type checking a file, it only cares about the types for the file, and the
`tsc` compiler has a lot of logic to try to resolve those types. By default, it
expects _ambiguous_ module specifiers with an extension, and will attempt to
look for the file under the `.ts` specifier, then `.d.ts`, and finally `.js`
(plus a whole other set of logic when the module resolution is set to `"node"`).
Deno deals with explicit specifiers.

This can cause a couple problems though. For example, let's say I want to
consume a TypeScript file that has already been transpiled to JavaScript along
with a type definition file. So I have `mod.js` and `mod.d.ts`. If I try to
import `mod.js` into Deno, it will only do what I ask it to do, and import
`mod.js`, but that means my code won't be as well type checked as if TypeScript
was considering the `mod.d.ts` file in place of the `mod.js` file.

In order to support this in Deno, Deno has two solutions, of which there is a
variation of a solution to enhance support. The two main situations you come
across would be:

- As the importer of a JavaScript module, I know what types should be applied to
  the module.
- As the supplier of the JavaScript module, I know what types should be applied
  to the module.

The latter case is the better case, meaning you as the provider or host of the
module, everyone can consume it without having to figure out how to resolve the
types for the JavaScript module, but when consuming modules that you may not
have direct control over, the ability to do the former is also required.

### Providing types when importing

If you are consuming a JavaScript module and you have either created types (a
`.d.ts` file) or have otherwise obtained the types, you want to use, you can
instruct Deno to use that file when type checking instead of the JavaScript file
using the `@deno-types` compiler hint. `@deno-types` needs to be a single line
double slash comment, where when used impacts the next import or re-export
statement.

For example if I have a JavaScript modules `coolLib.js` and I had a separate
`coolLib.d.ts` file that I wanted to use, I would import it like this:

```ts
// @deno-types="./coolLib.d.ts"
import * as coolLib from "./coolLib.js";
```

When type checking `coolLib` and your usage of it in the file, the
`coolLib.d.ts` types will be used instead of looking at the JavaScript file.

The pattern matching for the compiler hint is somewhat forgiving and will accept
quoted and non-question values for the specifier as well as it accepts
whitespace before and after the equals sign.

### Providing types when hosting

If you are in control of the source code of the module, or you are in control of
how the file is hosted on a web server, there are two ways to inform Deno of the
types for a given module, without requiring the importer to do anything special.

#### Using the triple-slash reference directive

Deno supports using the triple-slash reference directive, which adopts the
reference comment used by TypeScript in TypeScript files to _include_ other
files and applies it to JavaScript files.

For example, if I had create `coolLib.js` and along side of it I had created my
type definitions for my library in `coolLib.d.ts` I could do the following in
the `coolLib.js` file:

```js
/// <reference path="./coolLib.d.ts" />

// ... the rest of the JavaScript ...
```

When Deno encounters this directive, it would resolve the `./coolLib.d.ts` file
and use that instead of the JavaScript file when TypeScript was type checking
the file, but still load the JavaScript file when running the program.

#### Using X-TypeScript-Types header

Similar to the triple-slash directive, Deno supports a header for remote modules
that instructs Deno where to locate the types for a given module. For example, a
response for `https://example.com/coolLib.js` might look something like this:

```
HTTP/1.1 200 OK
Content-Type: application/javascript; charset=UTF-8
Content-Length: 648
X-TypeScript-Types: ./coolLib.d.ts
```

When seeing this header, Deno would attempt to retrieve
`https://example.com/coolLib.d.ts` and use that when type checking the original
module.

### Important points

#### Type declaration semantics

Type declaration files (`.d.ts` files) follow the same semantics as other files
in Deno. This means that declaration files are assumed to be module declarations
(_UMD declarations_) and not ambient/global declarations. It is unpredictable
how Deno will handle ambient/global declarations.

In addition, if a type declaration imports something else, like another `.d.ts`
file, its resolution follow the normal import rules of Deno. For a lot of the
`.d.ts` files that are generated and available on the web, they may not be
compatible with Deno.

To overcome this problem, some solution providers, like the
[Skypack CDN](https://www.skypack.dev/), will automatically bundle type
declarations just like they provide bundles of JavaScript as ESM.

#### Deno Friendly CDNs

There are CDNs which host JavaScript modules that integrate well with Deno.

- [Skypack.dev](https://docs.skypack.dev/skypack-cdn/code/deno) is a CDN which
  provides type declarations (via the `X-TypeScript-Types` header) when you
  append `?dts` as a query string to your remote module import statements. For
  example:

  ```ts
  import React from "https://cdn.skypack.dev/react?dts";
  ```

### Behavior of JavaScript when type checking

If you import JavaScript into TypeScript in Deno and there are no types, even if
you have `checkJs` set to `false` (the default for Deno), the TypeScript
compiler will still access the JavaScript module and attempt to do some static
analysis on it, to at least try to determine the shape of the exports of that
module to validate the import in the TypeScript file.

This is usually never a problem when trying to import a "regular" ES module, but
in some cases if the module has special packaging, or is a global _UMD_ module,
TypeScript's analysis of the module can fail and cause misleading errors. The
best thing to do in this situation is provide some form of types using one of
the methods mention above.

#### Internals

While it isn't required to understand how Deno works internally to be able to
leverage TypeScript with Deno well, it can help to understand how it works.

Before any code is executed or compiled, Deno generates a module graph by
parsing the root module, and then detecting all of its dependencies, and then
retrieving and parsing those modules, recursively, until all the dependencies
are retrieved.

For each dependency, there are two potential "slots" that are used. There is the
code slot and the type slot. As the module graph is filled out, if the module is
something that is or can be emitted to JavaScript, it fills the code slot, and
type only dependencies, like `.d.ts` files fill the type slot.

When the module graph is built, and there is a need to type check the graph,
Deno starts up the TypeScript compiler and feeds it the names of the modules
that need to be potentially emitted as JavaScript. During that process, the
TypeScript compiler will request additional modules, and Deno will look at the
slots for the dependency, offering it the type slot if it is filled before
offering it the code slot.

This means when you import a `.d.ts` module, or you use one of the solutions
above to provide alternative type modules for JavaScript code, that is what is
provided to TypeScript instead when resolving the module.
