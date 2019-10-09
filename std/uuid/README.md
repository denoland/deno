# UUID

Support for version 1, 3, 4, and 5 UUIDs.

## Usage

```ts
import uuid, { validate } from "https://deno.land/std/uuid/mod.ts";

// Generate a v4 uuid
const myUUID = uuid();

// Validate a v4 uuid
const isValid = validate(aString);
```
