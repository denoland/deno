// In the strict default profile (relaxed gate off), the four always-on Node
// compat variables stay readable without a prompt. NO_COLOR is one of them; an
// unlisted variable such as TERM still fails naming --allow-env. With
// --deny-env=NO_COLOR the always-on read fails too, since deny wins.
for (const key of ["NO_COLOR", "TERM"]) {
  try {
    console.log(`${key}: ${Deno.env.get(key)}`);
  } catch (err) {
    const named = err.message.includes("--allow-env") ? "--allow-env" : "?";
    console.log(`${key}: ${err.name} ${named}`);
  }
}
