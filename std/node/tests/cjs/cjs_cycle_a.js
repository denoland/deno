// deno-lint-ignore-file no-undef
module.exports = false;
require("./cjs_cycle_a");
module.exports = true;
