import { test } from "node:test";

test("test", async (t) => {
  await t.test("nested 1", async (t) => {
    await t.test("nested 2", () => {
      console.log("OK!");
    });
  });
});
