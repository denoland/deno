// Copyright 2018-2025 the Deno authors. MIT license.
// deno-lint-ignore-file no-process-global
import { primordials } from "ext:core/mod.js";
import { getOptionValue } from "ext:deno_node/internal/options.ts";

const {
  ErrorPrototype,
  ErrorPrototypeToString,
  ObjectPrototypeIsPrototypeOf,
  SafeSet,
} = primordials;

let disableWarningSet;

export function onWarning(
  warning: Error & { code?: string; name?: string; detail?: string },
) {
  if (!disableWarningSet) {
    disableWarningSet = new SafeSet();
    const disableWarningValues = getOptionValue("--disable-warning");
    for (let i = 0; i < disableWarningValues?.length; i++) {
      disableWarningSet.add(disableWarningValues[i]);
    }
  }
  if (
    (warning?.code && disableWarningSet.has(warning.code)) ||
    (warning?.name && disableWarningSet.has(warning.name))
  ) return;

  if (!ObjectPrototypeIsPrototypeOf(ErrorPrototype, warning)) return;

  const isDeprecation = warning.name === "DeprecationWarning";
  if (isDeprecation && process.noDeprecation) return;
  const trace = process.traceProcessWarnings ||
    (isDeprecation && process.traceDeprecation);
  let msg = `(${process.release.name}:${process.pid}) `;
  if (warning.code) {
    msg += `[${warning.code}] `;
  }
  if (trace && warning.stack) {
    msg += `${warning.stack}`;
  } else {
    msg += typeof warning.toString === "function"
      // deno-lint-ignore prefer-primordials
      ? `${warning.toString()}`
      : ErrorPrototypeToString(warning);
  }
  if (typeof warning.detail === "string") {
    msg += `\n${warning.detail}`;
  }
  process.stderr.write(msg + "\n");
}
