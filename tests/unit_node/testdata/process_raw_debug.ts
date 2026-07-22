import process from "node:process";

//deno-lint-ignore no-undef
// @ts-ignore - this is a private API in Node, but some packages depend on it
process._rawDebug("this should go to stderr", { a: 1, b: ["a", 2] });
