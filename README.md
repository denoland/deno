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


## 🌐 Web Resources & Interactive Index
- [CATEGORY ART32](https://themindplays.pages.dev/category-art32.html)
- [CONNECT 2 CARS](https://studyplayings.web.app/connect-2-cars.html)
- [CATEGORY CASUAL](https://studyquests.pages.dev/category-casual.html)
- [FEED ME MONSTERS IDLE BATTLE](https://themindplays.pages.dev/feed-me-monsters-idle-battle.html)
- [CATEGORY BASKETBALL](https://themindplays.pages.dev/category-basketball.html)
- [STICKMAN HALLOWEEN SURVIVE](https://thelearnquester.web.app/stickman-halloween-survive.html)
- [SUPER STOCK STACK](https://thequizzone.pages.dev/super-stock-stack.html)
- [SNOW RACE 3D FUN RACING](https://studyplayings.web.app/snow-race-3d-fun-racing.html)
- [PUMPKING VS MUMMY](https://themindplays.pages.dev/pumpking-vs-mummy.html)
- [HAWAII MATCH 6](https://quizverses-9d2f2.web.app/hawaii-match-6.html)
- [CATEGORY SPACE57](https://quizverses.github.io/category-space57.html)
- [CATEGORY DESTROY256](https://learnquesters.pages.dev/category-destroy256.html)
- [CATEGORY FASHION105](https://themindplays.pages.dev/category-fashion105.html)
- [CATEGORY SANDBOX](https://studyquests.github.io/category-sandbox.html)
- [CATEGORY CASUAL 4](https://themindplaying.web.app/category-casual-4.html)
- [DAILY JEWELS BLITZ MAHJONG](https://learnquester.pages.dev/daily-jewels-blitz-mahjong.html)
- [MERGE HEROES](https://iskillplay.web.app/merge-heroes.html)
- [CATEGORY RPG](https://themindskillplayplay.pages.dev/category-rpg.html)
- [MERGE TOWN](https://quizverses-9d2f2.web.app/merge-town.html)
- [INDEX41](https://themindplays.pages.dev/index41.html)
- [DEFORM IT](https://learnquester.pages.dev/deform-it.html)
- [CATEGORY IO](https://quizverses-9d2f2.web.app/category-io.html)
- [FINGER HEART MONSTER REFILL](https://quizverses.github.io/finger-heart-monster-refill.html)
- [MAHJONG TOUR](https://themindplay.pages.dev/mahjong-tour.html)
- [CAT ESCAPE](https://learnquester.pages.dev/cat-escape.html)
- [REAL MOTORBIKE SUPER HERO STUNT 3D](https://themindplays.pages.dev/real-motorbike-super-hero-stunt-3d.html)
- [STICKER PUZZLE BOOK](https://themindskillplayplay.pages.dev/sticker-puzzle-book.html)
- [CANNONS BLAST 3D](https://themindplaying.web.app/cannons-blast-3d.html)
- [CATEGORY MATH29](https://themindskillplayplay.pages.dev/category-math29.html)
- [INDEX8](https://learnquester.pages.dev/index8.html)
- [CATEGORY RACING DRIVING](https://themindskillplayplay.pages.dev/category-racing-driving.html)
- [MAGIC BOTTLES](https://learnquester.pages.dev/magic-bottles.html)
- [WORD HUNT](https://thelearnquester.web.app/word-hunt.html)
- [CATEGORY FPS](https://themindskillplayplay.pages.dev/category-fps.html)
- [HOLE BATTLEIO](https://thelearnquester.web.app/hole-battleio.html)
- [SUPERMARKET CASHIER SIMULATOR](https://themindskillplayplay.pages.dev/supermarket-cashier-simulator.html)
- [COLOR SCREW RESCUE PUZZLE](https://studyplayings.web.app/color-screw-rescue-puzzle.html)
- [CATEGORY CAN T STOP PLAYING215](https://quizverses.pages.dev/category-can-t-stop-playing215.html)
- [CATEGORY SPACE57](https://themindplaying.web.app/category-space57.html)
- [RAGDOLL JUMP](https://learnquesters.pages.dev/ragdoll-jump.html)
- [MERGE BALLS SHOOTER 2048 CONNECT FRUITS](https://themindplaying.web.app/merge-balls-shooter-2048-connect-fruits.html)
- [HIDDEN OBJECTS VACATION IN BRAZIL](https://quizverses-9d2f2.web.app/hidden-objects-vacation-in-brazil.html)
- [CATEGORY FPS174](https://themindskillplayplay.pages.dev/category-fps174.html)
- [CATEGORY MAKEUP CATEGORY](https://skillplay.github.io/category-makeup-category.html)
- [LOVIE CHICS SPRING BREAK FASHION](https://iskillplay.web.app/lovie-chics-spring-break-fashion.html)
- [INDEX15](https://quizverses.github.io/index15.html)
- [CATEGORY BLOCK94](https://themindplays.pages.dev/category-block94.html)
- [DTA 2 MANIAC](https://themindskillplayplay.pages.dev/dta-2-maniac.html)
- [GUN BUILDER](https://studyplayings.web.app/gun-builder.html)
- [CATEGORY SOCCER](https://quizverses.github.io/category-soccer.html)
- [ITALIAN ANIMALS CREATE YOUR OWN BRAINROT](https://themindplays.pages.dev/italian-animals-create-your-own-brainrot.html)
- [CATEGORY PLATFORM260](https://quizverses.github.io/category-platform260.html)
- [CATEGORY ESCAPE](https://themindplaying.web.app/category-escape.html)
- [CATEGORY WAR GAME](https://iskillplay.web.app/category-war-game.html)
- [ATOMIC MERGE 2048](https://themindplay.pages.dev/atomic-merge-2048.html)
- [CATEGORY CRAFTING45](https://quizverses.github.io/category-crafting45.html)
- [CATEGORY FASHION105](https://quizverses.github.io/category-fashion105.html)
- [INDEX8](https://quizverses.github.io/index8.html)
- [CAKE SORT](https://themindplays.pages.dev/cake-sort.html)
- [HOUSE DEEP CLEAN SIM](https://themindplaying.web.app/house-deep-clean-sim.html)
- [CATEGORY MERGE GAMES](https://iskillplay.web.app/category-merge-games.html)
- [FISH STORY 4](https://themindplay.pages.dev/fish-story-4.html)
- [CATEGORY HORROR](https://themindplaying.web.app/category-horror.html)
- [PUT THE FRUIT TOGETHER](https://themindplay.pages.dev/put-the-fruit-together.html)
- [CRASH THE ROBOT](https://themindplay.pages.dev/crash-the-robot.html)
- [CATEGORY CONTROLLER59](https://iskillplay.web.app/category-controller59.html)
- [INDEX10](https://quizverses.github.io/index10.html)
- [WINTER HEXA STACK](https://themindplays.pages.dev/winter-hexa-stack.html)
- [GUN EVOLUTION](https://themindskillplayplay.pages.dev/gun-evolution.html)
- [CATEGORY PUZZLE 4](https://quizverses.github.io/category-puzzle-4.html)
- [RACING BALL ADVENTURE](https://quizverses-9d2f2.web.app/racing-ball-adventure.html)
- [CATEGORY SIMULATION 2](https://learnquesters.pages.dev/category-simulation-2.html)
- [FISH FEEDING](https://studyplayings.pages.dev/fish-feeding.html)
- [CATEGORY FASHION](https://learnquester.pages.dev/category-fashion.html)
- [HELIX CRUSH](https://learnquester.pages.dev/helix-crush.html)
- [MONEY CHASER CITY PARKOUR GAME](https://themindskillplayplay.pages.dev/money-chaser-city-parkour-game.html)
- [STICKMAN DISMOUNT SIMULATOR](https://iskillplay.web.app/stickman-dismount-simulator.html)
- [SPRUNKI CHALLENGE](https://learnquester.github.io/sprunki-challenge.html)
- [THIEF STICK PUZZLE MAN ESCAPE](https://learnquesters.pages.dev/thief-stick-puzzle-man-escape.html)
- [CATEGORY PARTY23](https://thelearnquester.web.app/category-party23.html)
- [CATEGORY FARMING87](https://themindplaying.web.app/category-farming87.html)
- [BULLET SUPERHERO](https://iskillplay.web.app/bullet-superhero.html)
- [CATEGORY ALIEN34](https://themindplaying.web.app/category-alien34.html)
- [CATEGORY BUBBLE SHOOTER](https://skillplay.github.io/category-bubble-shooter.html)
- [CATEGORY WATER39](https://iskillplay.web.app/category-water39.html)
- [OTU](https://iskillplay.web.app/otu.html)
- [CLICKER HEROES](https://quizverses.pages.dev/clicker-heroes.html)
- [DRAW WAR](https://themindplaying.web.app/draw-war.html)
- [VALLEY OF WOLVES AMBUSH](https://learnquester.pages.dev/valley-of-wolves-ambush.html)
- [LUXURY HIGHWAY CARS](https://learnquester.pages.dev/luxury-highway-cars.html)
- [MERGE SHOOTER](https://themindplaying.web.app/merge-shooter.html)
- [STEALTH MASTER SNEAK CAT](https://themindplay.pages.dev/stealth-master-sneak-cat.html)
- [BALL CRAZE SORT](https://themindplay.pages.dev/ball-craze-sort.html)
- [CATEGORY FASHION105](https://themindplaying.web.app/category-fashion105.html)
- [CATEGORY POOL 2](https://learnquesters.pages.dev/category-pool-2.html)
- [CATEGORY CAR 2](https://themindskillplayplay.pages.dev/category-car-2.html)
- [TERMS](https://cryptotify.github.io/terms.html)
- [INDEX18](https://quizverses.pages.dev/index18.html)
- [INDEX28](https://quizverses.github.io/index28.html)
- [ONE LINE DRAWING](https://iskillplay.web.app/one-line-drawing.html)
- [CATEGORY WATER39](https://quizverses.github.io/category-water39.html)
- [COSMOS 404](https://themindplay.github.io/cosmos-404.html)
- [CATEGORY CAR 2](https://quizverses.pages.dev/category-car-2.html)
- [CATEGORY BUILDING179](https://iskillquest.pages.dev/category-building179.html)
- [MERGE CAR DEFENSE](https://learnquester.pages.dev/merge-car-defense.html)
- [SITEMAP](https://brainquests.netlify.app/sitemap.html)
- [EAT AND GROW FISH](https://themindzone.pages.dev/eat-and-grow-fish.html)
- [STAND ON THE RIGHT COLOR ROBBY](https://learnquesters.pages.dev/stand-on-the-right-color-robby.html)
- [REAL FREEKICK 3D](https://themindplay.pages.dev/real-freekick-3d.html)
- [CATEGORY POOL](https://learnquester.pages.dev/category-pool.html)
- [CATEGORY CONTROLLER 2](https://learnquesters.pages.dev/category-controller-2.html)
- [GROW WARSIO](https://themindplays.pages.dev/grow-warsio.html)
- [CATEGORY MERGE](https://learnquester.pages.dev/category-merge.html)
- [CATEGORY POINT AND CLICK124](https://learnquesters.pages.dev/category-point-and-click124.html)
- [SCREW JAM FUN PUZZLE GAME](https://themindzone.pages.dev/screw-jam-fun-puzzle-game.html)
- [SPACE SURVIVAL RAINBOW FRIENDS MONSTER](https://iskillquest.pages.dev/space-survival-rainbow-friends-monster.html)
- [BRAWL STARS SOUND](https://studyplayings.web.app/brawl-stars-sound.html)
- [TERMS](https://cryptotify.pages.dev/terms.html)
- [CATEGORY MERGE 2](https://iskillquest.pages.dev/category-merge-2.html)
- [CATEGORY OBSTACLE299](https://quizverses.pages.dev/category-obstacle299.html)
- [WHEEL OF BINGO](https://learnquesters.pages.dev/wheel-of-bingo.html)
- [ESCAPE ROOM MYSTERY KEY](https://studyplayings.pages.dev/escape-room-mystery-key.html)
- [OBBY VS ZOMBIES](https://theskillquest.pages.dev/obby-vs-zombies.html)
- [CATEGORY SHOOTER 2](https://themindskillplayplay.pages.dev/category-shooter-2.html)
- [EATING SIMULATOR](https://themindplay.pages.dev/eating-simulator.html)
- [JELLY BELLY MAKE THE ELEPHANT](https://quizverses.github.io/jelly-belly-make-the-elephant.html)
- [CUBE DROP PUZZLE](https://themindplaying.web.app/cube-drop-puzzle.html)
- [2048 SORT FACTORY](https://learnquester.pages.dev/2048-sort-factory.html)
- [CATEGORY PLATFORM260](https://thelearnquester.web.app/category-platform260.html)
- [BRILLIANT JEWELS](https://themindzone.pages.dev/brilliant-jewels.html)
