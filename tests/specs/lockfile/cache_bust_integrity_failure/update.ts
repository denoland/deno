// make the path in the deno_dir differ from the remote server
// and what's stored in the lockfile
const filePath =
  "./deno_dir/remote/http/localhost_PORT4545/3011c891e5bd4172aa2e157e4c688ab6f31e91da9719704a9a54aa63faa99c88";
const text = Deno.readTextFileSync(filePath);
Deno.writeTextFileSync(filePath, "//\n" + text);
