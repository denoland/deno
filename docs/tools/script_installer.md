## Script installer

Deno provides `deno install` to easily install and distribute executable code.

`deno install [FLAGS...] [EXE_NAME] [URL] [SCRIPT_ARGS...]` will install the
script available at `URL` under the name `EXE_NAME`.

This command creates a thin, executable shell script which invokes `deno` using
the specified CLI flags and main module. It is place in the installation root's
`bin` directory.

Example:

```shell
$ deno install --allow-net --allow-read file_server https://deno.land/std/http/file_server.ts
[1/1] Compiling https://deno.land/std/http/file_server.ts

✅ Successfully installed file_server.
/Users/deno/.deno/bin/file_server
```

To change the installation root, use `--root`:

```shell
$ deno install --allow-net --allow-read --root /usr/local file_server https://deno.land/std/http/file_server.ts
```

The installation root is determined, in order of precedence:

- `--root` option
- `DENO_INSTALL_ROOT` environment variable
- `$HOME/.deno`

These must be added to the path manually if required.

```shell
$ echo 'export PATH="$HOME/.deno/bin:$PATH"' >> ~/.bashrc
```

You must specify permissions that will be used to run the script at installation
time.

```shell
$ deno install --allow-net --allow-read file_server https://deno.land/std/http/file_server.ts 8080
```

The above command creates an executable called `file_server` that runs with
write and read permissions and binds to port 8080.

For good practice, use the
[`import.meta.main`](#testing-if-current-file-is-the-main-program) idiom to
specify the entry point in an executable script.

Example:

```ts
// https://example.com/awesome/cli.ts
async function myAwesomeCli(): Promise<void> {
  -- snip --
}

if (import.meta.main) {
  myAwesomeCli();
}
```

When you create an executable script make sure to let users know by adding an
example installation command to your repository:

```shell
# Install using deno install

$ deno install awesome_cli https://example.com/awesome/cli.ts
```
