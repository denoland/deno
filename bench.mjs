const pattern = new URLPattern({ pathname: "/" });

console.log(pattern.exec({ pathname: "/" }));

Deno.bench({
  name: "URLPattern",
  fn() {
    pattern.exec({ pathname: "/" });
  },
});
