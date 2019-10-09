/** A default TextEncoder instance */
export const encoder = new TextEncoder();

/** Shorthand for new TextEncoder().encode() */
export function encode(input?: string): Uint8Array {
  return encoder.encode(input);
}
