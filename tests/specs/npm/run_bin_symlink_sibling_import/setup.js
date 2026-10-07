Deno.mkdirSync(".bin", { recursive: true });
Deno.mkdirSync("pkg/bin", { recursive: true });
Deno.mkdirSync("pkg/lib", { recursive: true });

Deno.writeTextFileSync(
  "pkg/bin/cli.js",
  "import { x } from '../lib/x.js';\nconsole.log(x);\n",
);
Deno.writeTextFileSync("pkg/lib/x.js", "export const x = 'ok';\n");
Deno.symlinkSync("../pkg/bin/cli.js", ".bin/cli");
