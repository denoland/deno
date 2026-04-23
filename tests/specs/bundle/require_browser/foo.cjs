let arch;
if (typeof globalThis.window !== "undefined") {
  const os = require("node:os");
  arch = os.arch;
} else {
  arch = "browser";
}
module.exports = {
  hi: "hi",
  arch,
};
