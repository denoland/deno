/** A default TextDecoder instance */
export const decoder = new TextDecoder();

/** Shorthand for new TextDecoder().decode() */
export function decode(input?: Uint8Array): string {
  return decoder.decode(input);
}
