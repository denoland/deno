// A test with steps that is repeated runs its steps once per repetition. The
// summary must count the step once, not once per repetition.
Deno.test({
  name: "repeated with steps",
  repeats: 3,
  fn: async (t) => {
    await t.step("inner", () => {});
  },
});
