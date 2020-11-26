# UUID

Support for version 1, 4, and 5 UUIDs.

## Usage

```ts
import { v4 } from "https://deno.land/std@$STD_VERSION/uuid/mod.ts";

// Generate a v4 uuid.
const myUUID = v4.generate();

// Validate a v4 uuid.
const isValid = v4.validate(myUUID);
```
