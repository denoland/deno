addEventListener("beforeunload", (e) => {
  // The worker should be killed once benchmarks are done regardless of this.
  e.preventDefault();
});

Deno.bench("foo", () => {});
