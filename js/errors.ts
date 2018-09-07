import { deno as fbs } from "gen/msg_generated";
import { assert } from "./util";

const ERR_PREFIX = "Err";

export class DenoError<T extends fbs.ErrorKind> extends Error {
  constructor(readonly kind: T, msg: string) {
    super(msg);
    this.name = `deno.${ERR_PREFIX}${fbs.ErrorKind[kind]}`;
  }
}

const errorClasses = new Map();

function ErrorFactory<T extends fbs.ErrorKind>(
  kind: T
): new (msg: string) => DenoError<T> {
  const name = `${ERR_PREFIX}${fbs.ErrorKind[kind]}`;
  const anonymousClass = class extends DenoError<T> {
    constructor(msg: string) {
      super(kind, msg);
    }
  };
  Object.defineProperty(anonymousClass, "name", {
    value: name,
  });
  errorClasses.set(kind, anonymousClass);
  return anonymousClass;
}

// @internal
export function maybeThrowError(base: fbs.Base): void {
  const kind = base.errorKind();
  if (kind !== fbs.ErrorKind.NoError) {
    const errorClass = errorClasses.get(kind);
    throw new errorClass(base.error()!);
  }
}

// Each of the error codes in src/msg.fbs is manually mapped to a
// JavaScript error class. The testErrorClasses function below 
// checks that there is a class for every error code.
// TODO It would be good if we didn't have to manually maintain this list.
// tslint:disable:max-line-length
// tslint:disable:variable-name
export const ErrNotFound = ErrorFactory(fbs.ErrorKind.NotFound);
export const ErrPermissionDenied = ErrorFactory(fbs.ErrorKind.PermissionDenied);
export const ErrConnectionRefused = ErrorFactory(fbs.ErrorKind.ConnectionRefused);
export const ErrConnectionReset = ErrorFactory(fbs.ErrorKind.ConnectionReset);
export const ErrConnectionAborted = ErrorFactory(fbs.ErrorKind.ConnectionAborted);
export const ErrNotConnected = ErrorFactory(fbs.ErrorKind.NotConnected);
export const ErrAddrInUse = ErrorFactory(fbs.ErrorKind.AddrInUse);
export const ErrAddrNotAvailable = ErrorFactory(fbs.ErrorKind.AddrNotAvailable);
export const ErrBrokenPipe = ErrorFactory(fbs.ErrorKind.BrokenPipe);
export const ErrAlreadyExists = ErrorFactory(fbs.ErrorKind.AlreadyExists);
export const ErrWouldBlock = ErrorFactory(fbs.ErrorKind.WouldBlock);
export const ErrInvalidInput = ErrorFactory(fbs.ErrorKind.InvalidInput);
export const ErrInvalidData = ErrorFactory(fbs.ErrorKind.InvalidData);
export const ErrTimedOut = ErrorFactory(fbs.ErrorKind.TimedOut);
export const ErrInterrupted = ErrorFactory(fbs.ErrorKind.Interrupted);
export const ErrWriteZero = ErrorFactory(fbs.ErrorKind.WriteZero);
export const ErrOther = ErrorFactory(fbs.ErrorKind.Other);
export const ErrUnexpectedEof = ErrorFactory(fbs.ErrorKind.UnexpectedEof);
export const ErrEmptyHost = ErrorFactory(fbs.ErrorKind.EmptyHost);
export const ErrIdnaError = ErrorFactory(fbs.ErrorKind.IdnaError);
export const ErrInvalidPort = ErrorFactory(fbs.ErrorKind.InvalidPort);
export const ErrInvalidIpv4Address = ErrorFactory(fbs.ErrorKind.InvalidIpv4Address);
export const ErrInvalidIpv6Address = ErrorFactory(fbs.ErrorKind.InvalidIpv6Address);
export const ErrInvalidDomainCharacter = ErrorFactory(fbs.ErrorKind.InvalidDomainCharacter);
export const ErrRelativeUrlWithoutBase = ErrorFactory(fbs.ErrorKind.RelativeUrlWithoutBase);
export const ErrRelativeUrlWithCannotBeABaseBase = ErrorFactory(fbs.ErrorKind.RelativeUrlWithCannotBeABaseBase);
export const ErrSetHostOnCannotBeABaseUrl = ErrorFactory(fbs.ErrorKind.SetHostOnCannotBeABaseUrl);
export const ErrOverflow = ErrorFactory(fbs.ErrorKind.Overflow);
export const ErrHttpUser = ErrorFactory(fbs.ErrorKind.HttpUser);
export const ErrHttpClosed = ErrorFactory(fbs.ErrorKind.HttpClosed);
export const ErrHttpCanceled = ErrorFactory(fbs.ErrorKind.HttpCanceled);
export const ErrHttpParse = ErrorFactory(fbs.ErrorKind.HttpParse);
export const ErrHttpOther = ErrorFactory(fbs.ErrorKind.HttpOther);
// tslint:enable:variable-name
// tslint:enable:max-line-length

// The following code does not have any impact on Deno's startup
// performance as we're using V8 snapshots, this code will casue
// `snapshot_creator` to fail during build time whenever we
// forgot to register an error class.
function testErrorClasses(): void {
  const len = Object.keys(fbs.ErrorKind).length / 2;
  for (let kind = 0; kind < len; ++kind) {
    if (kind === fbs.ErrorKind.NoError) {
      continue;
    }
    assert(errorClasses.has(kind), `No error class for ${fbs.ErrorKind[kind]}`);
  }
}

testErrorClasses();
