const Module = require("module");

// simulate what pirates/esbuild-register does: hook _compile
const originalCompile = Module.prototype._compile;
Module.prototype._compile = function (content, filename, format) {
  if (typeof content !== "string") {
    throw new TypeError(
      "content passed to _compile should be a string, got " + typeof content,
    );
  }
  return originalCompile.call(this, content, filename, format);
};

require("./lib.ts");
