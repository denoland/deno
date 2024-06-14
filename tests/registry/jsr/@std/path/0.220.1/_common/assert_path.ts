// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// Copyright the Browserify authors. MIT License.

export function assertPath(path?: string) {
  if (typeof path !== "string") {
    throw new TypeError(
      `Path must be a string. Received ${JSON.stringify(path)}`,
    );
  }
}
