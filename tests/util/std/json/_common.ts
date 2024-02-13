// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import type { JsonValue } from "./common.ts";

/** JSON.parse with detailed error message. */
export function parse(text: string): JsonValue {
  try {
    return JSON.parse(text);
  } catch (error: unknown) {
    if (error instanceof Error) {
      // Truncate the string so that it is within 30 lengths.
      const truncatedText = 30 < text.length ? `${text.slice(0, 30)}...` : text;
      throw new (error.constructor as ErrorConstructor)(
        `${error.message} (parsing: '${truncatedText}')`,
      );
    }
    throw error;
  }
}
