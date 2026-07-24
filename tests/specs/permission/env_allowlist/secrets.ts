// Credential-shaped variables are never on the allowlist, so reading them
// still fails non-interactively naming --allow-env. `npm_config__authToken`
// shows that the npm_config_* namespace is not wildcarded.
for (const key of ["AWS_SECRET_ACCESS_KEY", "npm_config__authToken"]) {
  try {
    Deno.env.get(key);
    console.log(`${key}: UNEXPECTEDLY ALLOWED`);
  } catch (err) {
    const named = err.message.includes("--allow-env") ? "--allow-env" : "?";
    console.log(`${key}: ${err.name} ${named}`);
  }
}
