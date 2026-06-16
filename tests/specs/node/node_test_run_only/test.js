// deno-lint-ignore-file

import test from "node:test";

test("runOnly filters subtests", { only: true }, async (t) => {
  t.runOnly(true);
  await t.test("this subtest is skipped");
  await t.test("this subtest is run", { only: true });
});

test("runOnly(false) runs every subtest", { only: true }, async (t) => {
  t.runOnly(true);
  t.runOnly(false);
  await t.test("sub a");
  await t.test("sub b", { only: true });
});
