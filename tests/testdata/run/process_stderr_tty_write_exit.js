import process from "node:process";

const beforeHasRef = process.stderr._handle.hasRef();

process.stderr.write("\0".repeat(32 * 1024), () => {
  const callbackHasRef = process.stderr._handle.hasRef();
  Deno.writeTextFileSync(
    Deno.args[0],
    `stderr write callback before_has_ref=${beforeHasRef} callback_has_ref=${callbackHasRef}`,
  );
});
