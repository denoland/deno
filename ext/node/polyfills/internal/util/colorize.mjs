// Copyright 2018-2026 the Deno authors. MIT license.

// Mirrors Node's internal/util/colors.shouldColorize.
// Kept in a standalone module so both internal/util.mjs and
// internal/util/inspect.mjs can import it without introducing a circular
// dependency (errors.ts imports inspect.mjs, so inspect.mjs cannot import
// anything that transitively depends on errors.ts).

(function () {
const { primordials } = __bootstrap;
const { String } = primordials;

// Mirrors Node's internal/util/colors.shouldColorize so node_compat tests
// that flip FORCE_COLOR / NODE_DISABLE_COLORS / NO_COLOR / TERM=dumb behave
// like Node. NO_COLOR / NODE_DISABLE_COLORS / TERM=dumb only apply via
// `stream.getColorDepth()` (matching Node's tty.WriteStream.getColorDepth)
// or under FORCE_COLOR; a non-TTY plain stream stays at `false`, while a
// fake stream with `isTTY=true` and no `getColorDepth` returns `true`.
function shouldColorize(stream) {
  const env = globalThis.process.env || {};
  if (env.FORCE_COLOR !== undefined) {
    const v = String(env.FORCE_COLOR);
    if (v === "0" || v === "false") return false;
    if (
      (env.NODE_DISABLE_COLORS !== undefined &&
        env.NODE_DISABLE_COLORS !== "") ||
      (env.NO_COLOR !== undefined && env.NO_COLOR !== "") ||
      env.TERM === "dumb"
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
