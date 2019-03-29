// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as msg from "gen/cli/msg_generated";
import * as flatbuffers from "./flatbuffers";
import { sendSync } from "./dispatch";
import { assert } from "./util";

export function formatError(errString: string): string {
  const builder = flatbuffers.createBuilder();
  const errString_ = builder.createString(errString);
  msg.FormatError.startFormatError(builder);
  msg.FormatError.addError(builder, errString_);
  const offset = msg.FormatError.endFormatError(builder);
  const baseRes = sendSync(builder, msg.Any.FormatError, offset);
  assert(baseRes != null);
  assert(msg.Any.FormatErrorRes === baseRes!.innerType());
  const formatErrorResMsg = new msg.FormatErrorRes();
  assert(baseRes!.inner(formatErrorResMsg) != null);
  const formattedError = formatErrorResMsg.error();
  assert(formatError != null);
  return formattedError!;
}
