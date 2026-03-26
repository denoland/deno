// Copyright 2018-2026 the Deno authors. MIT license.

// Deno lint plugin that flags usage of `Deno.*` APIs in ext/node/polyfills.
// The goal is to migrate Node.js polyfills away from depending on the Deno
// namespace, using internal ops or ext: imports instead.
//
// When you migrate a file, decrease its count below. When it reaches 0,
// remove the entry entirely. Adding new Deno.* usage is not allowed --
// the lint check in tools/lint.js will fail if the actual count exceeds
// the expected count.

// Expected violation counts per file. This is the baseline -- the numbers
// must only go down. Update this object when migrating Deno.* APIs away.
// Paths are relative to the repo root.
export const EXPECTED_VIOLATIONS: Record<string, number> = {
  "ext/node/polyfills/fs.ts": 54,
  "ext/node/polyfills/process.ts": 30,
  "ext/node/polyfills/os.ts": 22,
  "ext/node/polyfills/internal/child_process.ts": 21,
  "ext/node/polyfills/internal/fs/utils.mjs": 10,
  "ext/node/polyfills/_fs/_fs_copy.ts": 8,
  "ext/node/polyfills/internal_binding/stream_wrap.ts": 7,
  "ext/node/polyfills/internal/process/report.ts": 6,
  "ext/node/polyfills/internal_binding/pipe_wrap.ts": 6,
  "ext/node/polyfills/path/_win32.ts": 5,
  "ext/node/polyfills/internal/fs/streams.mjs": 5,
  "ext/node/polyfills/_process/process.ts": 5,
  "ext/node/polyfills/internal/readline/promises.mjs": 4,
  "ext/node/polyfills/internal/readline/callbacks.mjs": 4,
  "ext/node/polyfills/internal_binding/udp_wrap.ts": 4,
  "ext/node/polyfills/_process/streams.mjs": 4,
  "ext/node/polyfills/internal/readline/interface.mjs": 3,
  "ext/node/polyfills/internal/errors.ts": 3,
  "ext/node/polyfills/internal/buffer.mjs": 3,
  "ext/node/polyfills/internal_binding/tcp_wrap.ts": 3,
  "ext/node/polyfills/_fs/_fs_lstat.ts": 3,
  "ext/node/polyfills/testing.ts": 2,
  "ext/node/polyfills/internal/util/inspect.mjs": 2,
  "ext/node/polyfills/internal/tty.js": 2,
  "ext/node/polyfills/internal/timers.mjs": 2,
  "ext/node/polyfills/internal/console/constructor.mjs": 2,
  "ext/node/polyfills/internal_binding/cares_wrap.ts": 2,
  "ext/node/polyfills/https.ts": 2,
  "ext/node/polyfills/_tls_wrap.js": 2,
  "ext/node/polyfills/_fs/_fs_dir.ts": 2,
  "ext/node/polyfills/worker_threads.ts": 1,
  "ext/node/polyfills/net.ts": 1,
  "ext/node/polyfills/internal/util/debuglog.ts": 1,
  "ext/node/polyfills/internal/util/colors.ts": 1,
  "ext/node/polyfills/internal/options.ts": 1,
  "ext/node/polyfills/internal/assert/assertion_error.js": 1,
  "ext/node/polyfills/internal_binding/node_options.ts": 1,
  "ext/node/polyfills/child_process.ts": 2,
  "ext/node/polyfills/_fs/cp/cp.ts": 1,
  "ext/node/polyfills/_fs/_fs_lutimes.ts": 1,
  "ext/node/polyfills/_events.mjs": 1,
};

const plugin: Deno.lint.Plugin = {
  name: "node-polyfills",
  rules: {
    "no-deno-api": {
      create(context) {
        return {
          MemberExpression(node) {
            if (
              node.object.type === "Identifier" &&
              node.object.name === "Deno"
            ) {
              const property = node.property.type === "Identifier"
                ? node.property.name
                : null;

              context.report({
                node: node.object,
                message: property
                  ? `Usage of \`Deno.${property}\` in Node.js polyfill. ` +
                    "Use internal ops or ext: imports instead."
                  : "Usage of `Deno` namespace in Node.js polyfill. " +
                    "Use internal ops or ext: imports instead.",
              });
            }
          },
        };
      },
    },
  },
};

export default plugin;
