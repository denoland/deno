# deno_installer

Install remote or local script as executables.

## Installation

`installer` can be installed using itself:

```sh
deno -A https://deno.land/std/installer/mod.ts deno_installer https://deno.land/std/installer/mod.ts -A
```

## Usage

Install script

```sh
# remote script
$ deno_installer file_server https://deno.land/std/http/file_server.ts --allow-net --allow-read
> [1/1] Compiling https://deno.land/std/http/file_server.ts
>
> ✅ Successfully installed file_server.
> ~/.deno/bin/file_server

# local script
$ deno_installer file_server ./deno_std/http/file_server.ts --allow-net --allow-read
> [1/1] Compiling file:///dev/deno_std/http/file_server.ts
>
> ✅ Successfully installed file_server.
> ~/.deno/bin/file_server
```

Run installed script:

```sh
$ file_server
HTTP server listening on http://0.0.0.0:4500/
```

## Custom installation directory

By default installer uses `~/.deno/bin` to store installed scripts so make sure it's in your `$PATH`.

```
echo 'export PATH="$HOME/.deno/bin:$PATH"' >> ~/.bashrc # change this to your shell
```

If you prefer to change installation directory use `-d` or `--dir` flag.

```
$ deno_installer --dir /usr/local/bin file_server ./deno_std/http/file_server.ts --allow-net --allow-read
> [1/1] Compiling file:///dev/deno_std/http/file_server.ts
>
> ✅ Successfully installed file_server.
> /usr/local/bin/file_server
```

## Update installed script

```sh
$ deno_installer file_server https://deno.land/std/http/file_server.ts --allow-net --allow-read
> ⚠️  file_server is already installed, do you want to overwrite it? [yN]
> y
>
> [1/1] Compiling file:///dev/deno_std/http/file_server.ts
>
> ✅ Successfully installed file_server.
```

Show help

```sh
$ deno_installer --help
> deno installer
  Install remote or local script as executables.

USAGE:
  deno -A https://deno.land/std/installer/mod.ts [OPTIONS] EXE_NAME SCRIPT_URL [FLAGS...]

ARGS:
  EXE_NAME  Name for executable
  SCRIPT_URL  Local or remote URL of script to install
  [FLAGS...]  List of flags for script, both Deno permission and script specific
              flag can be used.

OPTIONS:
  -d, --dir <PATH> Installation directory path (defaults to ~/.deno/bin)
```
