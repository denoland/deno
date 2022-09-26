// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
// deno-lint-ignore no-undef
const processMod = require("process");
const osMod = require("node:os");
console.log("process.pid", processMod.pid);
console.log("os.EOL", osMod.EOL);
const leftPad = require("left-pad");
const json = require("./data");
console.log(json);
console.log(leftPad("foo", 5)); // => "  foo"
console.log("main module", processMod.mainModule.filename);
