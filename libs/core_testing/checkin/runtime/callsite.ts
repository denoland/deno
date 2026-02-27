// Copyright 2018-2025 the Deno authors. MIT license.
import { op_current_user_call_site } from "ext:core/ops";

const callSiteRetBuf = new Uint32Array(2);
const callSiteRetBufU8 = new Uint8Array(callSiteRetBuf.buffer);

export function getCallSite() {
  const fileName = op_current_user_call_site(callSiteRetBufU8);
  const lineNumber = callSiteRetBuf[0];
  const columnNumber = callSiteRetBuf[1];
  return { fileName, lineNumber, columnNumber };
}
