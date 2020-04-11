# std/types

Contains types for popular external packages that are compatible with Deno.

Because Deno only resolves fully qualified file names, type definitions that
import other type definitions might not work with Deno. Also, when some type
definition supply some global interfaces, they can conflict with Deno. The types
located here have been validated to work with Deno.

There are several ways these type definitions can be referenced. Likely the
"best" way is that the CDN provider provides a header of `X-TypeScript-Types`
which points to the type definitions. We are working to have this available, but
currently you would need to use the compiler hint of `@deno-types`. For example
to import React:

```ts
// @deno-types="https://deno.land/std/types/react/@16.9.0/react.d.ts"
import React from "https://cdn.pika.dev/_/react@v16.9.0";
```

or

```ts
// @deno-types="https://deno.land/std/types/react/@16.9.0/react.d.ts"
import React from "https://dev.jspm.io/react@16.9.0";
```

#### Notes:

JSPM version of most libraries export everything through the default namespace,
so most of the time it might not be suited to work along with this definition
library.
