## Script installer

Deno provides `deno install` to easily install and distribute executable code.

`deno install [OPTIONS...] [URL] [SCRIPT_ARGS...]` will install the script
available at `URL` under the name `EXE_NAME`.

This command creates a thin, executable shell script which invokes `deno` using
the specified CLI flags and main module. It is placed in the installation root's
`bin` directory.

Example:

```shell
$ deno install --allow-net --allow-read https://deno.land/std@$STD_VERSION/http/file_server.ts
[1/1] Compiling https://deno.land/std@$STD_VERSION/http/file_server.ts

✅ Successfully installed file_server.
/Users/deno/.deno/bin/file_server
```

To change the executable name, use `-n`/`--name`:

```shell
deno install --allow-net --allow-read -n serve https://deno.land/std@$STD_VERSION/http/file_server.ts
```

The executable name is inferred by default:

- Attempt to take the file stem of the URL path. The above example would become
  'file_server'.
- If the file stem is something generic like 'main', 'mod', 'index' or 'cli',
  and the path has no parent, take the file name of the parent path. Otherwise
  settle with the generic name.
- If the resulting name has an '@...' suffix, strip it.

To change the installation root, use `--root`:

```shell
deno install --allow-net --allow-read --root /usr/local https://deno.land/std@$STD_VERSION/http/file_server.ts
```

The installation root is determined, in order of precedence:

- `--root` option
- `DENO_INSTALL_ROOT` environment variable
- `$HOME/.deno`

These must be added to the path manually if required.

```shell
echo 'export PATH="$HOME/.deno/bin:$PATH"' >> ~/.bashrc
```

You must specify permissions that will be used to run the script at installation
time.

```shell
deno install --allow-net --allow-read https://deno.land/std@$STD_VERSION/http/file_server.ts -p 8080
```

The above command creates an executable called `file_server` that runs with
network and read permissions and binds to port 8080.

For good practice, use the [`import.meta.main`](../examples/module_metadata.md)
idiom to specify the entry point in an executable script.

Example:

<!-- dprint-ignore -->

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

$ deno install -n awesome_cli https://example.com/awesome/cli.ts
```
