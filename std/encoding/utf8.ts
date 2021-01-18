// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

/** A default TextEncoder instance */
export const encoder = new TextEncoder();

/** Shorthand for new TextEncoder().encode() */
export function encode(input?: string): Uint8Array {
  return encoder.encode(input);
}

/** A default TextDecoder instance */
export const decoder = new TextDecoder();

/** Shorthand for new TextDecoder().decode() */
export function decode(input?: Uint8Array): string {
  return decoder.decode(input);
}
