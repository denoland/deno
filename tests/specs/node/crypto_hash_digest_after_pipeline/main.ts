// Regression test for https://github.com/denoland/deno/issues/33264
// `hash.digest(encoding)` must succeed after the Hash has been used as the
// Transform destination of `stream.pipeline`. Previously the hash state was
// consumed inside `_flush`, so the user's explicit digest call threw
// `ERR_CRYPTO_HASH_FINALIZED`. Node caches the digest after the first call,
// so multiple calls (in different encodings) return the same value.

import { createHash } from "node:crypto";
import { pipeline, Readable } from "node:stream";

const input = Readable.from(["hello"]);
const hash = createHash("sha256");

await new Promise<void>((resolve, reject) => {
  pipeline(input, hash, (err) => {
    if (err) {
      reject(err);
      return;
    }
    resolve();
  });
});

console.log("digest hex:", hash.digest("hex"));
// Calling digest again in a different encoding must not throw: Node caches
// the digest after the first call and returns the same value in the requested
// encoding.
console.log("digest base64:", hash.digest("base64"));
