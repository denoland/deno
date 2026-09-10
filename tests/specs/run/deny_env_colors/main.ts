// Under a global `--deny-env`, Node's internal color/terminal detection must
// not throw NotCapable: it reads FORCE_COLOR / NO_COLOR / NODE_DISABLE_COLORS /
// TERM (and other terminal/CI markers) as internal runtime config, without a
// permission check. This guards against a regression where those reads went
// through the permission path and crashed `util.inspect(x, { colors: true })`
// and `tty.getColorDepth()`.
import util from "node:util";
import tty from "node:tty";

// Forced colors exercises the FORCE_COLOR / NO_COLOR / NODE_DISABLE_COLORS /
// TERM branch of shouldColorize. Results are concatenated into single strings
// so console.log prints them verbatim (FORCE_COLOR would otherwise colorize
// inspected boolean args).
const inspected = util.inspect({ a: 1, b: "two" }, { colors: true });
console.log("inspect ok: " + (typeof inspected === "string"));

// Exercises getColorDepth, which reads a broader set of terminal/CI env vars.
const depth = tty.WriteStream.prototype.getColorDepth();
console.log("colorDepth ok: " + (typeof depth === "number"));

// Sanity check: a user read of a non-allowlisted var is still denied.
try {
  Deno.env.get("SOME_SECRET");
  console.log("secret: UNEXPECTEDLY ALLOWED");
} catch (err) {
  console.log("secret: " + (err as Error).name);
}
