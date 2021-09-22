Deno.test("capturing", async (t) => {
  let capturedTester!: Deno.Tester;
  await t.step("some step", (t) => {
    capturedTester = t;
  });
  // this should error because the scope of the tester has already completed
  await capturedTester.step("next step", () => {});
});
