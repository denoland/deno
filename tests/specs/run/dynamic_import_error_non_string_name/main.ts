// Regression test for GHSA-2f8r-ppr9-ff8f
// Dynamic import with non-string error name should not panic

try {
  await import(`data:application/javascript,
    const e = new Error("test error");
    Object.defineProperty(e, "name", { value: 42 });
    throw e;
  `);
} catch (e) {
  console.log("caught error");
}

console.log("done");
