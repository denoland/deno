Enforces type imports to be declared as type imports.

This rule ensures that the code works when the `verbatimModuleSyntax` TypeScript
compiler option is enabled. This is useful in libraries distributing TypeScript
code in order to work in more scenarios.

### Invalid:

```typescript
import { Person } from "./person.ts";

const person: Person = {
  name: "David",
};
console.log(person);
```

```typescript
import { output, Person } from "./person.ts";

const person: Person = {
  name: "David",
};
output(person);
```

### Valid:

```typescript
import type { Person } from "./person.ts";

const person: Person = {
  name: "David",
};
console.log(person);
```

```typescript
import { output, type Person } from "./person.ts";

const person: Person = {
  name: "David",
};
output(person);
```
