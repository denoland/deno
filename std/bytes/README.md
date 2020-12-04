# bytes

bytes module is made to provide helpers to manipulation of bytes slice.

# usage

All the following functions are exposed in `mod.ts`.

## indexOf

Find first index of binary pattern from given binary array.

```typescript
import { indexOf } from "https://deno.land/std@$STD_VERSION/bytes/mod.ts";

indexOf(
  new Uint8Array([1, 2, 0, 1, 2, 0, 1, 2, 0, 1, 3]),
  new Uint8Array([0, 1, 2]),
);

// => returns 2
```

## lastIndexOf

Find last index of binary pattern from given binary array.

```typescript
import { lastIndexOf } from "https://deno.land/std@$STD_VERSION/bytes/mod.ts";

lastIndexOf(
  new Uint8Array([0, 1, 2, 0, 1, 2, 0, 1, 3]),
  new Uint8Array([0, 1, 2]),
);

// => returns 3
```

## equals

Check whether given binary arrays are equal to each other.

```typescript
import { equals } from "https://deno.land/std@$STD_VERSION/bytes/mod.ts";

equals(new Uint8Array([0, 1, 2, 3]), new Uint8Array([0, 1, 2, 3])); // returns true
equals(new Uint8Array([0, 1, 2, 3]), new Uint8Array([0, 1, 2, 4])); // returns false
```

## startsWith

Check whether binary array starts with prefix.

```typescript
import { startsWith } from "https://deno.land/std@$STD_VERSION/bytes/mod.ts";

startsWith(new Uint8Array([0, 1, 2]), new Uint8Array([0, 1])); // returns true
startsWith(new Uint8Array([0, 1, 2]), new Uint8Array([1, 2])); // returns false
```

## endsWith

Check whether binary array ends with suffix.

```typescript
import { endsWith } from "https://deno.land/std@$STD_VERSION/bytes/mod.ts";

endsWith(new Uint8Array([0, 1, 2]), new Uint8Array([0, 1])); // returns false
endsWith(new Uint8Array([0, 1, 2]), new Uint8Array([1, 2])); // returns true
```

## repeat

Repeat bytes of given binary array and return new one.

```typescript
import { repeat } from "https://deno.land/std@$STD_VERSION/bytes/mod.ts";

repeat(new Uint8Array([1]), 3); // returns Uint8Array(3) [ 1, 1, 1 ]
```

## concat

Concatenate multiple binary arrays and return new one.

```typescript
import { concat } from "https://deno.land/std@$STD_VERSION/bytes/mod.ts";

concat(new Uint8Array([1, 2]), new Uint8Array([3, 4])); // returns Uint8Array(4) [ 1, 2, 3, 4 ]

concat(
  new Uint8Array([1, 2]),
  new Uint8Array([3, 4]),
  new Uint8Array([5, 6]),
  new Uint8Array([7, 8]),
); // => returns Uint8Array(8) [ 1, 2, 3, 4, 5, 6, 7, 8 ]
```

## contains

Check source array contains pattern array.

```typescript
import { contains } from "https://deno.land/std@$STD_VERSION/bytes/mod.ts";

contains(
  new Uint8Array([1, 2, 0, 1, 2, 0, 2, 1, 3]),
  new Uint8Array([0, 1, 2]),
); // => returns true

contains(
  new Uint8Array([1, 2, 0, 1, 2, 0, 2, 1, 3]),
  new Uint8Array([2, 2]),
); // => returns false
```

## copy

Copy bytes from one binary array to another.

```typescript
import { copy } from "https://deno.land/std@$STD_VERSION/bytes/mod.ts";

const dst = new Uint8Array(4);
const src = Uint8Array.of(1, 2, 3, 4);
const len = copy(src, dest); // returns len = 4
```
