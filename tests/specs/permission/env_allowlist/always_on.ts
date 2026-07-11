// The four always-on Node compat variables stay readable without a prompt even
// in the strict default profile (relaxed gate off), because they are folded
// into the default allow_env descriptors. Only readability is asserted (the
// values are left empty) so that setting them cannot perturb runtime behavior.
for (const key of ["FORCE_COLOR", "NODE_DEBUG", "NODE_OPTIONS", "NO_COLOR"]) {
  try {
    Deno.env.get(key);
    console.log(`${key}: readable`);
  } catch (err) {
    const named = err.message.includes("--allow-env") ? "--allow-env" : "?";
    console.log(`${key}: ${err.name} ${named}`);
  }
}
