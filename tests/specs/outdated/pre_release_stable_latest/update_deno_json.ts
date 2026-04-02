Deno.writeTextFileSync(
  "deno.json",
  JSON.stringify({
    imports: {
      "@denotest/pre-release-stable-latest":
        "npm:@denotest/pre-release-stable-latest@^1.0.0-beta.13",
    },
  }),
);
