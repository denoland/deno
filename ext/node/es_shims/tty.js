const m = Deno[Deno.internal].nodePolyfills.tty;

export const isatty = m.isatty;
export const ReadStream = m.ReadStream;
export const WriteStream = m.WriteStream;

export default m;
