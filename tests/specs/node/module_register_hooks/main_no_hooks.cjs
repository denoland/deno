const { registerHooks } = require("node:module");

// A hooks object without `resolve` or `load` is valid and is a no-op,
// matching Node.js (https://github.com/denoland/deno/issues/35151).
const hook = {
  printFunc(data) {
    console.log(data);
  },
};
const { deregister } = registerHooks(hook);
deregister();
console.log("ok");

// A provided hook that is not a function must still throw.
try {
  registerHooks({ resolve: "not a function" });
} catch (e) {
  console.log("threw:", e.code);
}
