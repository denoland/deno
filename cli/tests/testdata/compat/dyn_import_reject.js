import("./foobar.js").catch((e) => {
  console.log(e);
  console.log(e.code);
});
