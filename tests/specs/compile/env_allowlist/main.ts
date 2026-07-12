// A compiled binary applies the default-readable env var allowlist too, for
// parity with `deno run`. TERM is allowlisted and reads without a prompt;
// SOME_SECRET is not allowlisted and is denied. When compiled with
// --deny-env=TERM, TERM is denied as well (deny wins in the compiled app).
for (const key of ["TERM", "SOME_SECRET"]) {
  try {
    console.log(`${key}: ${Deno.env.get(key) ?? "<unset>"}`);
  } catch (err) {
    console.log(`${key}: ${(err as Error).name}`);
  }
}
