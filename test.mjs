import { test } from "node:test";

test("test", async (t) => {
  console.log("context 1", t);
  await t.test("nested 1", async (t) => {
    console.log("context 2", t);
    await t.test("nested 2", () => {
      console.log("OK!");
    });
  });
});
