Deno.test({
  name: "unresolved promise",
  fn() {
    return new Promise((_resolve, _reject) => {
      console.log("in promise");
    });
  },
});

Deno.test({
  name: "ok",
  fn() {
    console.log("ok test");
  },
});
