# std/types

Contains types for popular external packages that are compatible with Deno.

Because Deno only resolves fully qualified file names, type definitions that
import other type definitions might not work with Deno. Also, when some type
definition supply some global interfaces, they can conflict with Deno. The types
located here have been validated to work with Deno.

The types that are currently available:

- `react.d.ts` - For React 16. Sources known to work well for Deno:
  - Pika CDN: `https://cdn.pika.dev/_/react/v16`
  - JSPM: `https://dev.jspm.io/react@16`
- `react-dom.d.ts` - For ReactDOM 16. Sources known to work well for Deno:
  - Pika CDN: `https://cdn.pika.dev/_/react-dom/v16`
  - JSPM: `https://dev.jspm.io/react-dom@16`

There are several ways these type definitions can be referenced. Likely the
"best" way is that the CDN provider provides a header of `X-TypeScript-Types`
which points to the type definitions. We are working to have this available, but
currently you would need to use the compiler hint of `@deno-types`. For example
to import React:

```ts
// @deno-types="https://deno.land/std/types/react.d.ts"
import React from "https://cdn.pika.dev/_/react/v16";
```
