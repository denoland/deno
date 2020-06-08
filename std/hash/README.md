# std/hash

## MD5

**Uses:**

```ts
import { Md5 } from "https://deno.land/std/hash/md5.ts";

const md5 = new Md5();
const md5Instance = md5.update("中文"); // return instance of `Md5`
console.log(md5Instance instanceof Md5); // true
console.log(md5Instance.toString()); // a7bac2239fcdcb3a067903d8077c4a07
```

Calling `update` method, It will update internal state based on the input
provided. Once you call `md5Instance.toString()`, it will return the
`hash string`. You can provide format as `hash` or `base64`. The default format
is `hex`.

**sample:**

```ts
console.log(md5Instance.toString("base64")); // MNgWOD+FHGO3Fff/HDCY2w==
```

## SHA1

**Uses:**

Creating `sha1` hash is simple. You can use `Sha1` class instance and update the
digest. Calling `hex` method will return the sha1 in hex value. You can also use
`toString` method.

```ts
import { Sha1 } from "https://deno.land/std/hash/sha1.ts";

const sha1 = new Sha1().update("中文");
console.log(sha1.hex()); // 7be2d2d20c106eee0836c9bc2b939890a78e8fb3
console.log(sha1.toString()); // same as above
```

## Sha256 and HmacSha256

**Uses:**

Creating `Sha256` hash is simple. You can use `Sha256` class instance and update
the digest. Calling the `hex` method will return the sha256 in `hex` value. You
can also use the `toString` method.

**Note:** For `HmacSha256`, you can pass the secret `key` while creating an
instance of the object.

```ts
import { Sha256, HmacSha256 } from "https://deno.land/std/hash/sha256.ts";

const sha256 = new Sha256().update("中文");
console.log(sha256.hex());
console.log(sha256.toString()); // Same as above

const key = "Hi There";
const hmac = new HmacSha256(key).update("中文");

console.log(hmac.hex());
console.log(hmac.toString()); // Same as above
```
