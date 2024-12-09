Disallows reassignment of imported module bindings

ES module import bindings should be treated as read-only since modifying them
during code execution will likely result in runtime errors. It also makes for
poor code readability and difficult maintenance.

### Invalid:

```typescript
import defaultMod, { namedMod } from "./mod.js";
import * as modNameSpace from "./mod2.js";

defaultMod = 0;
namedMod = true;
modNameSpace.someExportedMember = "hello";
modNameSpace = {};
```

### Valid:

```typescript
import defaultMod, { namedMod } from "./mod.js";
import * as modNameSpace from "./mod2.js";

// properties of bound imports may be set
defaultMod.prop = 1;
namedMod.prop = true;
modNameSpace.someExportedMember.prop = "hello";
```
