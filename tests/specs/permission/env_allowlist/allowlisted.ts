// Reading allowlisted variables succeeds without a prompt. This covers a
// representative from each category: terminal (TERM), the LC_* locale wildcard
// (LC_ALL), the XDG_* wildcard (XDG_CONFIG_HOME), a `deno task` npm var
// (npm_package_name), the DEBUG_* wildcard (DEBUG_MYAPP), a CI detection var
// (GITHUB_ACTIONS) and the shell argv0 var (_). Under --deny-env=TERM the TERM
// read instead fails naming --allow-env (see deny_term.out).
for (
  const key of [
    "TERM",
    "LC_ALL",
    "XDG_CONFIG_HOME",
    "npm_package_name",
    "DEBUG_MYAPP",
    "GITHUB_ACTIONS",
    "_",
  ]
) {
  try {
    console.log(`${key}: ${Deno.env.get(key)}`);
  } catch (err) {
    const named = err.message.includes("--allow-env") ? "--allow-env" : "?";
    console.log(`${key}: ${err.name} ${named}`);
  }
}
