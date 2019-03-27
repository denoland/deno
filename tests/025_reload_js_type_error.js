// There was a bug where if this was executed with --recompile it would throw a
// type error.
window.test = null;
test = console;
test.log("hello");
