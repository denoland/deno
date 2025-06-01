// Copyright 2018-2025 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.
// deno-lint-ignore-file no-explicit-any

export function createSecureContext(options: any) {
  return {
    ca: options?.ca,
    cert: options?.cert,
    key: options?.key,
  };
}

export default {
  createSecureContext,
};
