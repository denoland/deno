// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

/** FormFile object */
export type FormFile = {
  /** filename  */
  filename: string;
  /** content-type header value of file */
  type: string;
  /** byte size of file */
  size: number;
  /** in-memory content of file. Either content or tempfile is set  */
  content?: Uint8Array;
  /** temporal file path. Set if file size is bigger than specified max-memory size at reading form */
  tempfile?: string;
};

/** Type guard for FormFile */
export function isFormFile(x): x is FormFile {
  return (
    typeof x === "object" &&
    x.hasOwnProperty("filename") &&
    x.hasOwnProperty("type")
  );
}
