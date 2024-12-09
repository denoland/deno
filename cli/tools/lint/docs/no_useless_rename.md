Disallow useless rename operations where both the original and new name are
exactly the same. This is often a leftover from a refactoring procedure and can
be safely removed.

### Invalid:

```ts
import { foo as foo } from "foo";
const { foo: foo } = obj;
export { foo as foo };
```

### Valid:

```ts
import { foo as bar } from "foo";
const { foo: bar } = obj;
export { foo as bar };
```
