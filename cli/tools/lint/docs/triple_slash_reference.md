Disallow certain triple slash directives in favor of ES6-style import
declarations

TypeScript's `///` triple-slash references are a way to indicate that types from
another module are available in a file. Use of triple-slash reference type
directives is generally discouraged in favor of ECMAScript Module imports. This
rule reports on the use of `/// <reference path="..." />`,
`/// <reference types="..." />`, or `/// <reference lib="..." />` directives.

### Invalid:

```typescript
/// <reference types="foo" />
import * as foo from "foo";
```

### Valid:

```typescript
import * as foo from "foo";
```
