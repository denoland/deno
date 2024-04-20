const info = await import("npm:@denotest/binary-package@1/package.json", {
  assert: { type: "json" },
});
console.log(json);
