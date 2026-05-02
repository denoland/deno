const { default: data } = await import("./data.json", {
  assert: { type: "json" },
});
await import("./relative_doesnt_exist.ts");
await import("not_real");