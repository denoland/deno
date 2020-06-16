# std/hash

## Usage

### Creating new hash instance

You can create a new Hasher instance by calling `createHash` defined in mod.ts.

```ts
import { createHash } from "https://deno.land/std/hash/mod.ts";

const hash = createHash("md5");
// ...
```

### Using hash instance

You can use `update` method to feed data into your hash instance. Call `digest`
method to retrive final hash value in ArrayBuffer.

```ts
import { createHash } from "https://deno.land/std/hash/mod.ts";

const hash = createHash("md5");
hash.update("Your data here");
const final = hash.digest(); // returns ArrayBuffer
```

Please note that `digest` invalidates the hash instance's internal state.
Calling `digest` more than once will throw an Error.

```ts
import { createHash } from "https://deno.land/std/hash/mod.ts";

const hash = createHash("md5");
hash.update("Your data here");
const final1 = hash.digest(); // returns ArrayBuffer
const final2 = hash.digest(); // throws Error
```

If you need final hash in string formats, call `toString` method with output
format.

Supported formats are `hex` and `base64` and default format is `hex`.

```ts
import { createHash } from "https://deno.land/std/hash/mod.ts";

const hash = createHash("md5");
hash.update("Your data here");
const hashInHex = hash.toString(); // returns 5fe084ee423ff7e0c7709e9437cee89d
```

```ts
import { createHash } from "https://deno.land/std/hash/mod.ts";

const hash = createHash("md5");
hash.update("Your data here");
const hashInBase64 = hash.toString("base64"); // returns X+CE7kI/9+DHcJ6UN87onQ==
```

### Supported algorithms

Following algorithms are supported.

- md2
- md4
- md5
- ripemd160
- ripemd320
- sha1
- sha224
- sha256
- sha384
- sha512
- sha3-224
- sha3-256
- sha3-384
- sha3-512
- keccak224
- keccak256
- keccak384
- keccak512
