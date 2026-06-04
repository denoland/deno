import { packageA } from "pkg-a";
import { packageB } from "pkg-b";
import { test } from "main-project";

Deno.test("cyclic workspace packages resolve through node_modules symlinks", () => {
  packageA();
  packageB();
  test();
});
