// Runs with -A so the profile is off. Creates the .git/hooks directory that the
// confined main.ts run then attempts (and fails) to write into.
Deno.mkdirSync(".git/hooks", { recursive: true });
console.log("setup: ok");
