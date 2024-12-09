Disallows the `assert` keyword for import attributes

ES import attributes (previously called import assertions) has been changed to
use the `with` keyword. The old syntax using `assert` is still supported, but
deprecated.

### Invalid:

```typescript
import obj from "./obj.json" assert { type: "json" };
import("./obj2.json", { assert: { type: "json" } });
```

### Valid:

```typescript
import obj from "./obj.json" with { type: "json" };
import("./obj2.json", { with: { type: "json" } });
```
