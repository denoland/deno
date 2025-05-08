import { test } from "node:test";

test("todo() method with message", async (t) => {
  t.todo("this is a todo test and is not treated as a failure");
  await t.test("test", () => {
    throw new Error("this does not fail the test");
  });
  throw new Error("this does not fail the test");
});

test("skip", async (t) => {
  t.skip("this is a skip test and is not treated as a failure");
  await t.test("test", () => {
    throw new Error("this does not fail the test");
  });
  throw new Error("this does not fail the test");
});
