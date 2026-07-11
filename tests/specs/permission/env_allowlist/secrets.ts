// Credential-shaped variables are never on the allowlist, so reading them
// still fails non-interactively naming --allow-env. `npm_config__authToken`
// shows that the npm_config_* namespace is not wildcarded, `GITHUB_TOKEN` that
// the GITHUB_* namespace is not wildcarded (only the boolean GITHUB_ACTIONS is
// listed), and `APP_SECRET` that an arbitrary unlisted variable is denied.
for (
  const key of [
    "AWS_SECRET_ACCESS_KEY",
    "npm_config__authToken",
    "GITHUB_TOKEN",
    "APP_SECRET",
  ]
) {
  try {
    Deno.env.get(key);
    console.log(`${key}: UNEXPECTEDLY ALLOWED`);
  } catch (err) {
    const named = err.message.includes("--allow-env") ? "--allow-env" : "?";
    console.log(`${key}: ${err.name} ${named}`);
  }
}
