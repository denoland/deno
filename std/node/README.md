# Deno Node compatibility

This module is meant to have a compatibility layer for the
[nodeJS standard library](https://nodejs.org/docs/latest-v12.x/api/).

**Warning** : Any function of this module should not be referred anywhere in the
deno standard library as it's a compatiblity module.

## CommonJS Module Loading

`createRequire(...)` is provided to create a `require` function for loading CJS
modules.

```ts
import { createRequire } from "https://deno.land/std/node/module.ts";

const require_ = createRequire(import.meta.url);
// Loads native module polyfill.
const path = require_("path");
// Loads extensionless module.
const cjsModule = require_("./my_mod");
// Visits node_modules.
const leftPad = require_("left-pad");
```
