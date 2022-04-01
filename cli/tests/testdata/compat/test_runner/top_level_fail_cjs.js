const { notStrictEqual, strictEqual } = require("assert");

notStrictEqual(require.main, module, "The module was loaded as a main module");
strictEqual(10, 20);
