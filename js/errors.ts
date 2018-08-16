import { deno as fbs } from "gen/msg_generated";

export class DenoError<T extends fbs.ErrorKind> extends Error {
  constructor(readonly kind: T, msg: string) {
    super(msg);
    this.name = `deno.${fbs.ErrorKind[kind]}`;
  }
}

export function maybeThrowError(base: fbs.Base): void {
  const kind = base.errorKind();
  if (kind !== fbs.ErrorKind.NoError) {
    throw new DenoError(kind, base.error()!);
  }
}
