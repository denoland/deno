import process from "node:process";

//deno-lint-ignore no-undef
// @ts-ignore - Node typings don't even have this because it's
// been deprecated for 4 years. But it's used in `signal-exit`,
// which in turn is used in `node-tap`.
process.reallyExit = function () {
  console.info("really exited");
};
process.exit();
