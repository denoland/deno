## Setup your environment

To productively get going with Deno you should set up your environment. This
means setting up shell autocomplete, environmental variables and your editor or
IDE of choice.

### Environmental variables

There are several env vars that control how Deno behaves:

`DENO_DIR` defaults to `$HOME/.deno` but can be set to any path to control where
generated and cached source code is written and read to.

`NO_COLOR` will turn off color output if set. See https://no-color.org/. User
code can test if `NO_COLOR` was set without having `--allow-env` by using the
boolean constant `Deno.noColor`.

### Shell autocomplete

You can generate completion script for your shell using the
`deno completions <shell>` command. The command outputs to stdout so you should
redirect it to an appropriate file.

The supported shells are:

- zsh
- bash
- fish
- powershell
- elvish

Example:

```shell
deno completions bash > /usr/local/etc/bash_completion.d/deno.bash
source /usr/local/etc/bash_completion.d/deno.bash
```

### Editors and IDEs

Because Deno requires the use of file extensions for module imports and allows
http imports, and the most editors and language servers do not natively support
this at the moment, many editors will throw errors about being unable to find
files or imports having unnecessary file extensions.

The community has developed extensions for some editors to solve these issues:

- [VS Code](https://marketplace.visualstudio.com/items?itemName=axetroy.vscode-deno)
  by [@axetroy](https://github.com/axetroy).

Support for JetBrains IDEs is not yet available, but you can follow and upvote
these issues to stay up to date:

- https://youtrack.jetbrains.com/issue/WEB-41607
- https://youtrack.jetbrains.com/issue/WEB-42983
- https://youtrack.jetbrains.com/issue/WEB-31667

If you don't see your favorite IDE on this list, maybe you can develop an
extension. Our [community Discord group](https://discord.gg/TGMHGv6) can give
you some pointers on where to get started.
