import { deno as fbs } from "gen/msg_generated";

// @internal
export class DenoError<T extends fbs.ErrorKind> extends Error {
  constructor(readonly kind: T, msg: string) {
    super(msg);
    this.name = `deno.${fbs.ErrorKind[kind]}`;
  }
}

// @internal
export function maybeThrowError(base: fbs.Base): void {
  const err = maybeError(base);
  if (err != null) {
    throw err;
  }
}

export function maybeError(base: fbs.Base): null | DenoError<fbs.ErrorKind> {
  const kind = base.errorKind();
  if (kind === fbs.ErrorKind.NoError) {
    return null;
  } else {
    return new DenoError(kind, base.error()!);
  }
}
