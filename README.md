# Deno

[![](https://img.shields.io/crates/v/deno.svg)](https://crates.io/crates/deno)
[![Twitter badge][]][Twitter link] [![Discord badge][]][Discord link]
[![YouTube badge][]][Youtube link]

<img align="right" src="https://deno.land/logo.svg" height="150px" alt="the deno mascot dinosaur standing in the rain">

[Deno](https://deno.com/runtime) is a _simple_, _modern_ and _secure_ runtime
for **JavaScript** and **TypeScript** that uses V8 and is built in Rust.

### Features

- [Secure by default.](https://deno.land/manual/basics/permissions) No file,
  network, or environment access, unless explicitly enabled.
- Provides
  [web platform functionality and APIs](https://deno.land/manual/runtime/web_platform_apis),
  e.g. using ES modules, web workers, and `fetch()`.
- Supports
  [TypeScript out of the box](https://deno.land/manual/advanced/typescript).
- Ships only a single executable file.
- [Built-in tooling](https://deno.land/manual/tools#built-in-tooling) including
  `deno test`, `deno fmt`, `deno bench`, and more.
- Includes [a set of reviewed standard modules](https://deno.land/std/)
  guaranteed to work with Deno.
- [Supports npm.](https://deno.land/manual/node)

### Install

Shell (Mac, Linux):

```sh
curl -fsSL https://deno.land/install.sh | sh
```

PowerShell (Windows):

```powershell
irm https://deno.land/install.ps1 | iex
```

[Homebrew](https://formulae.brew.sh/formula/deno) (Mac):

```sh
brew install deno
```

[Chocolatey](https://chocolatey.org/packages/deno) (Windows):

```powershell
choco install deno
```

[Scoop](https://scoop.sh/) (Windows):

```powershell
scoop install deno
```

Build and install from source using [Cargo](https://crates.io/crates/deno):

```sh
cargo install deno --locked
```

See
[deno_install](https://github.com/denoland/deno_install/blob/master/README.md)
and [releases](https://github.com/denoland/deno/releases) for other options.

### Getting Started

Try [running a simple program](https://examples.deno.land/hello-world):

```sh
deno run https://deno.land/std/examples/welcome.ts
```

Or [setup a simple HTTP server](https://examples.deno.land/http-server):

```ts
import { serve } from "https://deno.land/std@0.182.0/http/server.ts";

serve((_req) => new Response("Hello, World!"));
```

[More examples](https://examples.deno.land/).

### Additional Resources

- **[The Deno Manual](https://deno.land/manual)** is a great starting point for
  [additional examples](https://deno.land/manual/examples),
  [setting up your environment](https://deno.land/manual/getting_started/setup_your_environment),
  [using npm](https://deno.land/manual/node), and more.
- **[Runtime API reference](https://deno.land/api)** documents all APIs built
  into Deno CLI.
- **[Deno Standard Modules](https://deno.land/std)** do not have external
  dependencies and are reviewed by the Deno core team.
- **[deno.land/x](https://deno.land/x)** is the registry for third party
  modules.
- **[Blog](https://deno.com/blog)** is where the Deno team shares important
  product updates and "how to"s, about solving technical problems.

### Contributing

We appreciate your help!

To contribute, please read our
[contributing instructions](https://deno.land/manual/contributing).

[Build Status - Cirrus]: https://github.com/denoland/deno/workflows/ci/badge.svg?branch=main&event=push
[Build status]: https://github.com/denoland/deno/actions
[Twitter badge]: https://img.shields.io/twitter/follow/deno_land.svg?style=social&label=Follow
[Twitter link]: https://twitter.com/intent/follow?screen_name=deno_land
[YouTube badge]: https://img.shields.io/youtube/channel/subscribers/UCqC2G2M-rg4fzg1esKFLFIw?style=social
[YouTube link]: https://www.youtube.com/@deno_land
[Discord badge]: https://img.shields.io/discord/684898665143206084?logo=discord&style=social
[Discord link]: https://discord.gg/deno
