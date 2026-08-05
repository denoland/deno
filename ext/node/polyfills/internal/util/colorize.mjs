// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = __bootstrap;
const { op_get_env_no_permission_check } = core.ops;
const { String } = primordials;

// Reads an env var used for internal color detection without a permission
// check. These are non-secret terminal/color markers and this is internal
// runtime configuration, so `util.inspect(x, { colors: true })` / `styleText`
// keep working even under `--deny-env` (mirrors the NODE_OPTIONS bootstrap
// read). Returns undefined for an unset var, matching a plain `process.env`
// read. Reads the live OS env, which is what `process.env.X = ...` writes to,
// so node_compat tests that flip these still observe their changes.
function colorEnv(name) {
  const value = op_get_env_no_permission_check(name);
  return value === null ? undefined : value;
}

// Mirrors Node's internal/util/colors.shouldColorize so node_compat tests
// that flip FORCE_COLOR / NODE_DISABLE_COLORS / NO_COLOR / TERM=dumb behave
// like Node. NO_COLOR / NODE_DISABLE_COLORS / TERM=dumb only apply via
// `stream.getColorDepth()` (matching Node's tty.WriteStream.getColorDepth)
// or under FORCE_COLOR; a non-TTY plain stream stays at `false`, while a
// fake stream with `isTTY=true` and no `getColorDepth` returns `true`.
function shouldColorize(stream) {
  const forceColor = colorEnv("FORCE_COLOR");
  if (forceColor !== undefined) {
    const v = String(forceColor);
    if (v === "0" || v === "false") return false;
    const nodeDisableColors = colorEnv("NODE_DISABLE_COLORS");
    const noColor = colorEnv("NO_COLOR");
    if (
      (nodeDisableColors !== undefined && nodeDisableColors !== "") ||
      (noColor !== undefined && noColor !== "") ||
      colorEnv("TERM") === "dumb"
    ) {
      // FORCE_COLOR with any non-default value still wins in Node 20+.
      return v !== "" && v !== "0";
    }
    return true;
  }
  if (!stream?.isTTY) return false;
  if (typeof stream.getColorDepth === "function") {
    return stream.getColorDepth() > 2;
  }
  // Match Node: TTY-flagged stream with no getColorDepth defaults to colorful.
  return true;
}

return { shouldColorize };
})();
