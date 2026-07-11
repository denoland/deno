// Reading allowlisted variables (including prefix-pattern members) succeeds
// without a prompt. Under --deny-env=TERM the TERM read instead fails naming
// --allow-env (see deny_term.out).
for (const key of ["TERM", "LC_ALL", "XDG_CONFIG_HOME", "npm_package_name"]) {
  try {
    console.log(`${key}: ${Deno.env.get(key)}`);
  } catch (err) {
    const named = err.message.includes("--allow-env") ? "--allow-env" : "?";
    console.log(`${key}: ${err.name} ${named}`);
  }
}
