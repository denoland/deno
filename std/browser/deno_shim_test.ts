import { getDenoShim } from "./deno_shim.ts";
import { assert } from "../testing/asserts.ts";

const { test } = Deno;

const denoShim = await getDenoShim();

test({
  name: "denoShim equality",
  fn() {
    assert(denoShim !== Deno);
  },
});

test({
  name: "property match",
  fn() {
    for (const key of Object.keys(Deno)) {
      assert(key in denoShim, `${key} should be in denoShim`);
      assert(
        typeof denoShim[key as keyof typeof denoShim] ===
          typeof Deno[key as keyof typeof Deno],
        `Types of property ${key} should match.`,
      );
    }
  },
});
