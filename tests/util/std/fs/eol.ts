// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

// End-of-line character for POSIX platforms such as macOS and Linux.
export const LF = "\n" as const;

/** End-of-line character for Windows platforms. */
export const CRLF = "\r\n" as const;

/**
 * End-of-line character evaluated for the current platform.
 *
 * @example
 * ```ts
 * import { EOL } from "https://deno.land/std@$STD_VERSION/fs/eol.ts";
 *
 * EOL; // Returns "\n" on POSIX platforms or "\r\n" on Windows
 * ```
 *
 * @todo(iuioiua): Uncomment the following line upon deprecation of the `EOL`
 * enum.
 */
// export const EOL = Deno.build.os === "windows" ? CRLF : LF;

/**
 * Platform-specific conventions for the line ending format (i.e., the "end-of-line").
 *
 * @deprecated (will be removed in 0.209.0) This will be replaced by an
 * OS-dependent `EOL` constant.
 */
export enum EOL {
  /** Line Feed. Typically used in Unix (and Unix-like) systems. */
  LF = "\n",
  /** Carriage Return + Line Feed. Historically used in Windows and early DOS systems. */
  CRLF = "\r\n",
}

const regDetect = /(?:\r?\n)/g;

/**
 * Detect the EOL character for string input.
 * returns null if no newline.
 *
 * @example
 * ```ts
 * import { detect, EOL } from "https://deno.land/std@$STD_VERSION/fs/mod.ts";
 *
 * const CRLFinput = "deno\r\nis not\r\nnode";
 * const Mixedinput = "deno\nis not\r\nnode";
 * const LFinput = "deno\nis not\nnode";
 * const NoNLinput = "deno is not node";
 *
 * detect(LFinput); // output EOL.LF
 * detect(CRLFinput); // output EOL.CRLF
 * detect(Mixedinput); // output EOL.CRLF
 * detect(NoNLinput); // output null
 * ```
 */
export function detect(content: string): EOL | null {
  const d = content.match(regDetect);
  if (!d || d.length === 0) {
    return null;
  }
  const hasCRLF = d.some((x: string): boolean => x === EOL.CRLF);

  return hasCRLF ? EOL.CRLF : EOL.LF;
}

/**
 * Format the file to the targeted EOL.
 *
 * @example
 * ```ts
 * import { EOL, format } from "https://deno.land/std@$STD_VERSION/fs/mod.ts";
 *
 * const CRLFinput = "deno\r\nis not\r\nnode";
 *
 * format(CRLFinput, EOL.LF); // output "deno\nis not\nnode"
 * ```
 */
export function format(content: string, eol: EOL): string {
  return content.replace(regDetect, eol);
}
