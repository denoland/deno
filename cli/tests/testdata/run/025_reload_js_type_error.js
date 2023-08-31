// deno-lint-ignore-file
// There was a bug where if this was executed with --reload it would throw a
// type error.
window.test = null;
test = console;
test.log("hello");
