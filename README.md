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
| **Auto-aim** | None | Each hit of the combo turns Mario to the nearest hostile around him (it keeps to the same one during a string), and a blow lunges a step at an enemy just out of reach (fists and hammer, on the ground) |
| **Jump kick** | From a standstill it goes straight up | A small dash forward, aimed like the combo |
| **Punch / kick reach** | 80 units, 50° cone, punches only | 135 units, 60° cone, punches **and** kicks |
| **Head stomp** | Wide hitbox bounced Mario off heads he missed; walked on top of big beasts | Tighter stomp radius, height capped so large enemies can be stomped properly |

Auto-aim only targets enemies: regular foes, bosses, hostile groups and invaders. Friendly NPCs,
allies, props, and enemies already down or being carried are left alone.

### New moves

On top of SM64's own moves, Mario gets a hack and slash move set (new libsm64 actions in
`libsm64/src/decomp/game/er_moves.c`, sequences in `src/moves.rs`):

| Input | Move |
|---|---|
| X, X, X, then X again | Punch, punch, kick, then a longer string: spinning backfist, double kick, whirlwind kick |
| X, X, (beat), X | Rush: two fast jabs and a flying kick |
| X, X, Y | Super jump punch. Small or stunned enemies go up with Mario for an air combo (X in the air) |
| Y in the air | Meteor dive |
| Y, X | Mario Tornado |
| B, Y | Koopa shell slide |
| B | Dash, invulnerable, where the stick points. Dashed just as a nearby enemy attacks: a **perfect dodge** (see below) |
| RB (hold) | Fireball, charged 1 / 2 / 4 s; held 10 s, a giant one; held 20 s, the sun (see below). A blow on Mario puts the charge out. Shots home in gently on the nearest enemy ahead. Thrown with a palm thrust (recoil on the charged ones), in the air it holds Mario up for a moment, X right after goes on with melee |
| A during an air combo | Enemy step: Mario kicks off the enemy for another jump (3 per air combo) |
| B between air blows | With an enemy in the air, the air dash comes back after each blow (3 per jump) |
| D-pad left / up | Metal and Wing caps (they recharge in about 55-60 s) |
| D-pad right | **Mega Mushroom** (see below) |

**Weapons (LB).** A tap of LB takes the next weapon: fists, hammer, Chain Chomp, boomerang. Holding
LB goes back to the fists. A banner shows the weapon's name for a moment.

