// toObject enumerates only the allowlisted variables present in the
// environment, without a prompt. Credential-shaped variables are absent.
const obj = Deno.env.toObject();
console.log(`TERM present: ${"TERM" in obj}`);
console.log(`LANG present: ${"LANG" in obj}`);
console.log(`MY_APP_SECRET present: ${"MY_APP_SECRET" in obj}`);
console.log(`APP_TOKEN present: ${"APP_TOKEN" in obj}`);
