// Copyright 2018 the Deno authors. All rights reserved. MIT license.
// Public deno module.
/// <amd-module name="deno"/>
export {
  env,
  exit,
  FileInfo,
  makeTempDirSync,
  mkdirSync,
  readFileSync,
  renameSync,
  statSync,
  lstatSync,
  writeFileSync
} from "./os";
export {
  errorKinds,
  DenoError,
  ErrNotFound,
  ErrPermissionDenied,
  ErrConnectionRefused,
  ErrConnectionReset,
  ErrConnectionAborted,
  ErrNotConnected,
  ErrAddrInUse,
  ErrAddrNotAvailable,
  ErrBrokenPipe,
  ErrAlreadyExists,
  ErrWouldBlock,
  ErrInvalidInput,
  ErrInvalidData,
  ErrTimedOut,
  ErrInterrupted,
  ErrWriteZero,
  ErrOther,
  ErrUnexpectedEof,
  ErrEmptyHost,
  ErrIdnaError,
  ErrInvalidPort,
  ErrInvalidIpv4Address,
  ErrInvalidIpv6Address,
  ErrInvalidDomainCharacter,
  ErrRelativeUrlWithoutBase,
  ErrRelativeUrlWithCannotBeABaseBase,
  ErrSetHostOnCannotBeABaseUrl,
  ErrOverflow,
  ErrHttpUser,
  ErrHttpClosed,
  ErrHttpCanceled,
  ErrHttpParse,
  ErrHttpOther
} from "./errors";
export { libdeno } from "./libdeno";
export const argv: string[] = [];