**Mixing weapons (swap strikes).** Tap LB in the middle of a string, or right after a blow, and
Mario switches on the spot, opening with the new weapon's swap strike while the string goes on
(the next X is the new weapon's next blow), Devil May Cry and God of War style: the hammer comes in
with its overhead smash, the Chain Chomp with its wide sweep, the boomerangs with a fan of three,
the fists with the rush. In the air it's the weapon's air blow (two per jump, four with an enemy up
there). Each swap scores style points and flashes a golden ring.

**Perfect dodge.** Dash (B) just as an enemy close by starts an attack and the world slows down for
a second, Bayonetta's Witch Time: a blue flash, a "waha!", and for a moment Mario's blows are
counters that hit 1.6x as hard (`src/dodge.rs`). Once every few seconds at most.

**Hammer.** Mario pulls out a big dark hammer and his X and Y moves turn into hammer
moves: a five-swing string, an earthquake slam (Y), a plunge from the air (Y in the air: the higher
he comes down from, the more the slam charges, and from 12 m up it lands as the giant hammer), a
Donkey Kong flurry that ends with a giant hammer (X, X, Y), a Bowser spin (Y, X) and a
launcher (X, Y): a golf swing that sends enemies flying high, where they hang for a moment so
Mario can jump after them and keep the combo going in the air. When he's
not swinging it, the hammer hangs across his back and he grabs it for each attack.

Holding Y with the hammer charges the slam: Mario squats and trembles with the hammer raised (he can
still turn), it grows with a soft chime per level, and fully charged it comes down as the giant
hammer. Enemies that fall out of an air combo bounce off the ground once, and a hit on the bounce
sends them back up. Launched enemies leave a streak and land with a thud of dust.

**Squash and stretch.** The weapons move like a cartoon's: the hammer stretches out and smears
along its swing and squashes flat on a heavy blow, the Chain Chomp stretches long as it flies and
squashes when it slams into something, its chain's links pull long and thin when taut, and the
boomerangs pop out of Mario's hand. Between throws the big boomerang hangs across Mario's back,
like the hammer, and he grabs it for each throw.

**Real 3D weapons.** The hammer, the boomerangs and the Chain Chomp are 3D models built into
Mario's own mesh, each on a bone of its own (`src/assets/gear.rs`): the boomerangs spin flat and
bank into their curve, the Chomp is a dark iron ball with eyes and teeth whose jaw opens to bite,
on a chain of real links that sags when slack and pulls taut in a lash. Their sounds vary and stay
soft when repeated: the Chomp's chain rattles with SM64's metal clinks, its bark and whooshes come
louder or softer each time, and the boomerangs whir one at a time.

**Boomerang** (the Boomerang Bros.'), the ranged weapon (`src/boomerang.rs`):

| Input | Move |
|---|---|
| X | Throw one, from one hand then the other (it flies up to 18 m). It curves out, hits on the way out and again on the way back. Up to three in the air |
| Y | Three at once in a fan |
| Y (hold) | The big boomerang: Mario whirls round on the spot with it held out (like swinging Bowser by the tail), faster and faster as it grows in his hand, then lets fly. It flies far (up to 36 m) homing in on the enemy, hangs on it for 3 s spinning (following it as it moves, or the next one when it falls), hitting everyone around it again and again |
| X, X, Y | Boomerang launcher: thrown up in an arc, it sends enemies flying up |
| X / Y in the air | Throws in the air; the first few hold Mario up, Devil May Cry style |

Boomerangs and fireballs keep a launched enemy floating where it is, so you can launch one with
the hammer or the boomerang and keep it up from a distance.

**Chain Chomp**, the crowd weapon (`src/chomp.rs`): wide reach, low damage, many enemies at once. It
hops along beside Mario between attacks. Its swings leave a ribbon of fire behind it and throw
sparks off the chain (Kratos' Blades of Chaos), and with the Chomp in hand the hook (RT) is the
Chomp itself: it flies out on its chain, jaws open.

| Input | Move |
|---|---|
| X, X, X | Lunge straight ahead, wide sweep in front, crash down far ahead (shockwave) |
| X, Y | Bite: it lunges low, bites and tosses enemies up for an air combo |
| Y | Cyclone: Mario swings it around himself (steer with the stick), then lets it fly ahead |
| X in the air | It swings once all the way around him |
| Y in the air | Crash: Mario drops with it under him, shockwave on landing |

**Mega Mushroom (D-pad right).** Mario grows to 4 times his height for 15 seconds (90 s
recharge), in SM64 power-up steps with the power-up music. His whole SM64 world is scaled, so
collisions grow with him: low ceilings and narrow passages stop him, and if there is no room
where he stands (a ceiling, walls close by) he won't grow and Lakitu buzzes. Big, he's a titan:
all of him moves at 70% pace (walk, blows, jumps; the music keeps its own), he walks slowly for
his size, fists only, every hit does 2.5x and shakes the screen, enemy hits hurt about a third as
much, his steps crush whoever is under his feet and his landings send a quake through everyone
around (5.5 m), his jumps go up a giant's way, and Y is a
two-fisted slam with a huge shockwave (hold Y to charge it; one every 2.5 s). No dashes, shell,
meteor dive or super jump punch (too fast for a giant), no caps, no Yoshi and no first-person
view while he's big. D-pad right again ends it early, and so do a door, a fog wall, a ladder, a
loading screen or dying. It replaced the Vanish cap.

**The hook (RT).** Mario flings his cap out on a line of light, Devil May Cry's Snatch. It aims
itself: any enemy close to Mario (within 9 m, all around him, the nearest first and those in front
before those behind), farther ones where Mario or the camera looks; with none, the spot of the map
the camera looks at (up to 20 m):

| It catches | What happens |
|---|---|
| A small enemy | Dragged in to Mario; in the air it hangs in front of him, ready for the air string |
| A big enemy (a boss, a strong one) | Mario flies to it and pops up in front of it for an air combo |
| A wall | Mario flies to it and clings to it a moment: A wall kicks off it, LT lets go |
| A floor or a ledge | Mario flies there and lands |

Walls and ledges are good for **one hook per jump**: after that it only clinks off them until
Mario lands, so there's no climbing forever. Enemies can be hooked any number of times, and each
one gives the wall hook back (jump, wall, enemy, wall again works). It barely hurts: it's for
keeping a combo going and for getting around. It works on the ground and in the air
(`src/hook.rs`).

**Yoshi in a fight (`src/ride.rs`).** Riding Yoshi, A still jumps and B dashes, and Yoshi has his
own moves (his mouth opens for the tongue, he nods with each egg and on a stomp):

| Input on Yoshi | Move |
|---|---|
| X | Tongue: lashed out ahead, it hits whoever it reaches, and every enemy it licks makes him an egg |
| Y (hold for more) | Throw an egg at the nearest enemy ahead: it arcs to it and bounces once off the ground. He has up to six (shown under the caps), a new one every 3 s and one per lick |
| RB (hold for more) | Mario's fireball from the saddle, a palm thrust straight at the nearest enemy ahead |
| LT in the air | Stomp: a shockwave all around where he lands |
| LB | Burst of speed for 2.5 s, dust behind him (fast enough to run enemies over), then 6 s of rest |

**Style meter.** Devil May Cry's: every hit in a row scores style points (finishers more, and a
hit of a different kind than the last one a bonus, so varied strings climb faster), and the string
earns a rank: D DANDY, C COOL, B BONKERS, A AWESOME, S SUPER, SS SUPER STAR, SSS WAHOO. A new rank
slams in with its word, a soft red coin chime a note higher each time and Mario cheering from S up;
a bar shows the time left before the string drops. Getting hurt ends it ("MAMMA MIA"). From rank
S, a combo's finisher is a **Star Finish**: a golden flash, stars and coins flying out, a banner
and a longer slow motion. A finisher that takes an enemy out, or any kill from rank B up, slows the
world down for a moment (`src/style.rs`).

Hits come with hit-stop, star bursts, shockwave rings, a swing trail and a camera shake on the
heavy ones (`src/impact.rs`). Fireballs are lit balls of fire with flames licking all around them
(drawn back to front, so they turn with the camera), a cooling tail and embers, hidden behind walls.

**The camera indoors.** Lakitu senses the room around Mario (walls all around, the ceiling): in a
corridor or a small room it comes closer and lower and stays under the ceiling, a wall right behind
it makes it slide round to where it can see instead of zooming into Mario's back, and it keeps off
walls by the lens. Out in the open it looks a little ahead of where Mario runs and backs off a bit
at full speed, and on a steady run it swings round behind him on its own (God of War style, never
when he runs towards it or right after you turn it yourself). It keeps real time in slow motion, so
it never lags behind Mario. Something in the way (a column, a doorframe) makes it turn a little to
see past it before it dives in close, and it never comes closer than a metre to Mario. Next to a big enemy Lakitu's camera backs off and looks up at it,
more the bigger and closer it is; if one is close but out of view, the camera drifts round to
show it (not right after you turn the camera yourself). In a fight the camera works like God of
War's or Devil May Cry's: it leans towards the enemy you're fighting so both share the screen (with a group around Mario,
towards the group's middle, backing off a little for each enemy),
turns gently to keep it in view, and in an air combo it rises with Mario, backs off a little and
levels out to look up at the fight. A big enemy that gets between the camera
and Mario (or around the camera) turns see-through until it's out of the way.

**Enemy grabs.** Elden Ring's grabs work on Mario: a troll's fist, a beast's jaws, a boss lifting
him up. The game holds him and carries him through its throw while Mario dangles in its grip,
kicking and groaning (SM64's Chuckya grab); one that holds him inside its body swallows him whole,
Bubba style, and he's gone until it spits him out. Let go, he's thrown off with a cry. The throw's
damage counts in wedges (`src/grab.rs`). When the enemy's animation lifts or swings the Tarnished's
body, Mario's pose follows it (`src/engine_mario.rs`), so he's really up in the troll's fist.

**Ailments and fire.** Elden Ring's ailments build up on Mario as on any Tarnished, the game does
their damage (in wedges), and Mario shows them the SM64 way (`src/status.rs`): poison and scarlet
rot make him cough (Hazy Maze's gas) in green or orange puffs, blood loss bursts red (that wedge
costs no knockback), frostbite freezes him stiff for a second and leaves him shivering, sleep
knocks him out (mash buttons to wake up sooner), madness shocks him, and a fire on him sends him
running round in flames like SM64's (the burn's damage is Elden Ring's, not SM64's). Deathblight
stays off.

**Giant fireball.** Held 10 s, RB throws a ball of fire that works like a kamehameha: regular
enemies it runs into are swept along inside it, burning a little at a time; a boss or a big body
stops it and it clings to him, burning, for a second and a half; then it blows up, hurting
everyone within 5 m, and the enemies it carried fly off. It also blows up against a wall or at the
end of its range.

**The sun.** Held on for 20 s (and not hit once: any blow on Mario, even a push that costs no
health, puts out whatever he's charging), the giant one grows into an immense ball held high over
his head. It flies slowly and works the same way, clings longer, and its explosion reaches 9 m:
waves of fire roll out, the regular enemies near its heart are flung away, and pools of lava are
left on the ground for 2-3 s, burning whoever steps in them.

**Shoves.** An enemy's blow that costs no health (a push, a shield bash) now makes Mario reel back
too (SM64's soft knockback, no wedge lost).

**Big blows hurt more than wedges.** A heavy hit from a boss or a big brute sometimes (a roll of
the dice, never twice in a row) sends Mario flying far off, screaming; if a wall is in the way he
smacks into it with stars and a bonk (`src/blown.rs`). And a fall from very high up (over 22 m)
is the end of him, as in Elden Ring.

### Damage balance

Every hit takes a share of the enemy's max HP. Bosses take a small fraction of that, more for the
close and risky hits.

| Attack | Original | This fork |
|---|---|---|
| Punches, kicks, combos, hammer (X and Y) | 67% of enemy HP | **1-23%** per hit. A regular enemy takes about 10 punches; finishers hit hardest, the super jump punch's rising blows barely scratch |
| Boomerang | — | **2-4%** per pass (it hits going and coming back); the big one about 2% per hit while it hovers |
| Counters (after a perfect dodge) | — | **x1.6** for 1.6 s |
| Chain Chomp | — | **2-10%**, on every enemy it reaches; the cyclone the least |
| Long strings | — | Past 10 hits on the same enemy each hit is worth 5% less than the one before (down to 40%, finishers 70%), so long combos last instead of melting the enemy |
| Stomp | 50% of enemy HP | **20%**; bouncing on the same enemy without landing halves it each time (100%, 50%, 25%... down to 10%) |
| Ground pound | 67%, no boss bonus | **31%**, boss bonus **×2**, shockwave hits enemies around |
| Fireball | — | **9-25%** by charge; the giant one burns **6%** every quarter second while it carries or clings, and its explosion does **64%** to everyone within 5 m; the sun's explosion does **75%** within 9 m and its lava pools **4%** every half second (on bosses about 8% and 0.3%) |
| Elden Ring hits on Mario | 1 or 2 full wedges | **half a wedge** or a whole one, on foot and on Yoshi |

The ground pound is slow and leaves Mario open, so it's worth the risk.

### Fixes and quality of life

- Elden Ring only lets the player create about 256 bullets a session (their emitters are never
  freed), and every blow of Mario's that the game registers, and his footstep noise, is one. Once
  they ran out, blows no longer woke or staggered enemies and Mario made no noise. They're now
  rationed: a light blow registers on the same enemy at most every second (heavy blows and
  finishers always), and the noise only plays with an enemy within 20 m that Mario isn't already
  fighting. Known limit: a very long session can still run out.

- **Enemies notice Mario like a normal player.** The game never walks the Tarnished in Mario
  mode, so enemies never heard his steps and only fought back once hit. Now Mario makes the
  game's own AI noise (`src/noise.rs`): soft when he walks, louder when he runs, jumps, lands
  or fights, loudest on heavy blows. Standing still or crawling is silent, so sneaking still works.

- **Yoshi kept coming back on his own.** Punching (X) while getting off Yoshi queued the game's
  "use item" whistle, and he was summoned again every few seconds. X now reaches the game only
  while Mario whistles.
- **No more random slides.** Elden Ring's ground is full of small steep triangles that started a
  slide in the middle of a walk or a fight. A slide now needs a slope steeper than ~40° held for a
  few ticks (SM64's rules still apply once Mario is sliding).
- **Audio survives device changes.** Unplugging headphones or switching the default output used to
  leave Mario silent for the rest of the session. The audio thread now reopens the stream.
- **Quieter background sounds.** Lakitu's camera clicks play at 25% and Yoshi's footsteps at 40%;
  the style meter's and the charges' chimes play softly.
- **Fewer hitches in fights.** The game's raycast check of each collision triangle is remembered,
  so fast combat moves no longer re-check hundreds of triangles every few frames.

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
| X | , / left mouse | Punch, punch, kick (auto-aimed combo, see [New moves](#new-moves)). Running: dive (into an enemy's back or side it picks the enemy up) |
| Y | | Special move: breakdance sweep, slide kick (running), combos with X and B |
| B | | Dash: a quick lunge where the stick points (ahead without it), invulnerable; X or A out of it |
| RB (hold) | | Fireball, charged 1 / 2 / 4 s; held 10 s, a giant one that sweeps enemies along and blows up; held 20 s, the sun, which leaves lava pools |
| D-pad | | Caps: left Metal, up Wing, down take it off (and interact); right Mega Mushroom |
| RT | Middle mouse button | The hook (see above) |
| LB | | Next weapon (fists, hammer, Chain Chomp, boomerang); hold to go back to the fists. Mid-string: a swap strike |
| LT | K | Crouch, ground pound in the air |
| Right stick | Arrow keys | Lakitu's camera: left / right turn it smoothly round Mario (faster the further you push), down zooms out, up zooms in (and into the first-person view). In a fight the camera backs off to keep Mario and the enemy in view, more the farther you've zoomed out |
| D-pad down | E | Interact: talk, pick up, graces, doors (also takes the cap off) |
| L3 (left stick click) | R | Whistle for Yoshi |
| X / Y / RB / LT / LB on Yoshi | X: , or left mouse; Y: middle mouse; LT: K | Tongue, eggs, Mario's fireball, stomp, burst of speed |
| | F9 | Switch between Lakitu's and Elden Ring's camera |
| | F1 (hold 3 s) | Give up and respawn at the last grace |

## Luigi and co-op (experimental)

`character = luigi` in `er_mario.ini` builds the model as Luigi, in green and navy (the next two
launches rebuild it). `docs/coop/er-mario-coop.me3` is a me3 profile that loads
[Seamless Co-op](https://www.nexusmods.com/eldenring/mods/510) (its `SeamlessCoop` folder next to
the profile) together with ER Mario. This is groundwork, **not real co-op yet**: each player sees
themselves as Mario or Luigi, but the other player isn't animated as Mario on your screen, and as
the guest Mario's blows don't stick (the host's game owns the enemies). Syncing Mario's pose and
the guest's blows over the network is the next step.

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
