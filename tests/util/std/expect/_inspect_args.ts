// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// deno-lint-ignore-file

export function inspectArgs(args: unknown[]): string {
  return args.map(inspectArg).join(", ");
}

export function inspectArg(arg: unknown): string {
  const { Deno } = globalThis as any;
  return typeof Deno !== "undefined" && Deno.inspect
    ? Deno.inspect(arg)
    : String(arg);
}
