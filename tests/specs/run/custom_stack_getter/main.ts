// Libraries like Effect provide enhanced stack traces through a custom `.stack`
// getter. Deno should display that custom stack instead of discarding it in
// favor of internal call-site frames. See https://github.com/denoland/deno/issues/35243
const err = new Error("boom");
Object.defineProperty(err, "stack", {
  get() {
    return "Error: boom\n    at customFrameA (effect://fiber:1:1)\n    at customFrameB (effect://fiber:2:2)";
  },
});
throw err;
