## Frequently asked questions

### Getting errors when type checking like `cannot find namespace NodeJS`

One of the modules you are using has type definitions that depend upon the
NodeJS global namespace, but those types don't include the NodeJS global
namespace in their types.

The quickest fix is to skip type checking. You can do this by using the
`--no-check` flag.

Skipping type checking might not be acceptable though. You could try to load the
Node.js types yourself. For example from UNPKG it would look something like
this:

```ts
import type {} from "https://unpkg.com/@types/node/index.d.ts";
```

Or from esm.sh:

```ts
import type {} from "https://esm.sh/@types/node/index.d.ts";
```

Or from Skypack:

```ts
import type {} from "https://cdn.skypack.dev/@types/node/index.d.ts";
```

You could also try to provide only specifically what the 3rd party package is
missing. For example the package `@aws-sdk/client-dynamodb` has a dependency on
the `NodeJS.ProcessEnv` type in its type definitions. In one of the modules of
your project that imports it as a dependency, you could put something like this
in there which will solve the problem:

```ts
declare global {
  namespace NodeJS {
    type ProcessEnv = Record<string, string>;
  }
}
```

### Getting type errors like cannot find `document` or `HTMLElement`

The library you are using has dependencies on the DOM. This is common for
packages that are designed to run in a browser as well as server-side. By
default, Deno only includes the libraries that are directly supported. Assuming
the package properly identifies what environment it is running in at runtime it
is "safe" to use the DOM libraries to type check the code. For more information
on this, check out the
[Targeting Deno and the Browser](../typescript/configuration.md#targeting-deno-and-the-browser)
section of the manual.
