import process from "node:process";

function handleHasRef(stream) {
  return stream._handle?.hasRef?.();
}

const beforeHasRef = handleHasRef(process.stderr);

process.stderr.write("\0".repeat(32 * 1024), () => {
  const callbackHasRef = handleHasRef(process.stderr);
  const refState = beforeHasRef === undefined && callbackHasRef === undefined
    ? "handle_has_ref=unavailable"
    : `before_has_ref=${beforeHasRef} callback_has_ref=${callbackHasRef}`;
  Deno.writeTextFileSync(
    Deno.args[0],
    `stderr write callback ${refState}`,
  );
});
