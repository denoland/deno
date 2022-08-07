// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
// deno-lint-ignore no-undef
const processMod = require("process");
const osMod = require("node:os");
console.log("process", processMod);
console.log("os", osMod);
const leftPad = require("left-pad");
const json = require("./data");
console.log(json);
console.log(leftPad("foo", 5)); // => "  foo"
