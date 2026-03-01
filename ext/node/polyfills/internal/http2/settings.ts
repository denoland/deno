// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Node.js contributors. All rights reserved. MIT license.
//
// HTTP/2 settings encoding/decoding per RFC 7540.
// Each setting is a 16-bit identifier (big-endian) followed by a 32-bit value (big-endian).

// deno-lint-ignore-file prefer-primordials

import { Buffer } from "node:buffer";
import { primordials } from "ext:core/mod.js";
import {
  ERR_HTTP2_INVALID_PACKED_SETTINGS_LENGTH,
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_ARG_VALUE,
} from "ext:deno_node/internal/errors.ts";
import { isArrayBufferView } from "ext:deno_node/internal/util/types.ts";
import { hideStackFrames } from "ext:deno_node/internal/hide_stack_frames.ts";

const {
  NumberIsInteger,
  ObjectKeys,
  ObjectPrototypeHasOwnProperty,
  StringPrototypeToLowerCase,
} = primordials;

// HTTP/2 setting identifiers (RFC 7540 and extensions)
const SETTINGS_HEADER_TABLE_SIZE = 0x1;
const SETTINGS_ENABLE_PUSH = 0x2;
const SETTINGS_MAX_CONCURRENT_STREAMS = 0x3;
const SETTINGS_INITIAL_WINDOW_SIZE = 0x4;
const SETTINGS_MAX_FRAME_SIZE = 0x5;
const SETTINGS_MAX_HEADER_LIST_SIZE = 0x6;
const SETTINGS_ENABLE_CONNECT_PROTOCOL = 0x8;

// Known setting IDs to camelCase name
const SETTING_ID_TO_NAME: Record<number, string> = {
  [SETTINGS_HEADER_TABLE_SIZE]: "headerTableSize",
  [SETTINGS_ENABLE_PUSH]: "enablePush",
  [SETTINGS_MAX_CONCURRENT_STREAMS]: "maxConcurrentStreams",
  [SETTINGS_INITIAL_WINDOW_SIZE]: "initialWindowSize",
  [SETTINGS_MAX_FRAME_SIZE]: "maxFrameSize",
  [SETTINGS_MAX_HEADER_LIST_SIZE]: "maxHeaderListSize",
  [SETTINGS_ENABLE_CONNECT_PROTOCOL]: "enableConnectProtocol",
};

// Name to ID for packing
const SETTING_NAME_TO_ID: Record<string, number> = {
  headerTableSize: SETTINGS_HEADER_TABLE_SIZE,
  enablePush: SETTINGS_ENABLE_PUSH,
  maxConcurrentStreams: SETTINGS_MAX_CONCURRENT_STREAMS,
  initialWindowSize: SETTINGS_INITIAL_WINDOW_SIZE,
  maxFrameSize: SETTINGS_MAX_FRAME_SIZE,
  maxHeaderListSize: SETTINGS_MAX_HEADER_LIST_SIZE,
  enableConnectProtocol: SETTINGS_ENABLE_CONNECT_PROTOCOL,
};

// Lowercase name to ID for case-insensitive lookup (e.g. HEADERTABLESIZE -> headerTableSize ID)
const LOWER_NAME_TO_ID: Record<string, number> = {
  headertablesize: SETTINGS_HEADER_TABLE_SIZE,
  enablepush: SETTINGS_ENABLE_PUSH,
  maxconcurrentstreams: SETTINGS_MAX_CONCURRENT_STREAMS,
  initialwindowsize: SETTINGS_INITIAL_WINDOW_SIZE,
  maxframesize: SETTINGS_MAX_FRAME_SIZE,
  maxheaderlistsize: SETTINGS_MAX_HEADER_LIST_SIZE,
  enableconnectprotocol: SETTINGS_ENABLE_CONNECT_PROTOCOL,
};

