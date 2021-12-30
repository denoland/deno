const { notStrictEqual, strictEqual } = require("assert");

console.log(require.main === module);
notStrictEqual(require.main, module, "The module was loaded as a main module");
strictEqual(20, 20);
