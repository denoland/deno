// Copyright 2018-2025 the Deno authors. MIT license.

// @ts-check
/// <reference path="../../core/internal.d.ts" />
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../../cli/tsc/dts/lib.deno_web.d.ts" />

import { primordials } from "ext:core/mod.js";
const {
  ErrorPrototype,
  ObjectDefineProperty,
  ObjectEntries,
  ObjectHasOwn,
  ObjectPrototypeIsPrototypeOf,
  ObjectSetPrototypeOf,
  Symbol,
  SymbolFor,
} = primordials;

import * as webidl from "ext:deno_webidl/00_webidl.js";
import { createFilteredInspectProxy } from "./01_console.js";
import { DOMException } from "ext:core/ops";

const _name = Symbol("name");
const _message = Symbol("message");
const _code = Symbol("code");

// Defined in WebIDL 4.3.
// https://webidl.spec.whatwg.org/#idl-DOMException
const INDEX_SIZE_ERR = 1;
const DOMSTRING_SIZE_ERR = 2;
const HIERARCHY_REQUEST_ERR = 3;
const WRONG_DOCUMENT_ERR = 4;
const INVALID_CHARACTER_ERR = 5;
const NO_DATA_ALLOWED_ERR = 6;
const NO_MODIFICATION_ALLOWED_ERR = 7;
const NOT_FOUND_ERR = 8;
const NOT_SUPPORTED_ERR = 9;
const INUSE_ATTRIBUTE_ERR = 10;
const INVALID_STATE_ERR = 11;
const SYNTAX_ERR = 12;
const INVALID_MODIFICATION_ERR = 13;
const NAMESPACE_ERR = 14;
const INVALID_ACCESS_ERR = 15;
const VALIDATION_ERR = 16;
const TYPE_MISMATCH_ERR = 17;
const SECURITY_ERR = 18;
const NETWORK_ERR = 19;
const ABORT_ERR = 20;
const URL_MISMATCH_ERR = 21;
const QUOTA_EXCEEDED_ERR = 22;
const TIMEOUT_ERR = 23;
const INVALID_NODE_TYPE_ERR = 24;
const DATA_CLONE_ERR = 25;

ObjectSetPrototypeOf(DOMException.prototype, ErrorPrototype);

webidl.configureInterface(DOMException);
const DOMExceptionPrototype = DOMException.prototype;

ObjectDefineProperty(
  DOMExceptionPrototype,
  SymbolFor("Deno.privateCustomInspect"),
  {
    __proto__: null,
    value(inspect, inspectOptions) {
      if (ObjectHasOwn(this, "stack")) {
        const stack = this.stack;
        if (typeof stack === "string") {
          return stack;
        }
      }
      return inspect(
        createFilteredInspectProxy({
          object: this,
          evaluate: ObjectPrototypeIsPrototypeOf(DOMExceptionPrototype, this),
          keys: [
            "message",
            "name",
            "code",
          ],
        }),
        inspectOptions,
      );
    },
  },
);

const entries = ObjectEntries({
  INDEX_SIZE_ERR,
  DOMSTRING_SIZE_ERR,
  HIERARCHY_REQUEST_ERR,
  WRONG_DOCUMENT_ERR,
  INVALID_CHARACTER_ERR,
  NO_DATA_ALLOWED_ERR,
  NO_MODIFICATION_ALLOWED_ERR,
  NOT_FOUND_ERR,
  NOT_SUPPORTED_ERR,
  INUSE_ATTRIBUTE_ERR,
  INVALID_STATE_ERR,
  SYNTAX_ERR,
  INVALID_MODIFICATION_ERR,
  NAMESPACE_ERR,
  INVALID_ACCESS_ERR,
  VALIDATION_ERR,
  TYPE_MISMATCH_ERR,
  SECURITY_ERR,
  NETWORK_ERR,
  ABORT_ERR,
  URL_MISMATCH_ERR,
  QUOTA_EXCEEDED_ERR,
  TIMEOUT_ERR,
  INVALID_NODE_TYPE_ERR,
  DATA_CLONE_ERR,
});
for (let i = 0; i < entries.length; ++i) {
  const { 0: key, 1: value } = entries[i];
  const desc = { __proto__: null, value, enumerable: true };
  ObjectDefineProperty(DOMException, key, desc);
  ObjectDefineProperty(DOMException.prototype, key, desc);
}

export { DOMException, DOMExceptionPrototype };
