// Copyright 2018-2026 the Deno authors. MIT license.
(function () {
const { core, primordials } = __bootstrap;
const { op_get_env_no_permission_check } = core.ops;
const {
  SafeMap,
  ArrayPrototypeForEach,
  ArrayPrototypePush,
  ArrayPrototypeConcat,
  ArrayPrototypeSlice,
  StringPrototypeSlice,
  StringPrototypeStartsWith,
} = primordials;

// This module ports:
// - https://github.com/nodejs/node/blob/master/src/node_options-inl.h
// - https://github.com/nodejs/node/blob/master/src/node_options.cc
// - https://github.com/nodejs/node/blob/master/src/node_options.h

// Quote-aware tokenizer for NODE_OPTIONS. Node.js uses a shell-like parser
// that respects single and double quotes, so `--title="hello world"` is a
// single token whose value is `hello world`, not two tokens.
function splitNodeOptions(input: string): string[] {
  const args: string[] = [];
  let current = "";
  let inDouble = false;
  let inSingle = false;

  for (let i = 0; i < input.length; i++) {
    const ch = input[i];
    if (ch === '"' && !inSingle) {
      inDouble = !inDouble;
    } else if (ch === "'" && !inDouble) {
      inSingle = !inSingle;
    } else if (
      (ch === " " || ch === "\t" || ch === "\n" || ch === "\r") && !inDouble &&
      !inSingle
    ) {
      if (current.length > 0) {
        ArrayPrototypePush(args, current);
        current = "";
      }
    } else {
      current += ch;
    }
  }
  if (current.length > 0) {
    ArrayPrototypePush(args, current);
  }
  return args;
}

/** Gets the all options for Node.js
 * This function is expensive to execute. `getOptionValue` in `internal/options.ts`
 * should be used instead to get a specific option. */
type OptionValue = { value: string | boolean };

let optionsMap: Map<string, OptionValue> | undefined;
let execArgvOptionsMap: Map<string, OptionValue> | undefined;
let execArgvSnapshot: string[] | undefined;

function setOptionSourceExecArgv(execArgv: string[]) {
  execArgvSnapshot = ArrayPrototypeSlice(execArgv);
  optionsMap = undefined;
  execArgvOptionsMap = undefined;
}

function createDefaultOptions() {
  return new SafeMap([
    ["--warnings", { value: true }],
    ["--pending-deprecation", { value: false }],
    ["--expose-internals", { value: false }],
    ["--title", { value: "" }],
  ]);
}

function parseOption(options: Map<string, OptionValue>, arg: string) {
  if (StringPrototypeStartsWith(arg, "--title=")) {
    options.set("--title", { value: StringPrototypeSlice(arg, 8) });
    return;
  }
  if (StringPrototypeStartsWith(arg, "--tls-cipher-list=")) {
    options.set("--tls-cipher-list", {
      value: StringPrototypeSlice(arg, "--tls-cipher-list=".length),
    });
    return;
  }
  switch (arg) {
    case "--no-warnings":
      options.set("--warnings", { value: false });
      break;
    case "--pending-deprecation":
      options.set("--pending-deprecation", { value: true });
      break;
    case "--expose-internals":
    case "--expose_internals":
      options.set("--expose-internals", { value: true });
      break;
    case "--tls-min-v1.0":
    case "--tls-min-v1.1":
    case "--tls-min-v1.2":
    case "--tls-min-v1.3":
    case "--tls-max-v1.2":
    case "--tls-max-v1.3":
    case "--use-bundled-ca":
    case "--use-openssl-ca":
    case "--use-system-ca":
      options.set(arg, { value: true });
      break;
    case "--no-tls-min-v1.0":
      options.set("--tls-min-v1.0", { value: false });
      break;
    case "--no-tls-min-v1.1":
      options.set("--tls-min-v1.1", { value: false });
      break;
    case "--no-tls-min-v1.2":
      options.set("--tls-min-v1.2", { value: false });
      break;
    case "--no-tls-min-v1.3":
      options.set("--tls-min-v1.3", { value: false });
      break;
    case "--no-tls-max-v1.2":
      options.set("--tls-max-v1.2", { value: false });
      break;
    case "--no-tls-max-v1.3":
      options.set("--tls-max-v1.3", { value: false });
      break;
    case "--no-use-bundled-ca":
      options.set("--use-bundled-ca", { value: false });
      break;
    case "--no-use-openssl-ca":
      options.set("--use-openssl-ca", { value: false });
      break;
    case "--no-use-system-ca":
      options.set("--use-system-ca", { value: false });
      break;
    default:
      if (StringPrototypeStartsWith(arg, "--dns-result-order=")) {
        const value = StringPrototypeSlice(
          arg,
          "--dns-result-order=".length,
        );
        options.set("--dns-result-order", { value });
      }
      break;
  }
}

function getExecArgv() {
  if (execArgvSnapshot) {
    return execArgvSnapshot;
  }
  // `globalThis.process` is a lazy getter that loads `node:process`. During
  // node:process's OWN cold bootstrap (when the require system is deferred
  // out of the snapshot), reading it re-enters that load and the `default`
  // export is still in the temporal dead zone -- accessing the getter throws
  // rather than yielding undefined, so `?.` doesn't help. Guard it: no exec
  // args are available pre-bootstrap anyway, so fall back to `[]`.
  try {
    return globalThis.process?.execArgv ?? [];
  } catch {
    return [];
  }
}

function getOptions() {
  if (optionsMap) {
    return { options: optionsMap };
  }

  const options = createDefaultOptions();
  // Internal runtime bootstrap read of the process's own NODE_OPTIONS: this
  // happens during Node process setup, is not user code, and must work in
  // every context (including compiled binaries, whose embedded permissions do
  // not include the default env allowlist). Read it without a permission
  // check, matching the other internal Node config reads (NODE_EXTRA_CA_CERTS,
  // NODE_TLS_REJECT_UNAUTHORIZED, TERM). User-facing `Deno.env.get`/process.env
  // reads of NODE_OPTIONS still go through the permission path.
  const nodeOptions = op_get_env_no_permission_check("NODE_OPTIONS");
  const envArgs = nodeOptions ? splitNodeOptions(nodeOptions) : [];
  const execArgv = getExecArgv();
  const args = ArrayPrototypeConcat(envArgs, execArgv);
  ArrayPrototypeForEach(args, (arg) => parseOption(options, arg));
  optionsMap = options;
  return { options };
}

function getExecArgvOptions() {
  if (execArgvOptionsMap) {
    return { options: execArgvOptionsMap };
  }
  const options = new SafeMap();
  ArrayPrototypeForEach(getExecArgv(), (arg) => parseOption(options, arg));
  execArgvOptionsMap = options;
  return { options };
}

return {
  getExecArgvOptions,
  getOptions,
  setOptionSourceExecArgv,
};
})();
