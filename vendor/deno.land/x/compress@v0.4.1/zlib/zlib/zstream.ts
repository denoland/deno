export default class ZStream {
  /* next input byte */
  input: Uint8Array | null = null; // JS specific, because we have no pointers
  next_in = 0;
  /* number of bytes available at input */
  avail_in = 0;
  /* total number of input bytes read so far */
  total_in = 0;
  /* next output byte should be put there */
  output: Uint8Array | null = null; // JS specific, because we have no pointers
  next_out = 0;
  /* remaining free space at output */
  avail_out = 0;
  /* total number of bytes output so far */
  total_out = 0;
  /* last error message, NULL if no error */
  msg = "" /*Z_NULL*/;
  /* not visible by applications */
  state: any = null;
  /* best guess about the data type: binary or text */
  data_type = 2 /*Z_UNKNOWN*/;
  /* adler32 value of the uncompressed data */
  adler = 0;
}
