# Deno

[![](https://img.shields.io/crates/v/deno.svg)](https://crates.io/crates/deno)
[![Twitter badge][]][Twitter link] [![Discord badge][]][Discord link]
[![YouTube badge][]][YouTube link]

<img align="right" src="https://deno.land/logo.svg" height="150px" alt="the deno mascot dinosaur standing in the rain">

<<<<<<< HEAD
[Deno](https://www.deno.com)
([/ˈdiːnoʊ/](http://ipa-reader.xyz/?text=%CB%88di%CB%90no%CA%8A), pronounced
`dee-no`) is a JavaScript, TypeScript, and WebAssembly runtime with secure
defaults and a great developer experience. It's built on [V8](https://v8.dev/),
[Rust](https://www.rust-lang.org/), and [Tokio](https://tokio.rs/).

Learn more about the Deno runtime
[in the documentation](https://docs.deno.com/runtime/manual).

## Installation

Install the Deno runtime on your system using one of the commands below. Note
that there are a number of ways to install Deno - a comprehensive list of
installation options can be found
[here](https://docs.deno.com/runtime/manual/getting_started/installation).
=======
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
>>>>>>> 8c07f52a7 (1.38.4 (#21398))

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

<<<<<<< HEAD
### Build and install from source

Complete instructions for building Deno from source can be found in the manual
[here](https://docs.deno.com/runtime/manual/references/contributing/building_from_source).

## Your first Deno program

Deno can be used for many different applications, but is most commonly used to
build web servers. Create a file called `server.ts` and include the following
TypeScript code:

```ts
Deno.serve((_req: Request) => {
  return new Response("Hello, world!");
});
```

Run your server with the following command:

```sh
deno run --allow-net server.ts
```

This should start a local web server on
[http://localhost:8000](http://localhost:8000).

Learn more about writing and running Deno programs
[in the docs](https://docs.deno.com/runtime/manual).

## Additional resources

- **[Deno Docs](https://docs.deno.com)**: official guides and reference docs for
  the Deno runtime, [Deno Deploy](https://deno.com/deploy), and beyond.
- **[Deno Standard Library](https://deno.land/std)**: officially supported
  common utilities for Deno programs.
- **[deno.land/x](https://deno.land/x)**: registry for third-party Deno modules.
- **[Developer Blog](https://deno.com/blog)**: Product updates, tutorials, and
  more from the Deno team.

## Contributing

We appreciate your help! To contribute, please read our
[contributing instructions](https://docs.deno.com/runtime/manual/references/contributing/).
=======
[Scoop](https://scoop.sh/) (Windows):

```powershell
scoop install deno
```

Build and install from source using [Cargo](https://crates.io/crates/deno):

```sh
# Install build dependencies
apt install -y cmake protobuf-compiler # Linux
brew install cmake protobuf # macOS

# Build and install Deno
cargo install deno --locked
```

See
[deno_install](https://github.com/denoland/deno_install/blob/master/README.md)
and [releases](https://github.com/denoland/deno/releases) for other options.

### Getting Started

Try [running a simple program](https://examples.deno.land/hello-world):

```sh
deno run https://examples.deno.land/hello-world.ts
```

Or [setup a simple HTTP server](https://examples.deno.land/http-server):

```ts
Deno.serve((_req) => new Response("Hello, World!"));
```

[More Examples](https://examples.deno.land)

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
  product updates and “how to”s about solving technical problems.

### Contributing

We appreciate your help!

To contribute, please read our
[contributing instructions](https://deno.land/manual/references/contributing/).
>>>>>>> 8c07f52a7 (1.38.4 (#21398))

[Build status - Cirrus]: https://github.com/denoland/deno/workflows/ci/badge.svg?branch=main&event=push
[Build status]: https://github.com/denoland/deno/actions
[Twitter badge]: https://img.shields.io/twitter/follow/deno_land.svg?style=social&label=Follow
[Twitter link]: https://twitter.com/intent/follow?screen_name=deno_land
[YouTube badge]: https://img.shields.io/youtube/channel/subscribers/UCqC2G2M-rg4fzg1esKFLFIw?style=social
[YouTube link]: https://www.youtube.com/@deno_land
[Discord badge]: https://img.shields.io/discord/684898665143206084?logo=discord&style=social
[Discord link]: https://discord.gg/deno
