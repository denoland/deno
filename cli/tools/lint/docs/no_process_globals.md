Disallows the use of NodeJS `process` global.

NodeJS and Deno expose `process` global but they are hard to statically analyze
by tools, so code should not assume they are available. Instead,
`import process from "node:process"`.

### Invalid:

```typescript
// foo.ts
const foo = process.env.FOO;
```

### Valid:

```typescript
// foo.ts
import process from "node:process";

const foo = process.env.FOO;
```