// Default values (Node.js defaults)
const DEFAULT_HEADER_TABLE_SIZE = 4096;
const DEFAULT_ENABLE_PUSH = 1;
const DEFAULT_MAX_CONCURRENT_STREAMS = 4294967295;
const DEFAULT_INITIAL_WINDOW_SIZE = 65535;
const DEFAULT_MAX_FRAME_SIZE = 16384;
const DEFAULT_MAX_HEADER_LIST_SIZE = 65535;
const DEFAULT_ENABLE_CONNECT_PROTOCOL = 0;

const MIN_MAX_FRAME_SIZE = 16384;
const MAX_MAX_FRAME_SIZE = 16777215;
const MAX_INITIAL_WINDOW_SIZE = 2147483647;

export interface Http2SettingsObject {
  headerTableSize?: number;
  enablePush?: number;
  maxConcurrentStreams?: number;
  initialWindowSize?: number;
  maxFrameSize?: number;
  maxHeaderListSize?: number;
  enableConnectProtocol?: number;
  [key: string]: number | undefined;
}

/**
 * Returns the default HTTP/2 settings object used by Node.js.
 * Matches the defaults from RFC 7540 and Node.js behavior.
 */
export function getDefaultSettings(): Http2SettingsObject {
  return {
    headerTableSize: DEFAULT_HEADER_TABLE_SIZE,
    enablePush: DEFAULT_ENABLE_PUSH,
    maxConcurrentStreams: DEFAULT_MAX_CONCURRENT_STREAMS,
    initialWindowSize: DEFAULT_INITIAL_WINDOW_SIZE,
    maxFrameSize: DEFAULT_MAX_FRAME_SIZE,
    maxHeaderListSize: DEFAULT_MAX_HEADER_LIST_SIZE,
    enableConnectProtocol: DEFAULT_ENABLE_CONNECT_PROTOCOL,
  };
}

function writeUint16BE(
  buffer: Uint8Array,
  offset: number,
  value: number,
): void {
  buffer[offset] = (value >>> 8) & 0xff;
  buffer[offset + 1] = value & 0xff;
}

function writeUint32BE(
  buffer: Uint8Array,
  offset: number,
  value: number,
): void {
  buffer[offset] = (value >>> 24) & 0xff;
  buffer[offset + 1] = (value >>> 16) & 0xff;
  buffer[offset + 2] = (value >>> 8) & 0xff;
  buffer[offset + 3] = value & 0xff;
}

function readUint16BE(buffer: Uint8Array, offset: number): number {
  return (buffer[offset] << 8) | buffer[offset + 1];
}

function readUint32BE(buffer: Uint8Array, offset: number): number {
  return (
    (buffer[offset] * 0x1000000) +
    ((buffer[offset + 1] << 16) | (buffer[offset + 2] << 8) |
      buffer[offset + 3])
  );
}

function validateSettingValue(
  name: string,
  id: number,
  value: number,
): void {
  if (typeof value !== "number" || !NumberIsInteger(value) || value < 0) {
    throw new ERR_INVALID_ARG_VALUE(
      name,
      value,
      "must be a non-negative 32-bit integer",
    );
  }
  if (value > 0xffff_ffff) {
    throw new ERR_INVALID_ARG_VALUE(
      name,
      value,
      "must not exceed 2^32 - 1",
    );
  }
  switch (id) {
    case SETTINGS_ENABLE_PUSH:
      if (value !== 0 && value !== 1) {
        throw new ERR_INVALID_ARG_VALUE(
          name,
          value,
          "enablePush must be 0 or 1",
        );
      }
      break;
    case SETTINGS_MAX_FRAME_SIZE:
      if (value < MIN_MAX_FRAME_SIZE || value > MAX_MAX_FRAME_SIZE) {
        throw new ERR_INVALID_ARG_VALUE(
          name,
          value,
          `maxFrameSize must be between ${MIN_MAX_FRAME_SIZE} and ${MAX_MAX_FRAME_SIZE}`,
        );
      }
      break;
    case SETTINGS_INITIAL_WINDOW_SIZE:
      if (value > MAX_INITIAL_WINDOW_SIZE) {
        throw new ERR_INVALID_ARG_VALUE(
          name,
          value,
          `initialWindowSize must not exceed ${MAX_INITIAL_WINDOW_SIZE}`,
        );
      }
      break;
    case SETTINGS_ENABLE_CONNECT_PROTOCOL:
      if (value !== 0 && value !== 1) {
        throw new ERR_INVALID_ARG_VALUE(
          name,
          value,
          "enableConnectProtocol must be 0 or 1",
        );
      }
      break;
    default:
      break;
  }
}

