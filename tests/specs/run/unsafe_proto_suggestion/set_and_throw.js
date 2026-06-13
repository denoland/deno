// Assigning to `__proto__` is a silent no-op while the accessor is disabled
// (the default). Deno records that it happened so that, if the program then
// crashes, the uncaught-error formatter can suggest `--unstable-unsafe-proto`.
const obj = {};
obj.__proto__ = { polluted: true };

throw new Error("boom");
