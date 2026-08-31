try {
  await import("./main.cjs");
  console.log("unexpected success");
  Deno.exit(1);
} catch (err) {
  const message = err instanceof Error
    ? `${err.message}\n${err.stack ?? ""}`
    : String(err);
  if (message.includes("DENO_CJS_ANALYZER_SOURCE_CANARY")) {
    console.log("unexpected source");
    Deno.exit(1);
  }
  if (message.includes("Requires read access")) {
    console.log("blocked");
  } else {
    console.log("unexpected error");
    Deno.exit(1);
  }
}
