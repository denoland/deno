/* eslint-disable */

import { assert, assertEquals } from "../testing/asserts.ts";
import * as path from "../path/mod.ts";
import Context from "./snapshot_preview1.ts";

if (import.meta.main) {
  const options = JSON.parse(Deno.args[0]);
  const binary = await Deno.readFile(Deno.args[1]);
  const module = await WebAssembly.compile(binary);

  const context = new Context({
    env: options.env,
    args: options.args,
    preopens: options.preopens,
  });

  const instance = new WebAssembly.Instance(module, {
    wasi_snapshot_preview1: context.exports,
  });

  context.memory = instance.exports.memory;

  instance.exports._start();
} else {
  const rootdir = path.dirname(path.fromFileUrl(import.meta.url));
  const testdir = path.join(rootdir, "testdata");
  const outdir = path.join(testdir, "snapshot_preview1");

  for await (const entry of Deno.readDir(testdir)) {
    if (!entry.name.endsWith(".rs")) {
      continue;
    }

    const process = Deno.run({
      cmd: [
        "rustc",
        "--target",
        "wasm32-wasi",
        "--out-dir",
        outdir,
        path.join(testdir, entry.name),
      ],
      stdout: "inherit",
      stderr: "inherit",
    });

    const status = await process.status();
    assert(status.success);

    process.close();

    // TODO(caspervonb) allow the prelude to span multiple lines
    const source = await Deno.readTextFile(path.join(testdir, entry.name));
    const prelude = source.match(/^\/\/\s*\{.*/);
    if (prelude) {
      const basename = entry.name.replace(/.rs$/, ".json");
      await Deno.writeTextFile(
        path.join(outdir, basename),
        prelude[0].slice(2),
      );
    }
  }

  for await (const entry of Deno.readDir(outdir)) {
    if (!entry.name.endsWith(".wasm")) {
      continue;
    }

    Deno.test(entry.name, async function () {
      const basename = entry.name.replace(/\.wasm$/, ".json");
      const prelude = await Deno.readTextFile(path.resolve(outdir, basename));
      const options = JSON.parse(prelude);

      await Deno.mkdir(`${testdir}/scratch`);

      try {
        const process = await Deno.run({
          cwd: testdir,
          cmd: [
            `${Deno.execPath()}`,
            "run",
            "--quiet",
            "--unstable",
            "--allow-all",
            import.meta.url,
            prelude,
            path.resolve(outdir, entry.name),
          ],
          stdin: "piped",
          stdout: "piped",
          stderr: "piped",
        });

        if (options.stdin) {
          const stdin = new TextEncoder().encode(options.stdin);
          await Deno.writeAll(process.stdin, stdin);
        }

        process.stdin.close();

        const stdout = await Deno.readAll(process.stdout);

        if (options.stdout) {
          assertEquals(new TextDecoder().decode(stdout), options.stdout);
        } else {
          await Deno.writeAll(Deno.stdout, stdout);
        }

        process.stdout.close();

        const stderr = await Deno.readAll(process.stderr);

        if (options.stderr) {
          assertEquals(new TextDecoder().decode(stderr), options.stderr);
        } else {
          await Deno.writeAll(Deno.stderr, stderr);
        }

        process.stderr.close();

        if (options.files) {
          for (const [key, value] of Object.entries(options.files)) {
            assertEquals(value, await Deno.readTextFile(`${testdir}/${key}`));
          }
        }

        const status = await process.status();
        assertEquals(status.code, options.exitCode ? +options.exitCode : 0);

        process.close();
      } catch (err) {
        throw err;
      } finally {
        await Deno.remove(`${testdir}/scratch`, { recursive: true });
      }
    });
  }
}
