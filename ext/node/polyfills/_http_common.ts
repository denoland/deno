// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

import { primordials } from "ext:core/mod.js";
const { RegExpPrototypeTest, SafeRegExp } = primordials;
const tokenRegExp = new SafeRegExp(/^[\^_`a-zA-Z\-0-9!#$%&'*+.|~]+$/);
/**
 * Verifies that the given val is a valid HTTP token
 * per the rules defined in RFC 7230
 * See https://tools.ietf.org/html/rfc7230#section-3.2.6
 */
function checkIsHttpToken(val: string) {
  return RegExpPrototypeTest(tokenRegExp, val);
}

const headerCharRegex = new SafeRegExp(/[^\t\x20-\x7e\x80-\xff]/);
/**
 * True if val contains an invalid field-vchar
 *  field-value    = *( field-content / obs-fold )
 *  field-content  = field-vchar [ 1*( SP / HTAB ) field-vchar ]
 *  field-vchar    = VCHAR / obs-text
 */
function checkInvalidHeaderChar(val: string) {
  return RegExpPrototypeTest(headerCharRegex, val);
}

export const chunkExpression = new SafeRegExp(/(?:^|\W)chunked(?:$|\W)/i);
export {
  checkInvalidHeaderChar as _checkInvalidHeaderChar,
  checkIsHttpToken as _checkIsHttpToken,
};
