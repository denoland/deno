Disallows the use of NodeJS global objects.

NodeJS exposes a set of global objects that differs from deno (and the web), so
code should not assume they are available. Instead, import the objects from
their defining modules as needed.

### Invalid:

```typescript
// foo.ts
const buf = Buffer.from("foo", "utf-8"); // Buffer is not a global object in deno
```

### Valid:

```typescript
// foo.ts
import { Buffer } from "node:buffer";

const foo = Buffer.from("foo", "utf-8");
```
