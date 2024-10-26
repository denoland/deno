Deno.bench("console.log", function () {
  console.log("log");
});

Deno.bench("console.error", function () {
  console.error("error");
});

Deno.bench("console.info", function () {
  console.info("info");
});

Deno.bench("console.warn", function () {
  console.info("warn");
});
