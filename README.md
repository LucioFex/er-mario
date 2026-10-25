<div align="center">

# ER Mario · Combat Tuned

**Super Mario 64's Mario in Elden Ring, with combat that keeps you on the enemy.**

A fork of [deltarooo/er-mario](https://github.com/deltarooo/er-mario) that reworks how Mario's punches, kicks and stomps feel against Elden Ring enemies.

[![License: MIT](https://img.shields.io/badge/license-MIT-c9a227?style=flat-square)](LICENSE)
[![Based on er-mario 0.4.2](https://img.shields.io/badge/based%20on-er--mario%200.4.2-d62828?style=flat-square)](https://github.com/deltarooo/er-mario/releases/tag/v0.4.2)
[![Elden Ring 2.7.2](https://img.shields.io/badge/Elden%20Ring-2.7.2-3a3a3a?style=flat-square)](#requirements)
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
| **Head stomp** | Wide hitbox bounced Mario off heads he missed; walked on top of big beasts | Tighter stomp radius, height capped so large enemies can be stomped properly; a stomped enemy dents and springs back (a ground pound flattens a regular one a moment, a big one only dents) |

Auto-aim only targets enemies: regular foes, bosses, hostile groups and invaders. Friendly NPCs,
allies, props, and enemies already down or being carried are left alone.

### New moves

On top of SM64's own moves, Mario gets a hack and slash move set (new libsm64 actions in
`libsm64/src/decomp/game/er_moves.c`, sequences in `src/moves.rs`):

| Input | Move |
|---|---|
| X, X, X, then X again | Punch, punch, kick, then a longer string: spinning backfist, double kick, whirlwind kick. Running flat out, X is the string's first punch (no longer SM64's dive with its hop) |
| X, X, (beat), X | Rush: two fast jabs and a flying kick |
| X, X, Y | Super jump punch. Small or stunned enemies go up with Mario for an air combo (X in the air) |
| Y in the air | Meteor dive |
| Y, X | Mario Tornado |
| B, Y | Koopa shell slide |
| B | Dash, invulnerable, where the stick points. Dashed just as a nearby enemy attacks: a **perfect dodge** (see below). With the weapons that have their own B: [Star Sword](#star-sword) Star Step, hammer guard / roll, boomerang glide in the air, Chain Chomp slam (see each weapon) |
| back, forward + X (fists) | **Bullet Bill punch**: a coil back, then Mario flies fist first at the enemy, up to 7 m (the stick is read against the enemy, not against where he faces). Landed, it can be cut at once into the dash, a jump, the string (X goes on at the second punch) or a swap |
| X held through a dash | **Dodge Offset** (Bayonetta's): keep X down while you dash out of a string, and the next X goes on with the string's next blow instead of starting over. Every weapon: the fists' punches, the hammer's three swings, the Chomp's lash / sweep / slam, the sword's four cuts, the boomerang's throws |
| mash X as the Bullet Bill punch lands | **Hundred-hand flurry**: ten jabs in a second, then an uppercut off a little hop that launches small and medium enemies |
| X held (any weapon but the boomerang) | **A charge while he walks** (`src/charge.rs`): from a quarter of a second held, the blow X began gives way and Mario keeps walking (slower) with the weapon gathering power: sparks of its element in his hands, a chime at 0.47 s (level 1) and at 0.9 s (full). A jump, the dash, Y or a blow on him calls it off. Let go: each weapon's own quick blow that closes in (below) |
| X held, let go (fists) | **Bullet straight**: the charged fist driven out as he shoots in behind it, 3 m (6 m full: a flaming straight that flings a regular enemy, it lands alive). Into a wall right behind them: **WALL!** (more damage and stance) |
| X held, let go (hammer) | **Hammer leap**: a real jump up and ahead, 3 m (6 m full), the hammer swung up over his head and growing to twice its size (three times full), brought down where he lands in a quake |
| X held, let go (Chain Chomp) | **Chomp ram**: the Chomp flung ahead biting all the way, Mario dragged behind it 4 m (8 m full, and a crunching last bite) through everyone in the line |
| X held, let go (Star Sword) | **Drive** (see [Star Sword](#star-sword)) |
| X, (beat), X (fists) | **Heavy hook**: a beat after a lone first punch, a wide hook with his whole body turned into it |
| forward + X in the air string (fists) | **Meteor punch**: driven down and ahead; an enemy up in the air is spiked into the ground and bounces once (hit it again to send it back up). Jumping in at an enemy, forward + X is still the air string's first kick |
| back + X in the air (fists) | **Dropkick**: both feet into it, it flies off (into a wall: WALL!) |
| back, forward + X (hammer) | **Hammer charge**: the hammer hauled back over his shoulder, a shoulder charge up to 6 m into the first enemy, then the overhead slam where he stops (committed: no way out until it lands) |
| X, (beat), X (hammer) | **Hammer Bros. volley**: three small hammers thrown overhand in arcs, each onto the next enemy in front (one lands ahead with no one there) |
| forward + X in the air (hammer) | **Diving comet**: a moment's hang with the hammer up, an angled dive at the enemy, a ring shaking the ground where he lands |
| back, forward + X (Chain Chomp) | **Chomp rush**: the Chomp flung ahead, Mario dragged after it up to 9 m, its jaws on the first enemy in the way |
| mash X as the bite lands (Chain Chomp) | **Frenzy**: five quick bites and a last one tossing it up |
| back, forward + X (boomerang) | **Skim**: thrown low and straight, Mario dashing after it up to the enemy; it comes back to his hand there |
| X, (beat), X (boomerang) | **Pincer**: two at once curving out opposite ways and closing on the enemy from both sides (two in hand) |
| X held (boomerang) | **Steered boomerang**: steered with the stick for up to 2 s (each enemy once); let X go and it comes back |
| RB (hold) | Fireball, charged 1 / 2 / 4 s; held 10 s, a giant one; held 20 s, the sun (see below). With another weapon, its element's own move instead, each with a 10 s giant (see [Elements](#elements)). A blow on Mario puts the charge out, and so does any other attack (X, Y, B, RT, L1) pressed while it charges: RB has to come up again. Shots home in gently on the nearest enemy ahead. Thrown with a palm thrust (recoil on the charged ones), in the air it holds Mario up for a moment, X right after goes on with melee |
| A during an air combo | Enemy step: Mario kicks off the enemy for another jump (3 per air combo) |
| B between air blows | With an enemy in the air, the air dash comes back after each blow (3 per jump) |
| D-pad up | Wing cap, on a tap: it comes on as the button is let go (recharges in about 55 s). Held half a second: the [portable cannon](#portable-cannon) |
| D-pad right | **Mega Mushroom** (see below) |

**Weapons (LB).** A tap of LB takes the next weapon: fists, [Star Sword](#star-sword), hammer,
Chain Chomp, boomerang. Holding LB goes back to the fists. A banner shows the weapon's name for a
moment.

**Mixing weapons (swap strikes).** Tap LB in the middle of a string, or right after a blow, and
Mario switches on the spot, opening with the new weapon's swap strike while the string goes on
(the next X is the new weapon's next blow), Devil May Cry and God of War style: the hammer comes in
with its overhead smash, the Chain Chomp with its wide sweep, the boomerangs with a fan of three,
the fists with the rush. In the air it's the weapon's air blow (two per jump, four with an enemy up
there). Each swap scores style points and flashes a golden ring.

**Perfect dodge.** Dash (B) just as an enemy close by starts an attack and the world slows down for
a second, Bayonetta's Witch Time: a ring and a glint on Mario (nothing over the screen), a "waha!", and for a moment Mario's blows are
counters that hit 1.6x as hard (`src/dodge.rs`). Once every few seconds at most.

**Hammer.** The heavy weapon: slow, committed, and the one that takes the most health per blow.
Donkey Kong's hammer, nearly as tall as Mario (on his back too). Every blow has a wind-up that
can't be cut short (no dash, no jump until it has landed: a miss is punished), stops the world a
moment when it lands (hit-stop), shakes the camera softly and throws up dust and 3D rubble; Mario's
body squashes into the wind-up, stretches into the blow and sinks under the follow-through. Its
heavy blows break strong enemies' stance and knock regular ones down. A swing that stays on a big
enemy (a boss's body fills its whole reach) lands on him again, each time for half the one before
(100%, 50%, 25%...); the next blow is whole again.

| Input | Move | Per hit (regular enemy) |
|---|---|---|
| X | Hammer Bro's chop, down across from over his shoulder | 12% |
| X, X | King K. Rool's backhand, a wide sweep through everyone in front | 13% |
| X, X, X | Donkey Kong's smash: a little hop and the hammer, grown with the swing, buried in the ground; the ground shakes 3 m around, regular enemies go down | 19% |
| Y (hold to charge) | Bowser's quake: tapped 15%; a level up 21% and everyone around goes down; fully charged (1.2 s) the titan's blow, the hammer giant, 30% in 6 m | 15-30% |
| X, X, Y | Donkey Kong's drum: three pounds, each shaking wider, then a giant hammer | 3 x 7% + 23% |
| Y, X | Bowser spin: one blow all round each turn, the release flings them | 5% a turn + 13% |
| back + X (or X, Y) | Launcher (golf swing): they fly high for an air combo | 12% |
| forward + Y | Hammer Bro's jump: a leap onto them into the plunge (lands a level up) | 21% |
| X in the air | Somersault swing | 9% |
| Y in the air | Plunge: the higher he came down from, the more the slam charges (12 m: the titan's blow) | 15-30% |
| X + A | Hammer upswing, up into the air with them | 11.5% |
| B (stick still, held) | **Guard** (`src/guard.rs`): the hammer's head held up in front of him, he walks at 35% while it is up. Blows from the front 120° cost 75% less; the third blocked blow breaks the guard (1 s stunned, no guard for 2 s). No guard against a colossus's blows or from behind | |
| B just before a blow lands | **Just Guard** (5 ticks): no damage, the attacker recoils (stance, a clang) and the next X within 1.6 s is a counter, x1.6 | 2% (the clang) |
| B + stick pushed | **Heavy roll**: the dash at 0.6 of its speed, untouchable | |

(Shares at the 0.6 damage factor: vanilla deals 5% less of them, Reforged 30% less. Bosses take a few % of their bar at most
from the heaviest.) Y pressed during a swing's wind-up waits for it to land.

Holding Y with the hammer charges the slam: Mario squats and trembles with the hammer raised (he can
still turn), it grows with a soft chime per level, and fully charged it comes down as the titan's
blow. Enemies that fall out of an air combo bounce off the ground once, and a hit on the bounce
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
| X | Throw one, from one hand then the other (it flies up to 22 m). It curves out, hits on the way out and again on the way back. Up to three in the air |
| Y | Three at once in a fan |
| Y (hold) | The big boomerang: Mario whirls round on the spot with it held out (like swinging Bowser by the tail), faster and faster as it grows in his hand, then lets fly. It flies far (up to 43 m) homing in on the enemy, hangs on it for 3 s spinning (following it as it moves, or the next one when it falls), hitting everyone around it again and again |
| X, X, Y | Boomerang launcher: thrown up in an arc, it sends enemies flying up |
| X / Y in the air | Throws in the air; the first few hold Mario up, Devil May Cry style |
| B on the ground | The dash |
| B in the air | **Glide** (`src/glide.rs`): Mario holds the boomerang overhead and it spins like a propeller (he doesn't); he falls at 2.5 m/s at most and sails on at 8 m/s, steered by the stick (gently), up to 3 s, never gaining height. Once an airtime (landing, an enemy step or the hook give it back; no cooldown). X throws a boomerang from the air and ends it, L2 ground pounds, A lets go. Used up: the air dash |

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
| B | **Chomp Slam** (`src/chompslam.rs`, God of War's harpy swing): the Chomp bites the enemy he fights (or the nearest in front, within 5 m) and Mario heaves it over his head and slams it down on the other side, onto the ground and whoever stands there (8% to it, 6% to each it lands beside, a blast of 2.5 m); it bounces off the ground, ready for X or the launcher. B again within a beat slams it back the other way: 3 slams a string. Only small enemies, or stunned medium ones; a big one (or a boss) is only yanked: the Chomp bites, it lurches toward him a step, its stance cracks. No one to bite: the dash |
| B in the air | **Harpoon** (Orion's): the Chomp bites an enemy that is up there (juggled or off the ground) and he drives it down, 8%; nothing up there: the air dash |

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
itself: any enemy close to Mario (within 12 m of its body, all around him; one within 5 m first,
then the nearest, those in front before those behind), farther ones where Mario or the camera
looks; one counts as in sight if his hand or his chest sees its middle or high up on it (a bump in
the ground no longer hides an enemy right there); with none, the spot of the map the camera looks
at (up to 20 m):

| It catches | What happens |
|---|---|
| A small enemy | Mario flies to it and pops up in front of it for an air combo |
| A medium enemy (up to 1.4 m wide and 3.4 m tall) | Tapped: the **zip-kick**, he flies to it and kicks off it, bouncing up into the air for the air string. Held (0.2 s from the throw; the cap waits on it a moment if it got there sooner): the **vault**, a flip over its head landing behind it, facing it (BEHIND!: his blows on it count ×1.3 for a second) |
| A big enemy (a boss, a strong or huge one) | The **Attack on Titan hook**: the cap anchors to the bone nearest the middle of the screen (see [below](#attack-on-titan-hook)) |
| A wall | Mario flies to it and clings to it a moment: A wall kicks off it, LT lets go |
| A floor or a ledge | Mario flies there and lands |
| Nothing (the sky), RT held | A spot for **the rope** ahead and above him in a cone (a branch, a ledge, a ceiling 6-25 m off, best about 50° up); tapped, the cap only comes back |
| **LT held** (any time, ground or air) + RT: a small enemy | Dragged in to Mario; in the air it hangs in front of him, ready for the air string |
| LT + RT: a medium enemy (a stronger or wider regular one, up to 1.4 m wide and 3.4 m tall) | Dragged in too, but the same one only every 5 s, and in the air it drops in front of him (no endless juggle) |
| LT + RT: a big enemy (a boss, a strong one) | Only a tiny hit: it doesn't come and Mario doesn't go |

Walls and ledges are good for **one hook per jump**: after that it only clinks off them until
Mario lands, so there's no climbing forever. Enemies can be hooked any number of times, and each
one gives the wall hook back (jump, wall, enemy, wall again works). It barely hurts: it's for
keeping a combo going and for getting around. It works on the ground and in the air
(`src/hook.rs`).

**Yoshi in a fight (`src/ride.rs`).** Riding Yoshi, A still jumps and B dashes, and Yoshi has his
own moves (his mouth opens for the tongue, he nods with each egg and on a stomp):

| Input on Yoshi | Move |
|---|---|
| X | Tongue: a 3D tube out to 11 m, it locks on to the nearest enemy in a wide cone ahead and follows it closely as it moves (it's hidden behind Yoshi and Mario's bodies), hits whoever it reaches, and every enemy it licks makes him an egg |
| Y (hold for more) | Throw a big 3D egg (white with green spots, tumbling) at the nearest enemy in the cone ahead: it arcs to it and bounces once off the ground. He has up to six (shown under the caps), a new one every 3 s and one per lick |
| RB (hold for more) | Mario's fireball from the saddle, a palm thrust straight at the nearest enemy ahead |
| LT in the air | Stomp: a shockwave all around where he lands |
| LB | Burst of speed for 2.5 s, dust behind him (fast enough to run enemies over), then 6 s of rest |

**Style meter.** Devil May Cry's: every hit in a row scores style points (finishers more, and a
hit of a kind not used in the last four a bonus), and the string earns a rank: D DANDY, C COOL,
B BONKERS, A AWESOME, S SUPER, SS SUPER STAR, SSS WAHOO. It's meant to be earned: the same kind of
blow again within 6 s is worth 20% less each time (down to 30%), a long string on one enemy fades,
the points drain away faster the higher the rank (A 1 point a second, S 3, SS 4.8, SSS 7) so a
rank is only held by keeping up variety, finishers, skillshots and timing, and being hurt costs
**two ranks** (from B; below it the string is lost) and cuts the next seconds' points. The MAX
shows' knock-on blows score less per blow. Marks: B 19, A 33.5, S 63, SS 99, SSS 146 points;
mashing one button tops out around C, a plain fight around B, a varied one reaches S, SS and SSS
want a fast, varied, skilful one. A new rank slams in with its word, a soft red coin chime a note
higher each time and Mario cheering from S up; a bar shows the time left before the string drops.
A hurt shows "MAMMA MIA" (under the string, if it's kept). Reaching S earns the fight's **Star**
(see [Star Power](#star-power)). From rank
S, a combo's finisher is a **Star Finish**: a golden flash, stars and coins flying out, a banner
and a longer slow motion. A finisher that takes an enemy out, or any kill from rank B up, slows the
world down for a moment (`src/style.rs`).

Hits come with hit-stop, star bursts, shockwave rings, a swing trail and a camera shake on the
heavy ones (`src/impact.rs`). Fireballs are balls of fire in 3D (`src/flame.rs`), low-poly like
SM64's own, in layers: a churning heart (the giant one's magma, dark crust over glowing cracks; the
sun's white-hot plasma), a shell of flame glowing at its rim, tongues of flame streaming back, a
tail, and SM64's flame sprites, embers and smoke round it all. Each charge level is grander than
the last: from the full charge on, rays of light round its rim and its light on the ground; the big
ones' heat ripples off them and warms the screen as they fill it (inside the sun, all fire), and the
sun has arcs of fire looping off its face. Their bursts swell in a flash of fire; the giant one's
and the sun's raise a dome of fire over the ground and billowing smoke (the sun's a mushroom, a
white shock wave racing ahead), with stars. A wall in the way fades a ball by how much of it it
hides (rays to its heart and rim), and faces are cut at the camera instead of lost, so a sun held
right by the camera never vanishes or goes flat. Far off they use fewer faces, and one too small on
screen is a single sprite.

**The camera on slopes.** Lakitu reads the ground's grade ahead of Mario (even steep hillsides):
uphill it comes down a little and looks up the slope, downhill it rises and looks down the way he
goes.

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

**Coins as a purse.** The coin counter (kept per character) is now money, and each coin still heals
a wedge. The big abilities cost coins on top of their waits: **Capture 8**, **Mega Mushroom 8**,
**Rock Mushroom 5**, **portable cannon 4**, **wing cap 3** (a "NEED COINS" if the purse is short; `src/coins.rs`).
Like Elden Ring's runes, **half the purse is lost when Mario dies and stays in a bag where he last stood**
(a fall into the abyss leaves it at the edge; it keeps its place through loads and warps):
a big spinning coin in a golden glow under a soft golden column of light (seen from far off),
marked on the compass with its distance; touching it gives the coins back. Dying again before that
loses that bag for good. The bag is kept per character between sessions (`er_mario_bag.txt`) and is
found again from any tile of the overworld: the mod learns how the 256 m map tiles line up the first
time you walk from one into the next (`er_mario_tiles.txt`, logged as `coins: tile axes calibrated`). In co-op every player has a purse and coins of their own: each game drops a coin
where an enemy near that player dies, so Mario and Luigi each see and take theirs.

**Compass.** Elden Ring's compass is hidden with the rest of its HUD, so there is one at the top
middle: a strip with N / NE / E / SE ..., ticks every 15° and the way the camera looks (north is
+Z; if it comes out turned, `compass_north = 180` in `er_mario.ini` turns it). The coin bag sits on
it: pinned to the strip's end with an arrow when it's behind you, with a ▲ / ▼ when it's more than
10 m above or below, beating faster within 50 m, and as a grey coin with a "?" when it's in another
part of the map (a dungeon, another area).

**A light in the dark.** In caves, catacombs, tunnels and the underground, Mario carries the
Lantern's light (the game's own light, from the Lantern's effect) and a small Fire Flower flame
flickers by his left hand. It isn't doubled if you have the real Lantern lit, and it goes out when
you leave. `light = on / off` in `er_mario.ini` forces it; `light_areas = 30, 31, 32` sets which
kinds of map count as dark (the log names each area as you enter it: `light: area m31 ...`).

**The kart.** A tap of L3 is now Mario's own [Mario Kart kart](#mario-kart-kart); L3 held 1.5 s
(a green ring fills over him) is still the whistle for Yoshi. A test of whether the mod could drive Torrent himself (the kart's other way to be built)
is still there: `kart_test = on` in `er_mario.ini` gives L3 back to the whistle; ride Torrent and
`logs/er_mario.log` says every 2 s how much the game moved him on top of the mod (`kart test: ...`),
then `kart test: Torrent can be driven` or `... fights back`.

**Enemy grabs.** Elden Ring's grabs work on Mario: a troll's fist, a beast's jaws, a boss lifting
him up. The game holds him and carries him through its throw while Mario dangles in its grip,
kicking and groaning (SM64's Chuckya grab); one that holds him inside its body swallows him whole,
Bubba style, and he's gone until it spits him out. Let go, he's thrown off with a cry. The throw's
damage counts in wedges (`src/grab.rs`). When the enemy's animation lifts or swings the Tarnished's
body, Mario's pose follows it (`src/engine_mario.rs`), so he's really up in the troll's fist.

**Ailments and fire.** Elden Ring's ailments build up on Mario as on any Tarnished, the game does
their damage (in wedges), Mario shows them the SM64 way (`src/status.rs`), and each costs him
something of its own while it lasts:

| Ailment | What it does to Mario |
|---|---|
| Poison | Drains him (the game's damage); he coughs (Hazy Maze's gas) in green puffs; **heavy legs**: 85% of his top speed while it drains him |
| Scarlet rot | Drains him (faster); orange puffs; **the rot eats healing**: coins heal half as much |
| Blood loss | A red burst: **a wedge and a half at once**, no knockback |
| Frostbite | **Frozen stiff** for a second, then shivering; **frostbitten for 30 s**: every blow costs 20% more; icy glints |
| Sleep | **Knocked out** up to 3 s (mash buttons to wake up sooner; a hit wakes him) |
| Madness | **Shocked**, a yellow flash; his **style meter (and MAX) and a charge in his hand are lost** |
| Fire on him | SM64's burning run (the burn's damage is Elden Ring's) |
| Deathblight | The game's own: instant death, as in Elden Ring |

The stuns (frozen, asleep, shocked) land even in the middle of a blow on the ground, and in the air
as soon as he lands. Lava floors give SM64's lava boost; **poison and rot swamps are not lava**:
Mario wades in them with bubbles round his feet while the swamp builds the ailment up (each floor
is told apart by the effects the game puts on whoever stands on it).

**Giant fireball.** Held 10 s, RB throws a ball of fire that works like a kamehameha: regular
enemies it runs into are swept along inside it, burning a little at a time; a boss or a big body
stops it and it clings to him, burning, for a second and a half; then it blows up, hurting
everyone within 5 m, and the enemies it carried fly off. It also blows up against a wall or at the
end of its range.

### Combat v3

Timing, gadgets, hook tricks, finishers and power-ups (spec: `docs/superpowers/specs/2026-10-18-combat-v3-design.md`):

| Input | Move | Notes |
|---|---|---|
| A just before an enemy's blow lands | **Mikiri stomp** (Sekiro) | A somersault over the blow onto its head, a stomp that staggers it, a bounce into the air. Bosses' stance cracks (a few open them for the tail grab). 2.5 s cooldown; too early or late it's a plain jump |
| X right as a blow connects | **Action command**: NICE! / GREAT! | +30% on that blow, every third +50%. Mashing locks it out for a moment |
| X at the end of the dash | **Dash strike** (Hades) | A lunging punch with the dash's speed, at most every 1.2 s |
| back + X | **Launcher**, any weapon | Fists: super jump punch; hammer: launcher; Chomp: bite; boomerang: the upward throw |
| X + A together | **Rising blow**, any weapon (replaces SM64's jump kick) | Up into the air with whoever it catches, to go on with the air string (X up there): fists a rising spin kick, hammer an upswing, Chomp a bite upwards, boomerang the upward throw. From standing, the middle of a string, or the jump A just began |
| forward + Y | **Leap** at the enemy into the weapon's dive | Fists: meteor; hammer: plunge; Chomp: crash; boomerang: fan |
| B toward another enemy mid-string | **Freeflow** (Arkham) | A leap to it (3-12 m), the string goes on |
| 12 blows without being hurt | **MAX finisher** | The weapon twinkles, MAX shows (12 s to use it, blinking the last 3; lost if you're hurt); its next finisher: fists phantom flurry + launch, hammer cross-shaped quake, Chomp ring of fire flinging the small ones, boomerang three big ones |
| LT crouched or sliding + X | **Banana** | Regular enemies slip and fall (the game's own knockdown), big ones stagger, bosses walk over it. 2 at most, 10 s each |
| LT crouched + Y (hold: red) | **Green / red Koopa shell** | Kicked along the floor, off walls; off an enemy it ricochets to the next one within 9 m (never through a wall), going round all of them in turn (three: A, B, C, A, B; two: back and forth), 5 hits (knockdown); the red one homes in from the start. CAROM! at three enemies, PINBALL! at five hits. 8 s |
| LT crouched + B | **Bob-omb** | Lobbed, bounces, goes off after 2 s or on the first enemy: a 3.5 m blast that throws the regular ones up. 12 s |
| LT crouched + RB | **Spring** | A wide trampoline: enemies stepping on it fly up; Mario jumping onto it goes higher than any jump. 15 s |
| LT crouched + LB | **Stopwatch** | The enemy in front stops in time (3 s, bosses 1.2 s); blows on it build up momentum and it flies off when time runs again (bosses stagger). 25 s |
| D-pad left | **Rock Mushroom** | 12 s as a boulder: down slopes faster, off walls, a slam after a drop; bowls regular enemies over (3 at once: STRIKE!), bounces off bosses; A hops, X rams; hits cost half. 75 s |
| R3 as a projectile comes | **Parry** (Sekiro / Hollow Knight) | A palm thrust opens a 0.45 s window: an enemy's arrow or spell that reaches Mario (3.8 m) is turned back as a blue-white fireball with a level-3 charge's strength (about 20% on a regular enemy), homing on whoever fired it; a volley is stopped (up to 3 per press) but only one shot goes back. Only shots coming at him, from enemies (not neutral NPCs or the co-op partner); not while swimming, hanging, on a pole or a ladder, or as the Rock. A press that deflects nothing waits 1.6 s, a deflect 1 s. PARRY! (3 style points) |
| LT held + R3 (on his feet) | **Grab** | The nearest enemy in front within arm's reach (the next one if that can't be taken): a small one is carried like a Bob-omb, B throws it far (it flies like the home run, bowls over whoever it meets and lands alive, hardly hurt: the fun is the throw, not the damage); this is the only way to grab: no blow or dive picks one up; a medium or big one stunned, its stance broken (by the game's poise, a Mikiri, a wall slam: a soft chime tells), by the tail, never ridden then: spin the stick, B lets go and it flies (3-8% when it lands) |
| RT on a small enemy, then X | **Home run** | Batted away along the camera, bowling over whoever it meets |
| RT on one enemy, RT on another | **Tether** | The two yanked into each other |
| A in the air next to a colossus (an enemy taller than 7.5 m or wider than 3 m: the really huge ones), or A as the AoT hook's line brings him to one | **Climb the colossus** (Shadow of the Colossus) | Mario grabs on to the spot nearest him: its bones are the spots (feet, knees, hips, hands, arms, shoulders, back, neck, head, tail: they move with it), and one whose bones can't be read gets spots round its body. Hanging, his **grip** (a ring by him) runs down, faster the harder it thrashes, and comes back standing on its back or head; out of grip, or hit, he falls. A leaps to the next spot (the stick picks which way, else up), B lets go, LT held **slides him down the limb**, spot to spot and faster and faster (A off it jumps with all that speed, RT throws the rope and the slide's speed feeds the swing; off the limb's end he's flung), RT throws the rope from up there. Two **weak points** glow faintly on it (pale gold stars, clearer as he nears them: its head, and its back or a limb): X held on one charges the star strike (he trembles), let go it lands, harder the longer it charged (WEAK POINT!, a stance crack, a slow moment); X tapped anywhere else is a small blow. Every star struck out: **COLOSSAL!**, its stance broken (the tail grab's moment). The camera backs off by its size and sinks below him as he climbs, looking up at the beast |
| RT held as the cap catches a spot high above (a branch, a ledge, a ceiling; with nothing where the camera looks, one ahead and above in a cone), or RT thrown from on a colossus | **The rope** (Spider-Man, Bionic Commando) | Mario swings from it on a rope, a real pendulum. Thrown from on a colossus it hangs from its highest steady spot in reach (head, neck, back, shoulders, never a flailing hand) and follows it smoothly, round its body (never through it), the stick pumping the swing. A held climbs up the rope (shorter and faster, off the ground), B held lets more out; on the ground he runs along it, the rope taken up. All the way up, A grabs on to that spot. RT let go: off along the swing with all its speed and a bit more, up (the catapult), in a flying kick that hits whoever it meets. Rope to rope in the air; near a colossus's spot, A grabs on. RT tapped is the zip, as before. The hook's reach and the rope's length go by the place: shut in, 20 m as ever; out in the open by a huge colossus, up to 50 m of hook and 40 m of rope |
| LT + R3 next to a medium or big enemy worn down to half its health (the only way up) | **Capture** (Odyssey) | Only once it's at half its health or less: a gold ring with Mario's face closes in over its head for a moment, with a soft chime, when it gets there (God of War's prompt). Up on its back (RT can be let go) it fights for Mario for 30 s: the stick steers it, X / Y its own light / heavy attacks, which hurt the enemies in its reach in front of it, B a charge that bowls enemies over; blows meant for Mario hit it instead (worn out, he hops off), and its own blows and throws never hurt or unseat him; A or RT again hops off behind it; into a wall: WALL SLAM, its stance broken. One rooted to its spot (a giant plant: it never moved while you fought it) only turns where the stick points, to aim its attacks (no walking, no charge). Big and medium enemies, not bosses with a health bar. 20 s wait after it |

Creative kills get a **skillshot** (style points and a callout): AIR RAID, STOMPED, ROCK BOTTOM,
FRIENDLY FIRE, OUT OF THE PARK, KABOOM, BANANA SPLIT, SHELL SHOCK, PARRY THIS, MAXED OUT, and
DOUBLE / TRIPLE / MULTI KILL; the gadgets have their own (SLIP!, CAROM!, STRIKE!, HOME RUN!,
TETHER!, BOING!, STOP!). Finishers punch the camera in a little. Every gadget and power-up
refuses with a buzz when it can't work. The trap kit's things, the shells and the boulder are real
3D models (built from the ROM-free palette into the gauntlets' piece).

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

### Star Sword

The fifth weapon (`src/sword.rs`; its moves are data in `src/swordmoves.rs`, picked from the
buttons by `src/swordplay.rs`, played by the move engine `src/script.rs`): a real 3D sword built
into the gauntlets with the trap kit, shaped after Dante's Rebellion in Mario's colours: a
greatsword 1.8 m to its tip (longer than Mario is tall), its broad steel blade with light-blue
edges, a dark-red fuller with five gold diamonds and two notches at its base; for a guard Dry
Bowser's skull (horns tipped yellow, red glints in its eyes, its teeth on the blade) with two gold
ribs sweeping down; a red grip for both hands with yellow rings and Bowser's spiked bracelet for a
pommel. It rides across Mario's back, its hilt over his right shoulder and its tip out past his
left side, and comes out with a flourish at the first blow; his arms follow the blade (arm IK), his
legs step with each cut (leg IK, `src/footwork.rs`: a step toward the cut's side, round the enemy
from side to side along a string, the feet planted, never through walls or into bodies), and its
cuts hit along the blade itself, leaving a red-gold trail. Its moveset is Dante's Rebellion (Devil
May Cry 3) with some of Nero's (Devil May Cry 5), and it never spins him round: every cut moves at
once from the cocked pose (the hit in 2-4 ticks), chains or dodges out from 0.13-0.2 s, the stick
steers the follow-through, the hit-stop grows along a string, and the pose is held a moment after
it (longer on the finishers); a lunge
stops dead on its enemy. Its swings and hits sound its own (synthesized by the mod, `src/sfx.rs`:
a whoosh through the air, a ring of steel over a thud, deeper on the heavy ones), and each cut
shoves a regular enemy back a little (the heavy ones more). Recorded swings and hits sound better
still: put WAV files in a `sounds` folder in the mod's folder (`sword_swing_*.wav`,
`sword_swing_heavy_*.wav`, `sword_hit_*.wav`, `sword_hit_heavy_*.wav`, any rate; one picked at
random each time, a hit's over the synthesized thud); the log says
`sfx: N sword recordings from ...`. The ones I use are CC0 packs from OpenGameArt (see
[Credits](#credits)); they're not in this repository. Its element is **lightning**. Every
blow that lands can be cut short into the dash, a jump or a swap; the stick is read against the
enemy in front (not against where Mario faces).

| Input | Move | Share |
|---|---|---|
| X, X, X, X | **Combo A**: a diagonal cleave, a rising backhand, a rising diagonal, then the **Rebellion chop**: the blade cocked behind his head in both hands, a two-handed chop with a deep lunge, frozen there; it knocks them back | 7 / 7 / 8 / 12% |
| X, (beat), X | **Thunder cleave**: both hands overhead and down, a crack of lightning where it lands, cracks a stance | 14% |
| X, X, (beat), X | **Million Stab** (Combo B): a flash, nine stabs in 1.2 s rooted in a wide stance, a last full-length lunge that blows them back | 2% each + 10% |
| X, X, X, (beat), X | **Leap Splitter** (Nero's Split): a hop of 0.8 m, the blade overhead in both hands, then a plunging vertical cleave that crashes into the ground, a short ground wave (3 m), a heavy hold and a camera jolt; small enemies bounce 1.5 m off the ground for a relaunch. No spin | 10% + 6% |
| back, forward + X | **Stinger**: a deep coil, then he shoots point first at the enemy, almost flat, up to 8 m, and stops dead on it | 12% |
| mash X as it lands | **Million Stab** off the Stinger | 2% each + 10% |
| back + X, or Y | **High Time**: a deep coil, then a rising cut from the ground straight up that launches small and medium enemies; **the button still held** and Mario goes up with them | 10% |
| X, X, Y, or X + A | **Star launcher**: the same rising cut, Mario always going up with them for the air string | 10% |
| X held, let go | **Drive**: charged while he walks (the blade cocked high behind his right shoulder, crackling), let go: a cut down and forward into a low lunge that sends a wave of lightning along the ground, 10 m. Full (0.9 s): a heavier cut and a crack of lightning with the wave. Let go right as it fills (with its chime): **JUST!**, three cuts and three waves | 10% + 10% / 12% + 10% / 8% + 3 × 8% |
| X, Y | **Cross cut**: two crossing cuts leave a glowing X in the air that bursts 0.4 s later | 7% + 12% |
| forward + Y | **Helm Splitter**: a leap onto the enemy, a plunging cleave, a ring of lightning where he lands | 16% + 6% |
| B | **Star Step**: a short quick step (2 m, untouchable) whose first 0.2 s are a parry window against an attack that has just begun, up close and facing him. A small enemy (not a boss): **Phantom Cut**, he vanishes in a star streak and reappears behind it sheathing, 0.35 s later the cut line flashes across it (knockdown, then BEHIND! x1.3 for 1 s; if there is no free spot, the parry). A medium or big one (bosses too): **parry**, the blades clash, it staggers, he slides back 0.5 m and the next blows are a **Riposte** (x1.6 for 1.6 s). A colossus: a perfect dodge (Witch Time, then X is the Flurry Rush). No attack: just the step. Parry and Phantom Cut share the perfect dodge's 2.5 s cooldown | 12% (Phantom Cut) |
| B in the air | **Star Trick**: a parry if a blow is coming (no slide); else a blink 1.5 m over the enemy he fights (8 m, not a boss or a colossus) with a star burst and a short hang, the air string goes on from it. No enemy near: the plain air dash. Two an airtime | |
| B, then X (or X in the dash) | **Lightning iai**: through the enemy and past it in one cut, its line cracks with lightning a beat later behind him | 11% |
| X in a perfect dodge's slow time | **Flurry Rush**: in at the enemy, six cuts and a wide last one, untouchable meanwhile (Breath of the Wild); one per perfect dodge | 6 × 3% + 8% |
| X x4 in the air | **Aerial Rave**: three cuts that hold him up, then the blade raised high and down through them, Mario dropping with it (once a jump; three times with an enemy up there) | 6 / 6 / 7 / 10% |
| forward + X in the air | **Star Calibur**: the point drawn back, then a thrust 25° up and ahead that carries him toward the enemy (up to 6 m) and leaves him hanging a moment; the air string goes on out of it (once a jump) | 9% |
| Y in the air | **Helm Breaker**: a moment's hang, a fall straight down with the blade, a ring of lightning and a long crouch where he lands | 12% + 6% |
| LT in the air | **Pogo**: the point straight down; on an enemy he bounces up again (three a jump; after that LT is the ground pound again) | 8% |
| a NICE! with the sword | **Star Charge** (Nero's Exceed): each NICE! lights one of 3 gold stars along the blade; the next heavy blow (the Rebellion chop, the Stinger, the Leap Splitter) spends them all, +15% each, a gold burst and a soft zing. Lost when he is hit, after 6 s without a new one, or on swapping weapon | +15% a star |
| LB into the sword mid-fight | **Quick draw**: drawn as an iai cut in front (in the air, a cut down across him) | 10% |
| a string's finisher at full health | **Sword beam**: a small crescent flies 12 m (Zelda's) | 4% |
| a finisher with MAX ready | **Star Cut End**: a flash, the world slowed, then cuts of lightning across every enemy within 8 m, three times (Vergil's Judgement Cut End) | 3 × 5% |

The shares are a regular enemy's max HP before the damage factor (0.57 in vanilla, see
[Damage balance](#damage-balance)); bosses take their usual small part. Long strings fade as with
every weapon. Each cut strikes an enemy once, however long the blade stays in a big body (the
hit-stop no longer lets one cut land again).

**RB with the sword** is lightning of its own, never a projectile in another colour
(`src/bolt.rs`). Its bolts are 3D (a white-hot core in a bright tube with a violet haze, forks off
it, struck twice like real lightning), with a thunderclap (the crack and the rumble, synthesized),
a flash of the screen and a jolt of the camera; its charge is a small crackling ball in his hand:

| Charge | On the ground | In the air |
|---|---|---|
| a tap | **Skyward Strike**: the sword raised, a bolt shoots up off it and falls on the enemy aimed at (10%) | **Zeus throw**: a bolt javelin hurled down at the enemy (12%) |
| 1 s | a thicker bolt (14%) | a bigger javelin (16%) |
| 2 s | a bolt with a blast round him (20% + 8%) | a javelin (22%) |
| 4 s | **Ricochet Bolt**: he thrusts the sword ahead (no spin) and a horizontal bolt leaves the tip at the first enemy in front (28%), then ricochets like the shell: up to 5 jumps of 8 m in sight, to the one hit fewest times, then longest ago, then the nearest, never the same twice in a row, each hit 0.85 of the one before | the javelin splits in three where it lands (24%, then 10% on the three nearest) |
| 10 s | **Storm cloud** (Mario Kart's Thunder Cloud): a dark 3D cloud flies over the enemy and rains bolts on everyone under it for 4 s (6% a bolt), then a giant one (45% within 4.5 m) | the same, cast from the air |

### Elements

Each weapon imbues Mario's blows with one of Elden Ring's elements, and turns RB into a move of
that element's own, never a fireball in another colour (`src/element.rs`, `src/rbmoves.rs`; the
looks in `src/elemfx.rs`):

| Weapon | Element | What it builds up | RB |
|---|---|---|---|
| Fists | Fire | Nothing: an enemy weak to fire takes more of every blow | The fireball (charges up to the giant one and the sun) |
| Hammer | Frost | **Frostbite**: a burst of 9%, then for 10 s every blow on him hits 20% harder | **Ice rocket** (Doomfist's, with the hammer): the hammer hauled back over his shoulder while a block of ice grows on its head with the charge, then Mario rockets head first behind it into the enemy, 6 / 8 / 11 / 14 m; it knocks the enemy back, and slams it into a wall right behind it (WALL!). In the air: the **ice dive**, a diving punch with a wave of frost where he lands. **10 s: the Glacier fist**: a huge fist of ice flies 25 m through everyone in its way, a ring of ice spikes bursting from the ground where it stops |
| Chain Chomp | Blood | **Blood loss**: a burst of 12% | **Dark beam** (a small, evil kamehameha, in 3D: a black core inside a crimson tube, strands of darkness twisting round it, rings rushing down it): pouring from his hands 0.6 / 0.93 / 1.33 / 1.8 s by charge, 18 m to the first wall, through everyone in it, pushing regular enemies back a little and swinging itself a little onto the nearest enemy ahead; from 2 s of charge the stick swings it round; each hit bursts and sizzles where it strikes; in the air it holds him up while it lasts. **10 s: the Abyss beam**: three seconds of a huge one, swept round with the stick, drawing regular enemies in toward its core, bursting where it ends as it goes out |
| Boomerang | Poison | **Poison**: about 1% a second for 20 s | **Poison sweep**: the arm swung across spraying, a carpet of poison over the ground in front of him: 120° and 3 m for 6 s up to all round and 6 m for 10 s by charge; whoever walks in it is poisoned every half second. In the air: sprayed down, a pool where it lands. **10 s: the toxic swamp**: two turns spraying, a 10 m swamp for 12 s, bubbling, slowing the enemies in it (bosses only a little) |
| Star Sword | Lightning | **Shock**: a bolt from the sky on him (16%) that shakes his stance hard, and jumps to the two nearest enemies within 6 m (60% and 40% of it) | Its own moves: see [RB with the sword](#star-sword) |

Every enemy takes them as Elden Ring has him: his resistance to each status (NpcParam) is what it
takes to set it off (an immune one never; never less than a string of blows: some five of the
hammer's heaviest, a dozen bites of the Chomp, a dozen boomerang throws), and a blow of an
element he's weak to, or resists, is
worth a little more or less (x0.8 to x1.25; for the statuses, against his own resistances to the
others: each enemy has the one he resists least and the one he resists most): "WEAK" or "RESIST"
over him the first time. A status
set off makes him tougher to it for a while (a boss much more), and what's built up fades if he's
left alone, so no weapon wins everywhere: switching pays. Every element's RB charges at 1, 2 and
4 s, and to its giant at 10 s; the sun (20 s) stays the fists' own.

Taking a weapon shows its element: a burst of it at Mario's hand with a soft SM64 sound, then now
and then a mote round the weapon (embers, snowflakes of ice, wisps of darkness, bubbles). The HUD
shows the element's badge by the weapon (the fists' alone in its place), the weapon banner names
it, and a thin bar under an enemy's health fills with what's built up on him (glowing while
frostbite or poison is on him: its time left). Frostbite clings to him as ice crystals, blood loss
sprays, poison bubbles off him; the statuses go through the same shares as the blows (bosses a few
% at most).

### Pair finishers and criticals

**Pair finishers**: land a string's finisher with one weapon, and right away (L1) bring in another
one's swap strike: their two elements react on the enemy you finished on, **MIX!**, once a string
(`src/reaction.rs`):

| Weapons (elements) | Reaction |
|---|---|
| fists + hammer (fire + frost) | **Steam**: a burst of steam round him, small enemies staggered |
| fists + sword (fire + lightning) | **Plasma**: a blast of fire and lightning |
| fists + Chain Chomp (fire + blood) | **Boiling**: his blood boils, a heavy blow on him and a burn after it |
| fists + boomerang (fire + poison) | **Ignition**: the carpets of poison near him go up in flames (whoever stands in any takes it once; with none, a small burst on him) |
| hammer + sword (frost + lightning) | **Shatter**: a ring of ice shards, his stance cracked hard |
| hammer + Chain Chomp (frost + blood) | **Crimson spikes**: spikes of frozen blood rise under him |
| hammer + boomerang (frost + poison) | **Toxic hail**: pellets of poisoned ice fall round him |
| sword + Chain Chomp (lightning + blood) | **Blood bolts**: bolts jump from him to the three nearest |
| sword + boomerang (lightning + poison) | **Corrosive arc**: every poisoned enemy near is shocked |
| Chain Chomp + boomerang (blood + poison) | **Rot**: he rots, a little every half second for 4 s |

**Criticals** (Mortal Kombat 11's Krushing Blows): the right blow at the right moment hits ×1.6
with a moment of slow motion, the camera punched in and **CRITICAL**, once an enemy every 20 s
(`src/critical.rs`): a launcher on an enemy in the middle of its attack; the hammer's heavy blows
on a broken stance; the sword's thrust, iai or stabs in his back; a Chain Chomp bite on one up in
the air; a boomerang striking him both on its way out and back.

### Air

Small enemies only (a medium one only while stunned), never bosses or big bodies (`src/juggle.rs`):

- **Wall bounce**: a juggled enemy carried by a hit into a wall (at 2 m/s or more) bounces off it
  back toward Mario at 3 m/s, a puff of dust and a soft thud, 3% extra, once per enemy per string.
  Floors and ceilings don't count. It stays within reach for the next blow.
- **Spike and bounce (OTG)**: the Helm Breaker, the Leap Splitter and the hammer's diving comet
  bounce a small enemy 1.5 m up off the ground, ready to relaunch; once until it has touched the
  ground again.
- **Air billiards**: a launched enemy flying faster than 4 m/s that touches another launches it
  too (2.5 m up, 3% to both), up to 3 a string.
- **AIR xN**: blows in a row without Mario touching the ground raise the style gained (not the
  damage) by 5% each past the first, up to x1.5; a small "AIR xN" under the HITS counter from the
  third.

### Star Power

Reaching rank **S** earns the fight's **Star** (one a fight, one kept at most: a star by the style
meter with "LB+RB"; a fight ends after 20 s with no blow dealt or taken). **LB + RB together**
lets it out for **12 s** (`src/starpower.rs`): rainbow glints round Mario and SM64's power-up music
(soft), his blows worth **×1.2** (bosses ×1.1) and **×1.5** on a stance (bosses ×1.25), and small
hits shrugged off: their health is still taken as SM64's own blow would take it (and none for a
second after one), but no knockback, no rank lost, the charge kept. Without a Star, LB and RB
pressed together are just themselves.
The weapon in hand gets a trait:

| Weapon | Under Star Power |
|---|---|
| Star Sword | every finisher that lands throws the sword beam, at any health |
| Hammer | its quakes reach half again as far |
| Chain Chomp | its chain reaches farther, its cyclone 30% wider |
| Boomerang | a plain throw is two, curving out either side |
| Fists | a trail of fire where he runs: embers that burn whoever steps in them |

It drains with time and with the damage he deals (a rainbow bar). **LB + RB again** (after its
first 3 s) is the **STAR BLAST**: one big blow on every enemy whose body is within 7 m, the stronger
the more time was left. When it's over (either way) the style rank comes down to B.

Logs: `starpower: a Star (S rank)`, `starpower: on (12 s)`, `starpower: STAR BLAST x0.83 on 3`,
`starpower: over (time)`, `starpower: shrugged off a small hit`, `style: hurt, two ranks down (SS -> A)`.

### Attack on Titan hook

RT on a big enemy (a boss, a strong one, a giant) no longer zips Mario to it: the cap **anchors to
one of its bones**, the one nearest the middle of the screen (its neck, head, an arm, a leg, its
tail: the canonical bones the colossus climb uses), a ring on it, and the camera turns round behind
Mario to keep him and the bone on screen (`src/odm.rs`).

| On the line | What happens |
|---|---|
| Left stick held | He swings round the anchor (at most a quarter turn a second, 3 s a line), the line's length kept |
| Left stick let go | The line **reels him in**, faster and faster: 12 → 30 m/s |
| B | A **burst of gas** (a hiss): +40%, fading over 0.8 s. **With the stick to a side: a sidestep** round the anchor that way, quick at once and dying away in 0.4 s (the 3D maneuver gear's). Three charges (the pips by him), all back on any hit or on landing |
| Right stick flicked | The anchor moves to the next bone that way on screen |
| **X in the last quarter second** before he gets there | **The strike**: with the Star Sword, the hammer, the Chomp or the boomerang a spin slash, with the fists a flying kick. Harder the faster he comes: ×(speed / 20 m/s), from ×0.3 to ×1.8. On the **nape or the head from behind**: ×1.5 more, a stance crack and **NAPE!** (bosses: ×1.5 at most, all together) |
| No X | A glancing hit, worth half |
| A as he gets there, on a colossus | He grabs on (the climb) |
| RT | Let go: off the line, falling on with its speed |

The line's colour tells his speed: blue (under 12 m/s, a "tink" on the hit), gold (under 20), orange
(under 28), red (above, a heavy crash); the ring turns white in the strike's window. After any hit
the blow holds a moment, he backflips up and away (about 7 m), and **A** (or RT) throws the next
anchor. The line drops if it runs through the scenery, he hits a wall on the way, a blow knocks him
off, or after 6 s.

Logs: `odm: anchored on its Neck (12.3 m)`, `odm: STRIKE at 27.4 m/s on its Neck (tier 2, x2.05,
from behind)`, `odm: glancing hit at ...`, `odm: gas (1 left)`, `odm: flick to its Hand`,
`odm: re-anchor`, `odm: line dropped (caught on the scenery)`, `hook: zip-kick`,
`odm: vault over it (...)`, `combat: from behind x1.3`, `hook: the rope's cone found a spot ...`.

### Portable cannon

**Hold D-pad up half a second** (a ring fills over Mario's head) and an SM64 cannon pops up out of
the ground where he stands (`src/cannon.rs`): a pink base, a dark barrel; he hops in. It needs open
sky over him (not indoors, not under a roof), no boss fight, **4 coins** (back if he gets out without
firing), and it's ready again a minute after the shot (the seconds left show at the bottom right,
"CANNON 38"). A quick tap of D-pad up is still the wing cap, which now comes on as the button is let
go.

| In the cannon | What happens |
|---|---|
| Left stick | Aims it, SM64's way: left and right turn the barrel, up aims lower, down higher |
| Right stick | Aims it the camera's way (up aims higher) |
| A held | Fills the power (the bar at the bottom, a second to full); let go and it fires |
| B | Gets out (the coins back) |

Lakitu's camera sits behind the barrel; white dots show the flight and a gold ring where it lands,
with how far off: about **35 m** at no power to **100 m** at full (on flat ground at 45°; it was
three times that, and with the wing cap flying on from the top of a shot a map went by in a few).
Mario flies in SM64's cannon flight, on his stomach with a "Yahoo!", and lands in a belly slide, or
bounces off a wall with SM64's stars; the wing cap doesn't take over the flight. **LT in the
flight** is a ground pound straight
down: where he lands, a shockwave hits every enemy within 6 m. A flight out of the cannon never
hurts him by the fall. If the game hasn't loaded the far end of a shot yet, the shot is capped short
of it (a white mark on the power bar); with nothing loaded to land on at all, "NO LANDING".

Logs: `cannon: planted at ...`, `cannon: not here (no open sky)`, `cannon: fired at 64% (44.4 m/s,
42 deg), landing 212 m off`, `cannon: capped to 47% (the far landing isn't loaded)`,
`cannon: landed 208 m off after 7.3 s`, `cannon: hit a wall`, `cannon: wing cap, flying on`,
`cannon: pound from 31 m up`, `cannon: POUND on 3 (180 m off the cannon)`, `cannon: got out`.

### Mario Kart kart

**L3** (a tap; keyboard R) and Mario hops into his kart where he stands (`src/karting.rs`, `src/kart.rs`,
`src/kartplay.rs`): Mario Kart's red kart with the M on its nose, fat tyres with gold hubs that
spin and steer, a steering wheel he holds, two chrome exhausts. It drives like a small car with
weight, not a cursor: a chassis of 180 kg on four sprung wheels, so it pitches as it brakes and
accelerates, leans into turns, rides over bumps and off ramps and lands on its springs. Its roll
is held against the ground under its wheels (Mario Kart's lean, not a rollover: turning and
drifting over rough ground no longer tips it onto its roof) and in the air it levels itself off; a
hard enough knock can still tumble it, and then **A hops it back onto its wheels** at once (or, on
its roof for two seconds, it's put right). And it's fast, Mario Kart's way: 32 m/s
flat out, 20 m/s in two seconds, its tyres hugging Elden Ring's uneven ground (they keep their grip
over the crests, and it's pressed down onto the ground with its speed). On top of that: drifts,
mini-turbos, mushrooms, shells and bananas, coins for speed. It's for the open world (like
Torrent: "NOT HERE" in a dungeon or a legacy dungeon) and needs room where he stands ("NO ROOM").
A tap of L3 again and he hops out.

| In the kart | What happens |
|---|---|
| RT | Throttle (32 m/s flat out, a little more with coins) |
| LT | Brake; held at a standstill, reverse |
| Left stick | Steer (the front wheels turn less the faster it goes) |
| A | Hop (on its roof or its side: back onto its wheels). **Held while steering at speed: a drift** (sparks blue, orange, then purple: let go for a mini-turbo of 0.6 / 1 / 1.5 s). Off a ramp or a lip, A in the air is a **trick** (a barrel roll, "Yahoo!") and its landing a boost. A while a boost is on: a **super hop** |
| LB | A mushroom: a 1.2 s boost (three, one back every 10 s) |
| X | A green shell straight ahead, faster than the kart; it bounces off walls and ricochets between enemies (the trap kit's own, with its wait) |
| Y | A banana dropped behind; held, thrown ahead as you let go (the kit's, two at most) |
| RB | A Fire Flower fireball from the nose (one every 0.6 s) |
| Right stick | Look round (Lakitu comes back behind after a second and a half) |

Lakitu's camera chases behind the kart at Mario's own camera distance (C-down / C-up on foot pick
near, middle or far; the kart's view keeps it), and pulls well back and up with speed: 4 m farther
at the top speed, 3 m backing up fast, almost 2 m more in a boost, with streaks of wind at the
screen's edges. It follows the way the kart goes (in a drift, between its
nose and its slide) on a smooth spring, rides out the springs' bounce and the shiver of rough
ground, and stays behind when backing up. The engine is synthesised by the
mod (no recordings): a buzzing two-stroke that climbs with the speed, the tyres hissing in a drift
(`kart_engine = 0.5` in `er_mario.ini` makes it quieter, `0` silent). **Coins** picked up while
driving also count for the kart: up to ten, each a little more top speed (bottom right, with the
mushrooms and the shell's and banana's waits).

**Running into enemies:** a small one is flung away (it lands alive), a medium one is knocked back
staggering and the kart bounces off, a big one or a boss barely feels it and **the kart spins
out**: no cheese (once a second per enemy, three seconds for the big ones). **A blow on Mario**
costs its health as usual, spins the kart out and knocks three coins out of it; a heavy blow that
would send him flying throws him out of the kart. A fall from more than 22 m kills, as on foot.
Spiritsprings need Torrent; the super hop is the kart's own.

Logs: `kart: in at ...`, `kart: not here (not in the open world)`, `kart: mushroom (2 left)`,
`kart: mini-turbo 3`, `kart: trick`, `kart: green shell`, `kart: banana dropped`,
`kart: rammed ... (Small, 18.2 m/s, x0.91)`, `kart: hit by c4630, 3 coins lost (4 left)`, `kart: knocked out
of the kart`, `kart: fell 31 m: lethal`, `kart: no ground ahead, slowing`, `kart: out (L3) after 42 s`,
and every 10 s while driving `kart: 21.4 m/s, coins 6, wheels down 4, drift 0, mushrooms 2`. The
model goes into the Mario set's legs piece, drawn with its Fabric material (the piece's own
colours; the Belt one it had first came out grey), assets version 19: two launches after updating;
`kart: drawn by the overlay` means it couldn't, and it's drawn over the game instead.

### Damage balance

Every hit takes a share of the enemy's max HP. Bosses take a small fraction of that, more for the
close and risky hits.

| Attack | Original | This fork |
|---|---|---|
| Hammer | 67% of enemy HP | **12-30%** per hit (see [the hammer](#new-moves)), the slowest and riskiest; its heavy blows knock regular enemies down and crack strong enemies' stance |
| Punches, kicks, fist combos | 67% of enemy HP | **1-23%** per hit. A regular enemy takes about 10 punches; finishers hit hardest, the super jump punch's rising blows barely scratch. The game's own hit that goes with a blow (its reaction, its aggro) carries only 5% of the bare fist now: at 100% it added the whole fist on top (a third of a regular enemy's health per punch in Reforged) |
| Grab and throw (LT + R3, B) | — | **1.5%** on landing, never a kill; whoever it bowls over takes the bowled share and goes down |
| Boomerang | — | **2-4%** per pass (it hits going and coming back); the big one about 2% per hit while it hovers |
| Counters (after a perfect dodge) | — | **x1.6** for 1.6 s |
| Chain Chomp | — | **2-10%**, on every enemy it reaches; the cyclone the least. Fast, wide, light: it wins by numbers |
| Long strings | — | Past 10 punches' worth of blows on the same enemy each blow is worth 5% less than the one before per punch it weighs (a hammer blow weighs two or three, a cyclone tick a third), down to 40% (finishers 70%), so long combos last instead of melting the enemy, whatever the weapon |
| Stomp | 50% of enemy HP | **12%**; bouncing on the same enemy without landing halves it each time (100%, 50%, 25%... down to 10%) |
| Ground pound | 67%, no boss bonus | **21%**, boss bonus **×1.3**, shockwave hits enemies around |
| Fireball | — | **9-25%** by charge; the giant one burns **6%** every quarter second while it carries or clings, and its explosion does **64%** to everyone within 5 m; the sun's explosion does **60%** within 9 m and its lava pools **4%** every half second (on bosses about 9% and 0.4%, as measured) |
| The elements' RB moves | — | Ice rocket punch **10-27%** by charge (+ WALL!; charged 2 s and up it sends a regular enemy flying, bowling over others; it lands alive), ice dive **12%** + frost wave **6%**, Glacier fist **51%** on each it passes (regular enemies are blown away) + spikes **17%**; dark beam **1.4-2.5%** every tenth of a second by charge (**9-45%** in all), Abyss beam **2.7%** every tenth (about **80%** in all) and its burst **16%** within 4 m; poison sweep **3.4%** and its carpet **1.3%** every half second (+ poison). Every charge of these is worth a fifth more than the fireball of the same charge at least |
| Pair finishers | — | Steam **4.5%** round him, plasma **8%** within 4 m, boiling **7%**, ignition **8%** once on each in the carpets, shatter **5.5%** (+ stance), crimson spikes **9%**, toxic hail **2.3%** ×5, blood bolts **4.5%** each, corrosive arc **5.5%** each, rot **1.1%** ×8; criticals **×1.6** |
| Elements | — | x0.8 to x1.25 by the enemy's resistance; frostbite **9%** and +20% for 10 s, blood loss **12%**, poison **1%** a second for 20 s (bosses: their share, a little more for the statuses, and much tougher to each after it went off) |
| Elden Ring hits on Mario | 1 or 2 full wedges | **a quarter of a wedge** or half of one: Mario has **40% more health** and takes **30% less damage** (`src/tough.rs`: every loss of health, from any source, is cut to half; the meter keeps its eight wedges, each worth more). A lethal fall still kills. Luigi in co-op too |
| Mikiri stomp | — | **9%** and a heavy stagger; on bosses a stance crack |
| Action command | — | **+30%** of the blow (every third **+50%**) |
| Dash strike | — | **12%** |
| Bullet Bill punch | — | **9%** (a kick's weight: it sends them back a little), on bosses ×1.3 |
| Fists' new moves | — | Hundred-hand flurry **1%** a jab ×10 + **6%** uppercut; bullet straight (X held) **11%** / full **15%** (it sends a regular enemy flying, it lands alive); heavy hook **12.5%**; meteor punch **7%**; dropkick **8%** (flies off too); WALL! **+30%** of the blow |
| Hammer, Chomp, boomerang new moves | — | Hammer charge **7%** + slam **16%**; hammer leap (X held) **21%** / full **30%** in its quake; a volley hammer **5%** (three); comet **10%** + ring **5.5%**; Chomp rush **5.5%**, ram (X held) **5.5%** on each in the line, full + **8%** for its last bite; frenzy **1.7%** a bite ×5 + **4.5%**; skim **2.3%** a pass |
| Trap kit | — | Banana **6%** + knockdown, shell **7.5%** a hit + knockdown (up to 5 hits, bouncing from enemy to enemy), Bob-omb **21%** in its blast, spring **2%** |
| Rock Mushroom | — | **7%** a roll-over (once a second per enemy) + knockdown, slam **12%** |
| Hook moves | — | Home run **10.5%** (+6% landing, never a kill), bowled **6%**, tether **9%** each, swing kick **13.5%** |
| Star Power | — | Blows **×1.2** (bosses ×1.1), stance **×1.5** (bosses ×1.25); STAR BLAST **9-21.5%** on everyone within 7 m by the time left (bosses about 1-2.4%) |
| Portable cannon | — | Its pound **17%** on every enemy within 6 m of where he comes down (bosses about 1.9%); the one right under him takes the ground pound's too |
| Kart | — | Running over a small enemy **8%** at 20 m/s (**2.4-12%** by the speed into it, it flies off and lands alive), bumping a medium one **5.7%** (**1.7-8.6%**, a stagger); a big one or a boss half that (bosses about 0.4%) and the kart spins out |
| Attack on Titan hook | — | Strike **8%** at 20 m/s, **2.4-14.4%** by speed, **21.5%** at most on the nape from behind; glancing hit half (**1.2-7.2%**); bosses about **1.5%** at 20 m/s, **2.2%** at most (capped); zip-kick **5.7%**; blows on an enemy he vaulted **×1.3** for 1 s |
| MAX finishers | — | Flurry **2%** ×6 + **7.5%**, cross quake **7%** a step, Chomp ring **10.5%**, Star Cut End **3%** ×3 |
| Star Sword | — | **1-9%** per blow (its table's shares ×0.57: Combo A 4 / 4 / 4.5 / 7%, the thunder cleave 8%, the Stinger 7%, a stab 1%, the Drive 5.7% + its wave 5.7%); crescents 4.5-6%, sword beam 2%; its RB bolts 8.5-23%, the storm cloud 5% a bolt and 37% for its giant one; shock 9% |
| Capture: its attacks **5% / 8.5%**, charge **6%** each, wall slam | — | **11%** and the stance broken |

(The shares above were worked out with a 0.75 damage factor; the current one is 0.57, 0.42 with Reforged, whose enemies are meant to hold out longer (ini `damage_factor`), so take 24% off each (44% with Reforged; the hammer's row is at 0.6 already, see its own table; throws, Yoshi's trample and the bat keep theirs); bosses take about 11% of them (13% with Reforged; ini `boss_damage_factor`), more for the
risky close ones.) Any one enemy is knocked down at most every 3.5 s, whatever does it.

The ground pound is slow and leaves Mario open, so it's worth the risk.

### Fixes and quality of life

- **Mario never switches himself off.** A slow call into SM64 used to switch Mario off for the
  rest of the session after 3 s; now the game waits for it (logged every 5 s) and carries on. A bug
  inside a frame rebuilds Mario on the spot (with his health) instead of turning him off. With no
  ground anywhere near (the map still streaming in after the game moved its world origin), Mario is
  held where he is until it's there, and after 15 s put back on the last ground he stood on; a long
  drop or a high flight isn't mistaken for that (a long ray finds the ground far below).

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
- **A shorter game over.** Bowser shows up sooner after Mario's death (1.6 s) and his iris closes
  faster (1 s); the log records the whole timeline from death to respawn.
- **Fewer hitches in fights.** The game's raycast check of each collision triangle is remembered,
  so fast combat moves no longer re-check hundreds of triangles every few frames.
- **Yoshi no longer turns up on his own.** A talk with an NPC counts as a menu, and the game's
  "use item" was let through for a second after a menu (for the inventory's Use): the punch (X)
  right after a talk used the quick slot's item, the whistle. Now it's let through only with no X or
  R pressed since the menu closed.
- **No invisible walls after a fast travel.** A warp moves the Tarnished as far as the floating
  origin's steps do, and the mod took it for one: SM64 kept the old place's collision state. A jump
  the world origin's shift doesn't account for is now a warp, and SM64's world is read again where
  he is. (If he's ever held by something unseen, the log says what: `blocked: ...`.)
- **The Star Sword's charge looks like lightning** (it was drawn as the poison glob), its burst
  and its mark on enemies too.
- **Less of the mod's time per frame in big fights.** The characters near Mario are walked once a
  tick and shared by everything that asks (blows, shots, the hook, the kit, the kart...) instead of
  once per question; Torrent and a homing fireball's target are looked up by their handles; the
  settings file is read by a thread of its own; Mario's SM64 mesh is copied out of the tick only for
  the debug draws; the coins' line-of-sight rays and the colossus / hook HUDs no longer hold locks
  the overlay waits on; the overlay works out the camera once a frame. The `perf:` lines in the log
  (with `debug = 1`) show the mod's time per part, a new `character roster` among them.
- **The memory in the log.** Every 30 s the `perf: most time in the last 30 s` line ends with the
  game's memory (working set, private, page faults since the last line, handles) and the PC's (how
  much of its memory is in use, how much is free), and so does each respawn's line
  (`respawned: recreating Mario (memory: ...)`): if the game slows down over a long session, it
  tells whether it's the game's memory growing or the PC running short.

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

- **Elden Ring** on Steam, exe version **2.7.2** (patch 1.17.2). On any other version the mod switches itself off.
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
| X | , / left mouse | Punch, punch, kick (auto-aimed combo, see [New moves](#new-moves)). Running: dive. A punch out of a run brakes him into the blow (no sliding past the enemy). Right on a blow's impact: NICE!. Back + X: launcher. X + A together: the weapon's rising blow. With the [Star Sword](#star-sword), X is only its moves (no punches, kicks or dives) |
| Y | | Special move: breakdance sweep, slide kick (running), combos with X and B. Forward + Y: leap into the weapon's dive |
| B | | Dash: a quick lunge where the stick points (ahead without it), invulnerable; X at its end: dash strike; A out of it. Mid-string toward another enemy: freeflow. With the [Star Sword](#star-sword) it is the Star Step / Star Trick (parry, Phantom Cut, blink); with the hammer the guard (heavy roll with the stick pushed); with the boomerang the glide in the air; with the Chain Chomp the slam / harpoon |
| A just before an enemy's blow | | Mikiri stomp |
| RB (hold) | | Fireball, charged 1 / 2 / 4 s; held 10 s, a giant one that sweeps enemies along and blows up; held 20 s, the sun, which leaves lava pools. With the hammer the ice rocket punch, the Chain Chomp the dark beam, the boomerang the poison sweep (each with its 10 s giant, see [Elements](#elements)); with the Star Sword its lightning (the skyward strike, the Zeus throw in the air, held 10 s the storm cloud) |
| D-pad | | Left Rock Mushroom, up Wing cap (a tap; held 0.5 s, the [portable cannon](#portable-cannon)), right Mega Mushroom, down takes the cap off (and interact) |
| R3 | | Parry; with LT held, the grab |
| RT | Middle mouse button | The hook (see above), with LT held it pulls the enemy to Mario. Then X on a small one: home run; RT on another: tether. On a medium one: tapped the zip-kick, held the vault. On a big one: the [Attack on Titan hook](#attack-on-titan-hook) (stick orbits, B gas, X strikes, right stick changes the bone, A after a hit re-anchors). Held as it catches a spot high up (or one ahead and above): the rope (stick pumps, A held climbs, B held lowers, RT up lets go with the swing's speed) |
| LT held (crouched or sliding) + X / Y / B / RB / LB | | Trap kit: banana, shell (hold Y: red), Bob-omb, spring, stopwatch. Thrown out of a slide or a run they carry his speed |
| LB | | Next weapon (fists, Star Sword, hammer, Chain Chomp, boomerang); hold to go back to the fists. Mid-string: a swap strike |
| LB + RB together | | [Star Power](#star-power) (with a Star earned at rank S); on, again: the STAR BLAST |
| LT | K | Crouch, ground pound in the air (not when the press goes with RT or R3: the pull, the grab). With the Star Sword in the air: the pogo (three a jump) |
| Right stick | Arrow keys | Lakitu's camera: left / right turn it smoothly round Mario (faster the further you push; the way Elden Ring's own camera turns: its System > Camera "reverse X axis" option flips it, on is SM64's old way), down zooms out, up zooms in (and into the first-person view). In a fight the camera backs off to keep Mario and the enemy in view, more the farther you've zoomed out |
| D-pad down | E | Interact: talk, pick up, graces, doors (on the pad it also ends the cap, the Rock or the Mega Mushroom). It presses the game's own interact button, Y; if you moved it in the game's controls, set `pad_interact = A` (B, Y, LB, RB, BACK, START) in `er_mario.ini` |
| L3 (left stick click) | R | A tap (on release): the [Mario Kart kart](#mario-kart-kart), in and out. Held 1.5 s (a green ring fills over Mario): the whistle for Yoshi. Co-op: held 0.6 s, warp to your partner. With `kart_test = on` or on Yoshi, the whistle as it's pressed |
| Menu > Inventory > Use | same | Any item used from the inventory (Seamless Co-op's Tiny Great Pot and Effigy of Malenia, too): the game gets its use-item request while the menu is up and a second after it closes. A button still held from confirming in a menu (or ending a talk) doesn't make Mario jump: it's ignored until let go |
| X / Y / RB / LT / LB on Yoshi | X: , or left mouse; Y: middle mouse; LT: K | Tongue, eggs, Mario's fireball, stomp, burst of speed |
| | F9 | Switch between Lakitu's and Elden Ring's camera |
| | F1 (hold 3 s) | Give up and respawn at the last grace |

## Luigi and co-op (experimental)

Play [Seamless Co-op](https://www.nexusmods.com/eldenring/mods/510) as Mario and Luigi, also on
top of [Elden Ring Reforged](https://www.nexusmods.com/eldenring/mods/541). Step-by-step setup
(in Spanish): [docs/coop/README.md](docs/coop/README.md).

- `character = luigi` in `er_mario.ini` makes you Luigi, in green and navy with an **L** on his
  cap, a touch slimmer than his brother (the next two launches rebuild the model). Mario wears the
  Vagabond chest piece, Luigi its Altered version, and every install also builds the other brother,
  so each player sees the other as who they play.
- Luigi plays like Luigi: his jumps go a fifth higher, his skids on a stop or a turn are longer
  (slippery shoes), holding **A** as a jump comes down makes his legs kick like mad and hold him up
  for a moment (his flutter, once a jump), low on health he stands there shivering instead of
  panting, and his voice is a little higher (on the partner's game too).
- Each player's Mario pose goes to the other over Steam (the game's own `steam_api64`,
  SteamNetworkingMessages on a channel of the mod's), ~30 times a second, with where they are (map
  chunk coordinates) and their clock. The other game shows them a moment back in time, between the
  two poses around then (snapshot interpolation, 50 ms or more if packets come unevenly), at their
  own place, not where the game's slower updates have their Tarnished: no stutter or lag.
- The partner's pose is put back right after the game animates their Tarnished, as your own is
  (it used to flicker into the Tarnished's shape now and then, their shadow too).
- Each one's Mario sounds (voice, steps, blows) go to the other too, always softer than your own
  and fading with the distance (none past 50 m), and so do the mod's own (the weapons' clinks and
  whooshes, the hammer's quakes, fireballs, explosions, power-ups), at their own volume; the
  player's own cues (buzzes, chimes) stay theirs.
- The partner's wing cap shows its wings (drawn over their model), the Vanish cap makes them
  see-through, the Mega Mushroom makes them big, and their Torrent is Yoshi on your game too
  (posed with Yoshi's own walk, idle and jump from how it moves, its hooves silent).
- Their effects show too, around where they're shown: fireballs, the hook's line, the boomerangs'
  streaks, the hammer's and the Chain Chomp's ribbons and sparks, the blows' star bursts, stars and
  coins, rings on the ground, the dash, Yoshi's tongue and eggs. They go in a packet of their own,
  20 times a second, at most 1000 bytes (the newest of each kind kept; fireballs go as what they
  are, about 20 bytes a ball, and are drawn in 3D on the other side): with them in the pose, a
  fight's poses outgrew one network packet and were lost whole, and the partner froze mid combo.
  The pose alone is 873 bytes. Every 20 s the log sums up what went out and came in
  (`net: last 20 s sent ...; got ...; lost poses N, effects M`).
- Their place is their own: their place in their map block (physics position less the game's
  world origin shift, its "chunk position": the same on both games, physics positions aren't),
  worked out through your own Tarnished in the same block, however far the game's slower copy of
  them lags (at a join it stands on the host). In another block, the game's copy shows them. That copy is then moved there too: Mario is drawn on its model, and the game
  leaves it out by where it stands (they went invisible), and your enemies see them there. A pose
  without a place holds where they were for half a second, and a change between their own place
  and the game's blends instead of jumping. The log says which one is used (`coop: partner ... shown at their own place ...`).
- **Revive**: out of health with a partner who can help, you don't die: you go down (SM64's death
  pose, stars circling over your head, untouchable) and have 30 s. Your partner comes over and
  holds **D-pad down** next to you for a second (a ▼ RESCUE prompt and a ring filling): you're
  back up with three wedges, SM64's 1-UP, a spin into the air, stars and coins. Nobody in time,
  both down, or a death the game deals itself (a fall into the void): the real death, Seamless
  Co-op's spectating and respawn as before.
- **Leash**: past 30 m a TOO FAR warning, from 35 m the way out gets stiffer until it stops at
  40 m (coming back is always free); more than 48 m apart for 2 s (a fall, a zip, Torrent), the
  one who isn't fighting (both or neither: the guest) warps to the other through SM64's pipe in a
  puff of smoke. Never in a boss fight, while either is down, riding, in a menu, a door or a
  cutscene. Holding **L3** (R) for 0.6 s warps you to your partner right away (stuck behind a
  wall, lost), under the same rules; a quick tap still whistles for Yoshi.
- Lakitu frames your partner close by too: the view leans their way and backs off a little (it
  never turns for them: you keep the camera).
- The host owns the enemies: the guest's blows are sent to the host, who takes them off and deals
  the final blow. As the guest you can't grab enemies or launch them (they'd jump back to where
  the host has them), but every blow counts.
- A blow on your partner only pushes them.
- Two players are a bit harder than one: Mario's blows (shares of max HP) act as if enemies had
  35% more health per extra player (bosses 100%, `coop_health_scaling` / `coop_boss_health_scaling`
  in `er_mario.ini`), and the guide sets enemies to hit
  15% harder. Alone, nothing changes.
- With Reforged, put `er_mario.dll`, `er_mario.ini` and your ROM in Reforged's `dll\offline`:
  the mod then builds its models into Reforged's `addons\er-mario`, which Reforged's launcher
  loads (`package_dir = ...` in `er_mario.ini` overrides it). `base_assets = ...` makes the build
  start from another mod's menu, material or Torrent files instead of the game's.
- On Reforged, enemies have about 1.7 times vanilla's health, so the line where an enemy counts as
  a boss (smaller damage share, never lifted by the uppercut) doubles to 5000 HP when Reforged's
  DLL is loaded (`enemy_hp_scale` in `er_mario.ini`). Reforged's one-off health drains no longer
  set Mario on fire, and invisible characters (the helpers by graces, enemies the game isn't
  drawing) are left alone.
- Mario's blows land on anyone not on your side, as a weapon's would, whatever team the game
  puts them on (Caelid's soldiers at war with its hounds, the giants with swords...), except the
  friends' teams, a captured enemy, spirit ashes and Torrents, and, on teams not known to be
  enemies, the human NPCs (merchants, quest NPCs) and those with a name of their own in the
  game's NpcParam (Blaidd, Alexander...; no field enemy has one). One of those who hits Mario is
  an enemy from then on (the log says `combat: ... hit Mario: it's hostile from now on`). `hostile_teams = 30,48` / `friendly_teams = ...` in
  `er_mario.ini` add exceptions, should a team come out wrong.
- Both players need the same ER Mario build, the same Seamless version and the same regulation
  (both Reforged, or both vanilla: `docs/coop/er-mario-coop.me3` is the vanilla profile).
- The Metal cap is retired (D-pad left is the Rock Mushroom), so its look isn't sent. Packets are
  version 10: both players need the same build.


Co-op touches (nothing of this shows alone):
- Landing on your partner's head is a **Bros. Jump** (a high bounce off it).
- **Bros. Attack** (Mario & Luigi's): three BROS! in a row and that enemy is marked, both
  brothers' faces over its head in a ring turning red and green, for 8 s. A Bros. Jump off your
  partner's head then (easier to land while it's up) flips you up and over onto it (12 m at most):
  the world slows at the top, you come down in a ground pound homing in on it, and it takes a
  team finisher's blow (about 30% of a regular enemy, a boss's own smaller share), with rings in
  both colours, stars and a shake. The one jumped off gets **BROS POWER** for 5 s: their blows up
  close hit 1.3 times harder. Then 25 s before the next one. `damage_bros_attack` in
  `er_mario.ini` sets its share.
- A boss down with your partner close by: both star dance with "SUPER BROS!", gold and both
  brothers' rings and a fountain of stars and coins.
- Reviving: a trail of stars from you to them, "NICE ONE!" on your side and "THANKS, BRO!" on
  theirs.
- Back together after over 8 s more than 30 m apart: a ring and stars at their feet and a soft
  chime.
- **Partner card**, bottom left, only while something's going on (a fight, either of you hurt,
  them low, down or over 30 m away), fading out 5 s after it's quiet: their head (Mario or Luigi),
  power pie, the power they're using (cap, Rock, Mega, Yoshi, a capture), coins and style rank. Red
  and pulsing when they're in danger (2 wedges or less, or just hurt), grey and crossed out when down.
- In danger and in view: their pie over their head. Out of view: an arrow on the screen's edge with
  how far they are (red and quicker in danger, grey when down).
- On the compass: their head (with the distance past 30 m) and their coin bag (a coin ringed in
  their colour).
- **BROS!** over an enemy you both hit within 1.5 s (style points); a streak of them counts as
  "BROS x N" under the style meter (the word only on the first and every fifth).
- Callouts: "LUIGI!" (low), "LUIGI DOWN", "LUIGI IS BACK" (or MARIO), "1UP" over the one you
  revive. Down yourself: HELP and your seconds left up top. A downed partner's edge arrow is green.
- Both players need the same build: the packets are version 10 (the status packet: health, coins,
  power, style, last enemy hit, coin bag, down and its seconds left, boss fight, busy, a few times
  a second; the effects; the revive; the mod's own sounds with their volume; the Bros. Attack's
  leap and blow; the fireballs as balls, bursts and lava pools, each list's records sized so a later
  build can add fields an older one skips; the elements' shots, poison clouds, statuses on enemies
  and weapon motes ride on those: a byte more per ball and three lists at the end). From version 4 on a build skips a
  kind of packet it doesn't know and reads a longer one as far as it knows, so later additions
  (grabbing and throwing your partner) can come without breaking it.

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

**The mod**
- **[Delta (deltarooo)](https://github.com/deltarooo)**, author of [ER Mario](https://github.com/deltarooo/er-mario). Almost everything here is their work.
- [BenjaminMassey](https://github.com/BenjaminMassey): controller direction fix and the Linux build script (upstream)
- Combat, weapons, kart and everything else new in this fork: [Luciano Esteban (LucioFex)](https://github.com/LucioFex)

**Nintendo.** Super Mario 64, Mario, Luigi, Yoshi, Lakitu, the Chain Chomp, Bullet Bill, the
Koopa shells, the Bob-omb, the power-ups and caps, Bowser, the Power Star, Mario Kart (the kart, its
drifts, mini-turbos, mushrooms, shells, bananas and the Thunder Cloud), Super Mario Odyssey's
capture and Cappy, Super Smash Bros.' super jump punch coins, Paper Mario's action commands and The
Legend of Zelda's Skyward Strike, Helm Splitter, spin attack, down-thrust, Mortal Draw and Breath of
the Wild's Flurry Rush are Nintendo's (characters, names, designs, sounds and music). Mario's model,
textures, animations, sounds and music are read at runtime from **your own** Super Mario 64 ROM:
none of them is in this repository or its releases. Made by a fan, for fun; not affiliated with or
endorsed by Nintendo.

**FromSoftware / Bandai Namco.** Elden Ring and everything in it are theirs; nothing of the game is
included here, the mod only runs inside your own copy.

**Sounds**
- The Star Sword's recorded swings and hits (optional files in `sounds/` of the mod's folder, never
  in this repository):
  [RPG Sound Pack](https://lpc.opengameart.org/node/5311) (OpenGameArt, CC0) and
  [20 sword sound effects (attacks and clashes)](https://opengameart.org/content/20-sword-sound-effects-attacks-and-clashes)
  by StarNinjas (OpenGameArt, CC0: the heavy swings and, from its clashes, the heavy hits).
  Thank you!
- Every other sound is Super Mario 64's (from your ROM) or synthesized by the mod itself
  (`src/sfx.rs`, `src/kartengine.rs`: the thunder, the hook's gas, the beam's hits, the kart's
  engine).

**Inspirations (moves and ideas, no assets).** Capcom's Devil May Cry (Dante's Rebellion: Stinger,
High Time, Million Stab, Helm Breaker, Aerial Rave, Drive; Vergil's Judgement Cut End; the style
rank), Sony Santa Monica's God of War (the Blades of Chaos), PlatinumGames' Bayonetta (the Dodge
Offset, witch time), FromSoftware's Sekiro (the Mikiri counter), Team Cherry's Hollow Knight (the
parry), Team Ico's Shadow of the Colossus (the climbing), Hajime Isayama's Attack on Titan (the 3D
maneuver gear), Akira Toriyama's Dragon Ball (the kamehameha), Blizzard's Overwatch (Doomfist's
rocket punch), Spider-Man and Bionic Commando (the rope), Fist of the North Star and JoJo's Bizarre
Adventure (the hundred-hand flurry), Psyonix's Rocket League (the kart's feel). Their names belong
to their owners.

**Code and tools**
- [libsm64](https://github.com/libsm64/libsm64) (CC0) by the libsm64 contributors, built on the
  [Super Mario 64 decompilation](https://github.com/n64decomp/sm64) by the n64decomp team
- [fromsoftware-rs](https://github.com/vswarte/fromsoftware-rs) (MIT / Apache-2.0) by Vincent
  Swarte and contributors
- [me3](https://me3.help) by the me3 team
- [hudhook](https://github.com/veeenu/hudhook) (MIT) by veeenu
- Rust crates (MIT / Apache-2.0): glam, cpal, ilhook, windows, num-bigint, aes, base64, texpresso,
  pelite, png, miniz_oxide
- The Grand Archives' Elden Ring cheat table, for the item and inventory function patterns

## License

[MIT](LICENSE), same as the original. It covers the mod's own code only, not Super Mario 64's or
Elden Ring's content: Mario's model, textures, sounds and animations come from the player's ROM,
and nothing of FromSoftware's is included. The `libsm64` folder keeps libsm64's own license, CC0
([libsm64/LICENSE.md](libsm64/LICENSE.md)).

<sub>Super Mario 64, Mario and every Nintendo character, name and sound are Nintendo's. Elden Ring is
FromSoftware's and Bandai Namco's. This is a free, non-commercial fan mod, not affiliated with or
endorsed by any of them.</sub>
