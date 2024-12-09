Warns the wrong usage of triple-slash reference directives.

Deno supports the triple-slash reference directives of `types`, `path`, `lib`,
and `no-default-lib`. This lint rule checks if there is an invalid, badly-formed
directive because it is most likely a mistake.

Additionally, note that only the `types` directive is allowed in JavaScript
files. This directive is useful for telling the TypeScript compiler the location
of a type definition file that corresponds to a certain JavaScript file.
However, even in the Deno manual of the versions prior to v1.10 (e.g. [v1.9.2]),
there was a wrong statement describing that one should use the `path` directive
in such cases. Actually, the `types` directive should be used. See
[the latest manual] for more detail. So this rule also detects the usage of the
directive other than `types` in JavaScript files and suggests replacing it with
the `types` directive.

[v1.9.2]: https://deno.land/manual@v1.9.2/typescript/types#using-the-triple-slash-reference-directive
[the latest manual]: https://deno.land/manual/typescript/types#using-the-triple-slash-reference-directive

### Invalid:

#### JavaScript

```javascript
/// <reference path="./mod.d.ts" />
/// <reference no-default-lib="true" />
/// <reference foo="bar" />

// ... the rest of the JavaScript ...
```

#### TypeScript

```typescript
/// <reference foo="bar" />

// ... the rest of the TypeScript ...
```

### Valid:

#### JavaScript

```javascript
/// <reference types="./mod.d.ts" />
/// <reference lib="es2017.string" />

// ... the rest of the JavaScript ...
```

#### TypeScript

```typescript
/// <reference types="./mod.d.ts" />
/// <reference path="./mod.d.ts" />
/// <reference lib="es2017.string" />
/// <reference no-default-lib="true" />

// ... the rest of the TypeScript ...
```
