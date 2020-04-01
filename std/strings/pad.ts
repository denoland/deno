// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/** FillOption Object */
export interface FillOption {
  /** Char to fill in */
  char?: string;
  /** Side to fill in */
  side?: "left" | "right";
  /** If strict, output string can't be greater than strLen*/
  strict?: boolean;
  /** char/string used to specify the string has been truncated */
  strictChar?: string;
  /** Side of truncate */
  strictSide?: "left" | "right";
}

/**
 * Pad helper for strings.
 * Input string is processed to output a string with a minimal length.
 * If the parameter `strict` is set to true, the output string length
 * is equal to the `strLen` parameter.
 * Example:
 *
 *     pad("deno", 6, { char: "*", side: "left" }) // output : "**deno"
 *     pad("deno", 6, { char: "*", side: "right"}) // output : "deno**"
 *     pad("denosorusrex", 6 {
 *       char: "*",
 *       side: "left",
 *       strict: true,
 *       strictSide: "right",
 *       strictChar: "..."
 *     }) // output : "den..."
 *
 * @param input Input string
 * @param strLen Output string lenght
 * @param opts Configuration object
 * @param [opts.char=" "] Character used to fill in
 * @param [opts.side="left"] Side to fill in
 * @param [opts.strict=false] Flag to truncate the string if length > strLen
 * @param [opts.strictChar=""] Character to add if string is truncated
 * @param [opts.strictSide="right"] Side to truncate
 */
export function pad(
  input: string,
  strLen: number,
  opts: FillOption = {
    char: " ",
    strict: false,
    side: "left",
    strictChar: "",
    strictSide: "right",
  }
): string {
  let out = input;
  const outL = out.length;
  if (outL < strLen) {
    if (!opts.side || opts.side === "left") {
      out = out.padStart(strLen, opts.char);
    } else {
      out = out.padEnd(strLen, opts.char);
    }
  } else if (opts.strict && outL > strLen) {
    const addChar = opts.strictChar ? opts.strictChar : "";
    if (opts.strictSide === "left") {
      let toDrop = outL - strLen;
      if (opts.strictChar) {
        toDrop += opts.strictChar.length;
      }
      out = `${addChar}${out.slice(toDrop, outL)}`;
    } else {
      out = `${out.substring(0, strLen - addChar.length)}${addChar}`;
    }
  }
  return out;
}
