// The four Node compat variables (here NO_COLOR) are on the default allowlist,
// so they read without a prompt. They are now routed through the permission
// path instead of the old skip-check bypass, so --deny-env=NO_COLOR makes this
// read fail (see node_compat_deny.out).
try {
  console.log(`NO_COLOR: ${Deno.env.get("NO_COLOR")}`);
} catch (err) {
  const named = err.message.includes("--allow-env") ? "--allow-env" : "?";
  console.log(`NO_COLOR: ${err.name} ${named}`);
}
