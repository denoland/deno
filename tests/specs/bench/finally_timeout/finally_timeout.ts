Deno.bench("error", function () {
  const timer = setTimeout(() => null, 10000);
  try {
    throw new Error("fail");
  } finally {
    clearTimeout(timer);
  }
});

Deno.bench("success", function () {
});
