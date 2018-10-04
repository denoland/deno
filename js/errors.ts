import * as msg from "gen/msg_generated";
export { ErrorKind } from "gen/msg_generated";

// @internal
export class DenoError<T extends msg.ErrorKind> extends Error {
  constructor(readonly kind: T, errStr: string) {
    super(errStr);
    this.name = msg.ErrorKind[kind];
  }
}

// @internal
export function maybeThrowError(base: msg.Base): void {
  const err = maybeError(base);
  if (err != null) {
    throw err;
  }
}

export function maybeError(base: msg.Base): null | DenoError<msg.ErrorKind> {
  const kind = base.errorKind();
  if (kind === msg.ErrorKind.NoError) {
    return null;
  } else {
    return new DenoError(kind, base.error()!);
  }
}
