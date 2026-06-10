if (!Deno.args.some((a) => a.includes("minimum-dependency-age"))) {
  Deno.writeTextFileSync(
    "deno.json",
    JSON.stringify({
      "minimumDependencyAge": "2025-05-01T20:00:00.000Z",
    }),
  );
}
