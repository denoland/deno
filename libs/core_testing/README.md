# Checkin: A Tiny Testing Tool for `deno_core`

<img src="docs/logo.png" align="right" width="100" />

## Why Checkin?

Dino : _Deno_ :: Chicken : _Checkin_

Also because it _checks_ how deno_core works. Yuk yuk.

## Overview

_Checkin_ is a tiny testing tool designed to exercise the functionality of
`deno_core`. It implements a very small standard library which is just enough to
exercise all of the components that we expose to
[Deno](https://github.com/denoland/deno) without reinventing the wheel.

## Modules

Modules in _Checkin_ are written in TypeScript and transpiled before loading.
