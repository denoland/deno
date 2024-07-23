Enforces specifying explicit references to paths in module specifiers.

Non-explicit specifiers are ambiguous and require probing for the correct file
path on every run, which has a performance overhead.

### Invalid:

```typescript
import { add } from "./math/add";
import { ConsoleLogger } from "./loggers";
```

### Valid:

```typescript
import { add } from "./math/add.ts";
import { ConsoleLogger } from "./loggers/index.ts";
```
