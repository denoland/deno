Deno.test("custom stack getter", () => {
  const err = new Error("boom");
  Object.defineProperty(err, "stack", {
    get() {
      return "Error: boom\n    at fiberFrameA (effect://fiber:10:3)\n    at fiberFrameB (effect://fiber:20:5)";
    },
  });
  throw err;
});
