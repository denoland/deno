// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { hasOwnProperty } from "../util/has_own_property.ts";

/** FormFile object */
export interface FormFile {
  /** filename  */
  filename: string;
  /** content-type header value of file */
  type: string;
  /** byte size of file */
  size: number;
  /** in-memory content of file. Either content or tempfile is set  */
  content?: Uint8Array;
  /** temporal file path.
   * Set if file size is bigger than specified max-memory size at reading form
   * */
  tempfile?: string;
}

/** Type guard for FormFile */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function isFormFile(x: any): x is FormFile {
  return hasOwnProperty(x, "filename") && hasOwnProperty(x, "type");
}
