// `Notification` is only available in the `deno desktop` type libs, so this
// fails to type-check without `--desktop` and succeeds with it.
const n: Notification = new Notification("hi");
console.log(n);

// Desktop-specific APIs live under the `Deno.desktop` namespace, also only
// available with `--desktop`.
const win = new Deno.desktop.BrowserWindow();
console.log(win);
