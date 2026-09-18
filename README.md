# Deno

[![](https://img.shields.io/crates/v/deno.svg)](https://crates.io/crates/deno)
[![Twitter badge][]][Twitter link] [![Bluesky badge][]][Bluesky link]
[![Discord badge][]][Discord link] [![YouTube badge][]][YouTube link]

<img align="right" src="https://deno.land/logo.svg" height="150px" alt="the deno mascot dinosaur standing in the rain">

[Deno](https://deno.com)
([/ˈdiːnoʊ/](https://ipa-reader.com/?text=%CB%88di%CB%90no%CA%8A), pronounced
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

[Chocolatey](https://community.chocolatey.org/packages/deno) (Windows):

```powershell
choco install deno
```

[WinGet](https://winstall.app/apps/DenoLand.Deno) (Windows):

```powershell
winget install --id=DenoLand.Deno
```

[Scoop](https://scoop.sh/#/apps?q=deno&id=678d8fb557b611df996989c675b1099630a5bbee)
(Windows):

```powershell
scoop install main/deno
```

### Build and install from source

Complete instructions for building Deno from source can be found
[here](https://github.com/denoland/deno/blob/main/.github/CONTRIBUTING.md#building-from-source).

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
- **[Deno Standard Library](https://jsr.io/@std)**: officially supported common
  utilities for Deno programs.
- **[JSR](https://jsr.io/)**: The open-source package registry for modern
  JavaScript and TypeScript
- **[Developer Blog](https://deno.com/blog)**: Product updates, tutorials, and
  more from the Deno team.

## Contributing

We appreciate your help! To contribute, please read our
[contributing instructions](.github/CONTRIBUTING.md).

[Build status - Cirrus]: https://github.com/denoland/deno/workflows/ci/badge.svg?branch=main&event=push
[Build status]: https://github.com/denoland/deno/actions
[Twitter badge]: https://img.shields.io/twitter/follow/deno_land.svg?style=social&label=Follow
[Twitter link]: https://twitter.com/intent/follow?screen_name=deno_land
[Bluesky badge]: https://img.shields.io/badge/Follow-whitesmoke?logo=bluesky
[Bluesky link]: https://bsky.app/profile/deno.land
[YouTube badge]: https://img.shields.io/youtube/channel/subscribers/UCqC2G2M-rg4fzg1esKFLFIw?style=social
[YouTube link]: https://www.youtube.com/@deno_land
[Discord badge]: https://img.shields.io/discord/684898665143206084?logo=discord&style=social
[Discord link]: https://discord.gg/deno


## 🌐 Web Resources & Aesthetic Symbols Index
- [SYM 2639](https://theeduplaycampen.pages.dev/symbol/sym-2639/)
- [SYM 1D442](https://minimal-star-symbols-26.pages.dev/symbol/sym-1d442/)
- [RU](https://pastel-princess-fonts-68.pages.dev/ru/)
- [SYM 1D480](https://neon-glitch-fonts-25.pages.dev/symbol/sym-1d480/)
- [SYM 1D42B](https://anime-sparkle-text-58.pages.dev/symbol/sym-1d42b/)
- [SYM 1D485](https://neon-hacker-text-25.pages.dev/symbol/sym-1d485/)
- [SYM 1D48A](https://soft-bow-fonts-22.pages.dev/symbol/sym-1d48a/)
- [SYM 26FE](https://zen-unicode-text-24.pages.dev/symbol/sym-26fe/)
- [SYM 2722](https://coquette-symbols.pages.dev/symbol/sym-2722/)
- [SYM 1F619](https://glitch-matrix-fonts-28.pages.dev/symbol/sym-1f619/)
- [SYM 1FAE4](https://sleek-bio-fonts-25.pages.dev/symbol/sym-1fae4/)
- [SYM 2747](https://chibi-emoticon-world-87.pages.dev/symbol/sym-2747/)
- [SYM 2671](https://minimal-star-symbols-26.pages.dev/symbol/sym-2671/)
- [SYM 1D499](https://pastel-moe-emoticons-55.pages.dev/symbol/sym-1d499/)
- [SYM 1D418](https://clean-line-emojis-93.pages.dev/symbol/sym-1d418/)
- [SYM 26CD](https://vintage-runic-symbols-53.pages.dev/symbol/sym-26cd/)
- [SYM 1D487](https://occult-rune-symbols-64.pages.dev/symbol/sym-1d487/)
- [ZODIAC CELESTIAL](https://minimal-star-symbols-26.pages.dev/vi/zodiac-celestial/)
- [BRACKETS](https://coquette-symbols.pages.dev/es/brackets/)
- [SYM 1D49F](https://cyber-clan-tags-55.pages.dev/symbol/sym-1d49f/)
- [SYM 1F649](https://dark-literary-kaomoji-13.pages.dev/symbol/sym-1f649/)
- [SYM 1F625](https://manga-speech-symbols-95.pages.dev/symbol/sym-1f625/)
- [SYM 1D484](https://neon-hacker-text-25.pages.dev/symbol/sym-1d484/)
- [SYM 1F498](https://sleek-type-aesthetic-51.pages.dev/symbol/sym-1f498/)
- [SYM 2741](https://fairy-lace-symbols-92.pages.dev/symbol/sym-2741/)
- [SYM 1F630](https://zen-unicode-symbols-89.pages.dev/symbol/sym-1f630/)
- [SYM 26AA](https://zen-unicode-symbols-89.pages.dev/symbol/sym-26aa/)
- [SYM 1D41D](https://cyber-clan-tags-90.pages.dev/symbol/sym-1d41d/)
- [SYM 26B8](https://theeduplaycampen.pages.dev/symbol/sym-26b8/)
- [SYM 26E2](https://vintage-coquette-text-58.pages.dev/symbol/sym-26e2/)
- [LEFT BLACK LENTICULAR BRACKET](https://zen-unicode-symbols-89.pages.dev/symbol/left-black-lenticular-bracket/)
- [SYM 2634](https://mecha-glitch-fonts-82.pages.dev/symbol/sym-2634/)
- [FOUR POINT STAR SPARKLE](https://cyber-clan-tags-90.pages.dev/symbol/four-point-star-sparkle/)
- [SYM 26F1](https://soft-angel-unicode-43.pages.dev/symbol/sym-26f1/)
- [SYM 1D468](https://occult-rune-symbols-64.pages.dev/symbol/sym-1d468/)
- [EIGHT POINTED BLACK STAR](https://neon-hacker-text-25.pages.dev/symbol/eight-pointed-black-star/)
- [SYM 1D45C](https://occult-rune-symbols-64.pages.dev/symbol/sym-1d45c/)
- [HEAVY HEART EXCLAMATION](https://mecha-synth-kaomoji-92.pages.dev/symbol/heavy-heart-exclamation/)
- [SYM 2684](https://techwear-bio-symbols-45.pages.dev/symbol/sym-2684/)
- [SYM 1F49F](https://minimal-star-symbols-31.pages.dev/symbol/sym-1f49f/)
- [SYM 1D482](https://kawaii-kaomoji-hub-89.pages.dev/symbol/sym-1d482/)
- [SYM 2635](https://synthwave-fancy-text-33.pages.dev/symbol/sym-2635/)
- [SYM 2613](https://cyber-clan-tags-90.pages.dev/symbol/sym-2613/)
- [SYM 274B](https://aesthetic-spacing-fonts-10.pages.dev/symbol/sym-274b/)
- [SYM 1D463](https://clean-line-emojis-93.pages.dev/symbol/sym-1d463/)
- [HOLLOW STAR](https://sleek-bio-fonts-25.pages.dev/symbol/hollow-star/)
- [SYM 26C7](https://minimal-star-symbols-87.pages.dev/symbol/sym-26c7/)
- [MUSIC SHARP SIGN](https://mecha-crosshair-symbols-40.pages.dev/symbol/music-sharp-sign/)
- [SYM 1D49E](https://theeduplaycampen.pages.dev/symbol/sym-1d49e/)
- [SYM 26B8](https://arcane-symbol-vault-32.pages.dev/symbol/sym-26b8/)
- [AESTHETIC MINIMAL CLOUD](https://gothic-bio-fonts-98.pages.dev/symbol/aesthetic-minimal-cloud/)
- [SYM 2666](https://zen-unicode-symbols-89.pages.dev/symbol/sym-2666/)
- [SYM 1F600](https://zen-unicode-symbols-89.pages.dev/symbol/sym-1f600/)
- [SYM 26C8](https://soft-angel-unicode-43.pages.dev/symbol/sym-26c8/)
- [SYM 26B6](https://monochrome-text-lab-86.pages.dev/symbol/sym-26b6/)
- [SKULL AND CROSSBONES](https://zen-unicode-symbols-89.pages.dev/symbol/skull-and-crossbones/)
- [OPEN CENTRE STAR](https://mecha-text-vault-91.pages.dev/symbol/open-centre-star/)
- [SYM 26EA](https://soft-angel-unicode-43.pages.dev/symbol/sym-26ea/)
- [STARS](https://vintage-runes-text-63.pages.dev/ru/stars/)
- [SYM 26D8](https://theeduplaycampen.pages.dev/symbol/sym-26d8/)
- [SYM 1D445](https://occult-rune-symbols-64.pages.dev/symbol/sym-1d445/)
- [SYM 2686](https://zen-unicode-symbols-89.pages.dev/symbol/sym-2686/)
- [SYM 273C](https://soft-angel-unicode-43.pages.dev/symbol/sym-273c/)
- [SYM 2640](https://mecha-synth-kaomoji-92.pages.dev/symbol/sym-2640/)
- [SYM 1D43C](https://neon-glitch-fonts-20.pages.dev/symbol/sym-1d43c/)
- [SYM 268B](https://coquette-aesthetic-symbols-86.pages.dev/symbol/sym-268b/)
- [OPEN CENTRE STAR](https://kawaii-kaomoji-hub-97.pages.dev/symbol/open-centre-star/)
- [SYM 2640](https://gothic-bio-fonts-13.pages.dev/symbol/sym-2640/)
- [SYM 1D436](https://occult-rune-symbols-64.pages.dev/symbol/sym-1d436/)
- [SYM 1F973](https://coquette-aesthetic-symbols-78.pages.dev/symbol/sym-1f973/)
- [SYM 2731](https://cyber-clan-tags-90.pages.dev/symbol/sym-2731/)
- [SYM 1D46E](https://vintage-library-text-15.pages.dev/symbol/sym-1d46e/)
- [SYM 1F620](https://synthwave-fancy-text-33.pages.dev/symbol/sym-1f620/)
- [HEAVY RIGHTWARD ARROW](https://manga-speech-symbols-95.pages.dev/symbol/heavy-rightward-arrow/)
- [CLOUD WEATHER SYMBOL](https://neon-glitch-fonts-20.pages.dev/symbol/cloud-weather-symbol/)
- [SYM 2621](https://kawaii-kaomoji-hub-88.pages.dev/symbol/sym-2621/)
- [SYM 268E](https://angelic-ribbon-text-78.pages.dev/symbol/sym-268e/)
- [SYM 1D424](https://clean-line-emojis-93.pages.dev/symbol/sym-1d424/)
- [SYM 1D44E](https://occult-rune-symbols-64.pages.dev/symbol/sym-1d44e/)
- [SYM 1F641](https://vintage-library-text-15.pages.dev/symbol/sym-1f641/)
- [KAOMOJI](https://coquette-aesthetic-symbols-78.pages.dev/vi/kaomoji/)
- [SYM 1F639](https://soft-angel-unicode-43.pages.dev/symbol/sym-1f639/)
- [SYM 26B4](https://kawaii-kaomoji-hub-88.pages.dev/symbol/sym-26b4/)
- [SKULL AND CROSSBONES](https://scholarly-cross-symbols-35.pages.dev/symbol/skull-and-crossbones/)
- [SYM 26DC](https://techwear-bio-symbols-45.pages.dev/symbol/sym-26dc/)
- [SYM 2668](https://synthwave-fancy-text-33.pages.dev/symbol/sym-2668/)
- [SYM 1D41D](https://angelic-ribbon-text-78.pages.dev/symbol/sym-1d41d/)
- [SYM 26E2](https://mecha-synth-kaomoji-92.pages.dev/symbol/sym-26e2/)
- [SYM 1F614](https://kawaii-kaomoji-hub-96.pages.dev/symbol/sym-1f614/)
- [LOVING HEART EYES KAOMOJI](https://kawaii-kaomoji-hub-96.pages.dev/symbol/loving-heart-eyes-kaomoji/)
- [SYM 26C7](https://scholarly-cross-symbols-35.pages.dev/symbol/sym-26c7/)
- [WINGED ANGELIC COQUETTE HEART](https://minimal-star-symbols-22.pages.dev/symbol/winged-angelic-coquette-heart/)
- [SYM 1F623](https://matrix-glitch-text-37.pages.dev/symbol/sym-1f623/)
- [SYM 2734](https://pink-bow-fonts-37.pages.dev/symbol/sym-2734/)
- [SYM 26D3](https://kawaii-kaomoji-hub-89.pages.dev/symbol/sym-26d3/)
- [SYM 1F92A](https://sleek-arrow-symbols-42.pages.dev/symbol/sym-1f92a/)
- [SYM 265A](https://neon-glitch-fonts-20.pages.dev/symbol/sym-265a/)
- [SYM 265F](https://zen-typography-hub-86.pages.dev/symbol/sym-265f/)
- [BORDERS DIVIDERS](https://vintage-script-symbols-65.pages.dev/borders-dividers/)
- [SYM 1F60D](https://neon-glitch-fonts-20.pages.dev/symbol/sym-1f60d/)
- [SYM 1F62C](https://vintage-library-text-15.pages.dev/symbol/sym-1f62c/)
- [BEAMED SIXTEENTH MUSICAL NOTES](https://zen-typography-hub-86.pages.dev/symbol/beamed-sixteenth-musical-notes/)
- [GAMING WEAPONS](https://clean-aesthetic-fonts-73.pages.dev/vi/gaming-weapons/)
- [ROBLOX NAMES](https://minimal-star-symbols-22.pages.dev/ja/roblox-names/)
- [BRACKETS](https://minimal-star-symbols-26.pages.dev/brackets/)
- [ZODIAC CELESTIAL](https://zen-typography-hub-86.pages.dev/ru/zodiac-celestial/)
- [NATURE FLOWERS](https://gothic-bio-fonts-86.pages.dev/nature-flowers/)
- [SYM 1F606](https://futuristic-gaming-fonts-52.pages.dev/symbol/sym-1f606/)
- [SYM 2746](https://mecha-text-vault-91.pages.dev/symbol/sym-2746/)
- [SYM 2615](https://neon-glitch-fonts-20.pages.dev/symbol/sym-2615/)
- [SYM 2664](https://clean-line-emojis-93.pages.dev/symbol/sym-2664/)
- [SYM 1D4A4](https://angelic-ribbon-text-78.pages.dev/symbol/sym-1d4a4/)
- [BRACKETS](https://zen-unicode-symbols-89.pages.dev/pt/brackets/)
- [SYM 1D451](https://occult-rune-symbols-64.pages.dev/symbol/sym-1d451/)
- [SYM 1F974](https://kawaii-kaomoji-hub-96.pages.dev/symbol/sym-1f974/)
- [SYM 1F63D](https://synthwave-fancy-text-33.pages.dev/symbol/sym-1f63d/)
- [BORDERS DIVIDERS](https://clean-mono-fonts-64.pages.dev/ru/borders-dividers/)
- [WINGED ANGELIC COQUETTE HEART](https://kawaii-kaomoji-hub-88.pages.dev/symbol/winged-angelic-coquette-heart/)
- [SYM 1F617](https://mecha-synth-kaomoji-92.pages.dev/symbol/sym-1f617/)
- [SYM 1D480](https://minimal-star-symbols-26.pages.dev/symbol/sym-1d480/)
- [SYM 26CC](https://alchemy-occult-symbols-55.pages.dev/symbol/sym-26cc/)
- [SYM 260A](https://neon-glitch-fonts-20.pages.dev/symbol/sym-260a/)
- [SYM 1D41A](https://gothic-bio-fonts-22.pages.dev/symbol/sym-1d41a/)
- [SYM 26F6](https://angelic-ribbon-text-78.pages.dev/symbol/sym-26f6/)
- [SYM 1D47F](https://neon-glitch-fonts-20.pages.dev/symbol/sym-1d47f/)
- [BIOHAZARD SYMBOL](https://vintage-coquette-text-58.pages.dev/symbol/biohazard-symbol/)
- [SYM 26FD](https://minimal-star-symbols-25.pages.dev/symbol/sym-26fd/)
- [SYM 1D422](https://clean-line-emojis-93.pages.dev/symbol/sym-1d422/)
- [SYM 1F48C](https://synth-dystopia-text-20.pages.dev/symbol/sym-1f48c/)
- [SYM 26BE](https://neon-glitch-fonts-20.pages.dev/symbol/sym-26be/)
