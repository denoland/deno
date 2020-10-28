import {
  NormalizedAlgorithms,
  pbkdf2,
  pbkdf2Sync,
} from "./crypto.ts";
import {
  assert,
  assertEquals,
} from "../testing/asserts.ts";
import pbkdf2Fixtures from "./_crypto/pbkdf2.ts";

Deno.test("pbkdf2 hashes data correctly", () => {
  pbkdf2Fixtures.forEach(({
    dkLen,
    iterations,
    key,
    results,
    salt,
  }) => {
    for(const algorithm in results){
      pbkdf2(key, salt, iterations, dkLen, algorithm as NormalizedAlgorithms, (err, res) => {
        assert(!err);
        assertEquals(
          res?.toString('hex'),
          results[algorithm as NormalizedAlgorithms],
        );
      });
    }
  });
});

Deno.test("pbkdf2Sync hashes data correctly", () => {
  pbkdf2Fixtures.forEach(({
    dkLen,
    iterations,
    key,
    results,
    salt,
  }) => {
    for(const algorithm in results){
      assertEquals(
        pbkdf2Sync(key, salt, iterations, dkLen, algorithm as NormalizedAlgorithms).toString('hex'),
        results[algorithm as NormalizedAlgorithms],
      );
    }
  });
});