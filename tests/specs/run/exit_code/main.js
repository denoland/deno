if (Deno.exitCode != 0) {
  throw new Error("boom!");
}

Deno.exitCode = 42;

console.log("Deno.exitCode", Deno.exitCode);
