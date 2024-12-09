Disallows the use of external imports

- what's the motivation of this lint rule?
  - this rule emits warnings if external modules are imported via URL. "deps.ts"
    and import maps are exception.
- why is linted code considered bad?
  - importing external modules just works fine, but it will take time and effort
    when you want to upgrade those modules if they are imported in multiple
    places in your project.
- who should use it?
  - to avoid it you could use "deps.ts convention" or
    [import maps](https://docs.deno.com/runtime/manual/basics/import_maps),
    where you import all external modules and then re-export them or assign
    aliases to them.
  - so if you'd like to follow the "deps.ts convention" or to use import maps,
    this rule is for you.

### Invalid:

```typescript
import { assertEquals } from "https://deno.land/std@0.126.0/testing/asserts.ts";
```

### Valid:

```typescript
import { assertEquals } from "./deps.ts";
```

```typescript
// deps.ts

export {
  assert,
  assertEquals,
  assertStringIncludes,
} from "https://deno.land/std@0.126.0/testing/asserts.ts";
```

you can refer to the explanation of this convention here
https://docs.deno.com/runtime/manual/basics/modules/#it-seems-unwieldy-to-import-urls-everywhere
