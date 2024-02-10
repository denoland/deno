Deno.test("description", async (t) => {
  // the output is not great, but this is an extreme scenario
  console.log(1);
  await t.step("step 1", async (t) => {
    console.log(2);
    await t.step("inner 1", () => {
      console.log(3);
    });
    await t.step("inner 2", () => {
      console.log(4);
    });
    console.log(5);
  });
  console.log(6);
});
