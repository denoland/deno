// `Deno.BrowserWindow`, its `visible` option, and `printToPdf()` only exist in
// the `deno desktop` type libs, so this file fails to type-check without
// `--desktop` and succeeds with it.
const win = new Deno.BrowserWindow({ visible: false });

// printToPdf() resolves with the PDF bytes; the optional `path` also writes it.
const bytes: Promise<Uint8Array> = win.printToPdf();
const withPath: Promise<Uint8Array> = win.printToPdf({ path: "out.pdf" });

console.log(win, bytes, withPath);
