Deno.mkdirSync("node_modules/.bin", { recursive: true });
Deno.mkdirSync("node_modules/pkg/bin", { recursive: true });
Deno.mkdirSync("node_modules/pkg/lib", { recursive: true });

Deno.writeTextFileSync("package.json", "{}\n");
Deno.writeTextFileSync(
  "node_modules/pkg/package.json",
  '{"name":"pkg","version":"1.0.0","type":"module"}\n',
);
Deno.writeTextFileSync(
  "node_modules/pkg/bin/cli.js",
  "import { x } from '../lib/x.js';\nconsole.log('loaded ok:', x);\n",
);
Deno.writeTextFileSync(
  "node_modules/pkg/lib/x.js",
  "export const x = 'OK';\n",
);
Deno.symlinkSync("../pkg/bin/cli.js", "node_modules/.bin/cli");
