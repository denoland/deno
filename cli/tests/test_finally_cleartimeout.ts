Deno.test("test", function() {
  const timer = setTimeout(() => null, 10000);
  try {
    throw new Error("fail");
  } finally {
    clearTimeout(timer);
  }
});
