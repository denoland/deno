// Mutates the workspace between `deno install` runs so the spec can assert
// that the previous state's `node_modules/.bin` entries are pruned.

function editJson(path: string, edit: (json: any) => void) {
  const json = JSON.parse(Deno.readTextFileSync(path));
  edit(json);
  Deno.writeTextFileSync(path, `${JSON.stringify(json, null, 2)}\n`);
}

switch (Deno.args[0]) {
  case "rename-bin":
    // `tool` renames its executable: `.bin/toolname` must not survive.
    editJson("packages/tool/package.json", (json) => {
      json.bin = { renamed: "./cli.js" };
    });
    break;
  case "remove-bin":
    // `tool` drops its `bin` field entirely.
    editJson("packages/tool/package.json", (json) => {
      delete json.bin;
    });
    break;
  case "remove-member":
    // `other` leaves the workspace: its entry would otherwise be left behind
    // as a dangling symlink.
    editJson("deno.json", (json) => {
      json.workspace = json.workspace.filter((m: string) =>
        m !== "./packages/other"
      );
    });
    Deno.removeSync("packages/other", { recursive: true });
    break;
  default:
    throw new Error(`unknown edit: ${Deno.args[0]}`);
}

console.log("edited");