/**
 * Packs an HTTP/2 settings object into a Buffer as per RFC 7540.
 * Used for the HTTP2-Settings header.
 */
export const packSettings = hideStackFrames(
  function packSettings(settings: Http2SettingsObject): Buffer {
    if (
      settings == null ||
      typeof settings !== "object" ||
      Array.isArray(settings)
    ) {
      throw new ERR_INVALID_ARG_TYPE(
        "settings",
        "Object",
        settings,
      );
    }
    const keys = ObjectKeys(settings);
    const entries: Array<[number, number]> = [];
    for (let i = 0; i < keys.length; i++) {
      const key = keys[i];
      const lowerKey = StringPrototypeToLowerCase(key);
      let id: number;
      if (ObjectPrototypeHasOwnProperty(SETTING_NAME_TO_ID, key)) {
        id = SETTING_NAME_TO_ID[key];
      } else if (ObjectPrototypeHasOwnProperty(LOWER_NAME_TO_ID, lowerKey)) {
        id = LOWER_NAME_TO_ID[lowerKey];
      } else if (
        NumberIsInteger(Number(key)) && Number(key) >= 0 &&
        Number(key) <= 0xffff
      ) {
        id = Number(key);
      } else {
        throw new ERR_INVALID_ARG_VALUE(
          "settings",
          key,
          "is not a valid HTTP/2 setting name or identifier",
        );
      }
      const value = settings[key];
      if (value === undefined) continue;
      validateSettingValue(key, id, value);
      entries.push([id, value >>> 0]);
    }
    const buf = Buffer.allocUnsafe(entries.length * 6);
    const view = new Uint8Array(buf.buffer, buf.byteOffset, buf.byteLength);
    for (let i = 0; i < entries.length; i++) {
      const [id, value] = entries[i];
      writeUint16BE(view, i * 6, id);
      writeUint32BE(view, i * 6 + 2, value);
    }
    return buf;
  },
);

/**
 * Unpacks a Buffer or TypedArray containing packed HTTP/2 settings into an object.
 */
export const unpackSettings = hideStackFrames(
  function unpackSettings(
    buffer: Buffer | ArrayBufferView,
  ): Http2SettingsObject {
    if (buffer == null) {
      throw new ERR_INVALID_ARG_TYPE(
        "buffer",
        ["Buffer", "TypedArray"],
        buffer,
      );
    }
    let view: Uint8Array;
    if (Buffer.isBuffer(buffer)) {
      view = new Uint8Array(
        buffer.buffer,
        buffer.byteOffset,
        buffer.byteLength,
      );
    } else if (isArrayBufferView(buffer)) {
      view = new Uint8Array(
        buffer.buffer,
        buffer.byteOffset,
        buffer.byteLength,
      );
    } else {
      throw new ERR_INVALID_ARG_TYPE(
        "buffer",
        ["Buffer", "TypedArray"],
        buffer,
      );
    }
    const len = view.length;
    if (len % 6 !== 0) {
      throw new ERR_HTTP2_INVALID_PACKED_SETTINGS_LENGTH();
    }
    const result: Http2SettingsObject = {};
    for (let offset = 0; offset < len; offset += 6) {
      const id = readUint16BE(view, offset);
      const value = readUint32BE(view, offset + 2);
      const name = SETTING_ID_TO_NAME[id];
      if (name !== undefined) {
        result[name] = value;
      } else {
        result[String(id)] = value;
      }
    }
    return result;
  },
);
