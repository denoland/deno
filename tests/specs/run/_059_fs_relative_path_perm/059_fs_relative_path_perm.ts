// The permission error message shouldn't include the CWD.
Deno.readFileSync("non-existent");
