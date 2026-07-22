import mod = require("@denotest/dual-cjs-esm");

const kind: "other" = mod.getKind();
console.log(kind);
