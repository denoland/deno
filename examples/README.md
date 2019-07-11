# Deno example programs

This module contains small scripts that demonstrate use of Deno and its standard library.

You can run these examples by importing them via `deno` command:

```
> deno https://deno.land/std/examples/echo_server.ts --allow-net
```

Some of them are useful CLI programs that can be installed as executables:

`cat.ts` - print file to standard output

```
deno install deno_cat https://deno.land/examples.cat.ts --allow-read
deno_cat file.txt
```

`catj.ts` - print flattened JSON to standard output

```
deno install catj https://deno.land/examples/catj.ts --allow-read
catj example.json
catj file1.json file2.json
echo example.json | catj -
```

`gist.ts` - easily create and upload Gists

```
deno install gist https://deno.land/examples/gist.ts --allow-net --allow-env
export GIST_TOKEN=ABC # Generate at https://github.com/settings/tokens
gist --title "Example gist 1" script.ts
gist --t "Example gist 2" script2.ts
```
