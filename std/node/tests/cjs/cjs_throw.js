function hello() {
  throw new Error("bye");
}

// deno-lint-ignore no-undef
module.exports = { hello };
