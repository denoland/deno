// --allow-env=APP_SECRET composes with the default allowlist: the explicitly
// granted APP_SECRET and the allowlisted TERM both read without a prompt.
for (const key of ["APP_SECRET", "TERM"]) {
  try {
    console.log(`${key}: ${Deno.env.get(key)}`);
  } catch (err) {
    const named = err.message.includes("--allow-env") ? "--allow-env" : "?";
    console.log(`${key}: ${err.name} ${named}`);
  }
}
