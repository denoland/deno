// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import * as fbs from "gen/msg_generated";
import { flatbuffers } from "flatbuffers";
import { assert } from "./util";
import * as dispatch from "./dispatch";

/**
 * Decodes a string of data which has been encoded using base-64 encoding.
 */
export function atob(s: string): string {
  // TODO: make exception compliant to standard, InvalidCharacterError
  // https://html.spec.whatwg.org/multipage/webappapis.html#dom-atob
  // TODO: encoding! (JS is UTF16/UCS2)
  const builder = new flatbuffers.Builder();
  const data = builder.createString(s);
  fbs.AToB.startAToB(builder);
  fbs.AToB.addData(builder, data);
  const sentMsg = fbs.AToB.endAToB(builder);
  const baseRes = dispatch.sendSync(builder, fbs.Any.AToB, sentMsg);
  assert(baseRes != null);
  assert(fbs.Any.AToBRes === baseRes!.msgType());
  const receivedMsg = new fbs.AToBRes();
  assert(baseRes!.msg(receivedMsg) != null);
  return receivedMsg.decoded()!;
}

/**
 * Creates a base-64 encoded ASCII string from a string
 */
export function btoa(s: string): string {
  // TODO: make exception compliant to standard, InvalidCharacterError
  // https://html.spec.whatwg.org/multipage/webappapis.html#dom-btoa
  // TODO: encoding! (JS is UTF16/UCS2)
  const builder = new flatbuffers.Builder();
  const data = builder.createString(s);
  fbs.BToA.startBToA(builder);
  fbs.BToA.addData(builder, data);
  const sentMsg = fbs.BToA.endBToA(builder);
  const baseRes = dispatch.sendSync(builder, fbs.Any.BToA, sentMsg);
  assert(baseRes != null);
  assert(fbs.Any.BToARes === baseRes!.msgType());
  const receivedMsg = new fbs.BToARes();
  assert(baseRes!.msg(receivedMsg) != null);
  return receivedMsg.encoded()!;
}

// @types/text-encoding relies on lib.dom.d.ts for some interfaces. We do not
// want to include lib.dom.d.ts (due to size) into deno's global type scope.
// Therefore this hack: add a few of the missing interfaces in
// @types/text-encoding to the global scope before importing.

declare global {
  type BufferSource = ArrayBufferView | ArrayBuffer;

  interface TextDecodeOptions {
    stream?: boolean;
  }

  interface TextDecoderOptions {
    fatal?: boolean;
    ignoreBOM?: boolean;
  }

  interface TextDecoder {
    readonly encoding: string;
    readonly fatal: boolean;
    readonly ignoreBOM: boolean;
    decode(input?: BufferSource, options?: TextDecodeOptions): string;
  }
}

export { TextEncoder, TextDecoder } from "text-encoding";
