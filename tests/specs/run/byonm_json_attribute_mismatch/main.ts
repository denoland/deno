Object.defineProperty(globalThis, "packageExecuted", {
  value: false,
  writable: true,
});

try {
  await import("npm:testpkg@1.0.0", { with: { type: "json" } });
  console.log("unexpected success");
  Deno.exit(1);
} catch (err) {
  console.log(err instanceof TypeError);
  console.log(String(err).includes("Expected a JSON module"));
  console.log("executed", globalThis.packageExecuted);
}
