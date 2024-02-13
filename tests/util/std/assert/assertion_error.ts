// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
export class AssertionError extends Error {
  override name = "AssertionError";
  constructor(message: string) {
    super(message);
  }
}
