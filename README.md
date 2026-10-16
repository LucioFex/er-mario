<div align="center">

# ER Mario · Combat Tuned

**Super Mario 64's Mario in Elden Ring, with combat that keeps you on the enemy.**

A fork of [deltarooo/er-mario](https://github.com/deltarooo/er-mario) that reworks how Mario's punches, kicks and stomps feel against Elden Ring enemies.

[![License: MIT](https://img.shields.io/badge/license-MIT-c9a227?style=flat-square)](LICENSE)
[![Based on er-mario 0.4.2](https://img.shields.io/badge/based%20on-er--mario%200.4.2-d62828?style=flat-square)](https://github.com/deltarooo/er-mario/releases/tag/v0.4.2)
[![Elden Ring 2.7.1](https://img.shields.io/badge/Elden%20Ring-2.7.1-3a3a3a?style=flat-square)](#requirements)
[![Rust](https://img.shields.io/badge/built%20with-Rust%20%2B%20libsm64-b7410e?style=flat-square)](#building)

<img src="docs/screenshots/boss-closeup.jpg" alt="Mario squaring up to the Draconic Tree Sentinel" width="100%">

</div>

---

## Why this fork

The original mod puts libsm64 (Super Mario 64's real movement and animation) inside Elden Ring.
It already plays really well. The one thing that felt off was combat: SM64's punches were made
for Goombas and Bob-ombs, not for knights on horseback.

In SM64 a punch pushes Mario **back** from what he hits. In Elden Ring that's a penalty, because
backing away mid-combo gives the enemy free hits. Whiffs were common too: a punch thrown a few
degrees to the side of an enemy standing right in front of you just missed.

This fork makes Mario fight like a character in a hack and slash: every hit moves him forward, and
the combo turns him toward the enemy.

<div align="center">
<img src="docs/screenshots/combat.gif" alt="Mario closing in on the Draconic Tree Sentinel, ground pound and stomps" width="80%">
</div>

## What changed

### Combat feel

| | Original 0.4.2 | This fork |
|---|---|---|
| **Punch knockback** | A landed punch pushes Mario back (SM64's `-48` bounce) | Mario keeps his speed and stays in range for the next hit |
| **Combo step** | Only the first punch steps forward | Every hit (punch, punch, kick) steps forward a little further |
| **Auto-aim** | None | Each hit of the combo turns Mario to the nearest hostile within an 80° cone |
| **Jump kick** | From a standstill it goes straight up | A small dash forward, aimed like the combo |
| **Punch / kick reach** | 80 units, 50° cone, punches only | 135 units, 60° cone, punches **and** kicks |
| **Head stomp** | Wide hitbox bounced Mario off heads he missed; walked on top of big beasts | Tighter stomp radius, height capped so large enemies can be stomped properly |

Auto-aim only targets enemies: regular foes, bosses, hostile groups and invaders. Friendly NPCs,
allies, props, and enemies already down or being carried are left alone.

### Damage balance

| Attack | Original | This fork |
|---|---|---|
| Stomp | 50% of enemy HP | **67%**, boss bonus ×1.4 |
| Ground pound | 67%, no boss bonus | **100%** (one-shots regular enemies), boss bonus **×2** |
| Elden Ring hits on Mario | 1 or 2 full wedges | **half a wedge** or a whole one, on foot and on Yoshi |

The ground pound is slow and leaves Mario open, so now it's worth the risk.

### Fixes and quality of life

- **Yoshi kept coming back on his own.** Punching (X) while getting off Yoshi queued the game's
  "use item" whistle, and he was summoned again every few seconds. X now reaches the game only
  while Mario whistles.
- **No more random slides.** Elden Ring's ground is full of small steep triangles that started a
  slide in the middle of a walk or a fight. A slide now needs a slope steeper than ~40° held for a
  few ticks (SM64's rules still apply once Mario is sliding).
- **Audio survives device changes.** Unplugging headphones or switching the default output used to
  leave Mario silent for the rest of the session. The audio thread now reopens the stream.
- **Quieter background sounds.** Lakitu's camera clicks play at 25% and Yoshi's footsteps at 40%.

All gameplay changes live in `src/combat.rs`, `src/lib.rs` and a few small, commented
`er-mario:` patches inside `libsm64/src/decomp/game/` and `libsm64/src/fake_interaction.c`.

## Gallery

<table>
  <tr>
    <td width="50%"><img src="docs/screenshots/fire.jpg" alt="Mario dodging the Tree Sentinel's fire"></td>
    <td width="50%"><img src="docs/screenshots/mounted-boss.jpg" alt="Mario facing the Draconic Tree Sentinel on horseback"></td>
  </tr>
  <tr>
    <td width="50%"><img src="docs/screenshots/yoshi.jpg" alt="Mario riding Yoshi"></td>
    <td width="50%"><img src="docs/screenshots/exploring.jpg" alt="Mario exploring the golden forest"></td>
  </tr>
</table>

## Everything from the original mod

Triple jumps, wall kicks, long jumps, ground pounds, punches and kicks that hurt enemies, SM64's
health meter and coins, Lakitu's camera, Bowser's tail swing on staggered bosses, Bob-omb style
enemy throws, Yoshi in place of Torrent, Mario's own model, and Bowser laughing at you instead of
YOU DIED. See the [original README](https://github.com/deltarooo/er-mario#readme) for the full
feature list, controls and known issues.

## Requirements

- **Elden Ring** on Steam, exe version **2.7.1**. On any other version the mod switches itself off.
- **[me3](https://me3.help)**, the mod loader.
- **Your own Super Mario 64 ROM, US version** (`.z64`, `.n64` or `.v64`). Mario's model, textures,
  sounds and icons are built from it on your PC the first time you play. No Nintendo data ships
  with this repository.
- An Xbox-layout controller, or mouse and keyboard.

> [!WARNING]
> **Offline only.** me3 starts Elden Ring offline with Easy Anti-Cheat off, and the mod uses its
> own save file (`ER0000_mario.sl2`). Never play it online and never load the Mario save in the
> normal game.

## Install

1. Install the original mod by following its
   [Getting started](https://github.com/deltarooo/er-mario#getting-started) guide (me3, the
   release zip, your ROM, the first launch setup).
2. [Build](#building) this fork and copy `er_mario.dll` over the one in your **ER-Mario** folder
   (`.\build.ps1 -Dist <ER-Mario folder>` does it for you).
3. Launch with **er-mario.me3** as usual.

The mod still checks the original repository for new releases. Updating to an upstream release
replaces this fork's DLL with the original one.

## Controls

| Controller | Keyboard and mouse | Action |
|---|---|---|
| Left stick | W A S D | Move |
| A | L / right mouse | Jump (double, triple, wall kick, long jump) |
| X or B | , / left mouse | Punch, kick, dive (auto-aimed combo) |
| LB or LT | K | Crouch, ground pound in the air |
| Right stick | Arrow keys | Lakitu's camera |
| Y | E | Interact |
| RB or RT | R | Whistle for Yoshi |
| | F9 | Switch between Lakitu's and Elden Ring's camera |
| | F1 (hold 3 s) | Give up and respawn at the last grace |

## Building

Windows, with [Rust](https://rustup.rs), Visual Studio Build Tools (C++),
[LLVM](https://github.com/llvm/llvm-project/releases) (clang-cl compiles libsm64's C code) and
Python. Cargo fetches fromsoftware-rs itself (pinned to a commit in `Cargo.toml`).

```powershell
git clone https://github.com/LucioFex/er-mario
cd er-mario/libsm64
python import-mario-geo.py
cd ..
.\build.ps1
```

On Linux, install [`cargo-xwin`](https://github.com/rust-cross/cargo-xwin) plus `python3`,
`clang` and `llvm`, then use `build.sh` instead of `build.ps1`.

`import-mario-geo.py` downloads Mario's model code from the SM64 decompilation once and strips
his vertices, light colours and textures out of it: those are read from the player's ROM at
runtime. The model code isn't part of this repository.

## Credits

- **[Delta (deltarooo)](https://github.com/deltarooo)**, author of [ER Mario](https://github.com/deltarooo/er-mario). Almost everything here is their work.
- [libsm64](https://github.com/libsm64/libsm64) (CC0) by the libsm64 contributors, built on the
  [Super Mario 64 decompilation](https://github.com/n64decomp/sm64) by the n64decomp team
- [fromsoftware-rs](https://github.com/vswarte/fromsoftware-rs) (MIT / Apache-2.0) by Vincent
  Swarte and contributors
- [me3](https://me3.help) by the me3 team
- [hudhook](https://github.com/veeenu/hudhook) (MIT) by veeenu
- [BenjaminMassey](https://github.com/BenjaminMassey): controller direction fix and the Linux build script (upstream)
- The Grand Archives' Elden Ring cheat table, for the item and inventory function patterns
- Combat tuning in this fork: [Luciano Esteban (LucioFex)](https://github.com/LucioFex)

## License

[MIT](LICENSE), same as the original. It covers the mod's own code only, not Super Mario 64's or
Elden Ring's content: Mario's model, textures, sounds and animations come from the player's ROM,
and nothing of FromSoftware's is included. The `libsm64` folder keeps libsm64's own license, CC0
([libsm64/LICENSE.md](libsm64/LICENSE.md)).

<sub>Super Mario 64 and Mario are Nintendo's. Elden Ring is FromSoftware's. This is a free fan mod,
not affiliated with either.</sub>
