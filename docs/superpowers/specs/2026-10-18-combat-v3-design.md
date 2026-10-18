# Combat v3 — design (approved 2026-10-18)

Goal: take ER Mario's combat to GoW / DMC level while staying 100% Mario. Single player first;
co-op only gets two subtle touches. Fun over power: Mario must not become OP.

## Ground rules

- Weapons, items and thrown objects are **real 3D** (gear parts like the hammer). 2D sprites only
  for particles; prefer 3D effects (rings, ribbons, licks) where it is affordable.
- Sounds only from the SM64 ROM, soft when repeated (`sm64_play_sound_volume` ~0.1-0.3), varied.
- Enemies are never melted or stunlocked: cooldowns, per-target fades, bosses immune to cheap
  tricks, ragdoll launches never kill by themselves. Enemy damage unchanged.
- Vanilla Elden Ring, solo Reforged and Seamless co-op keep working. Every batch: build, install
  (C:\ER-Mario and `<ERR>\dll\offline`), clear log lines to verify, review subagent.
- Never fight with or move Mario in the user's save without asking; ask for a click when the game
  window needs focus.
- Feel free to correct an idea below if the code or the game shows a better way; note why.

## Controls changes

- **LT held on the ground (crouching, not running) + button = trap kit.** Running LT+X stays the
  SM64 slide kick; LT in the air stays ground pound; LT+A long jump/backflip unchanged.
- **D-pad left: metal cap removed** (user dislikes the invulnerability; Mega already covers
  "powered-up"). Replace with a new power-up that surprises visually, is very fun and **uses the
  map's properties** (terrain, height, water, walls, physics props...). To be designed at the start
  of the implementation session (propose 2-3 options, pick the best, explain).
- R3: Star Rush (batch 7). Everything else as today.

## Features (numbered as in the approved list)

### A. Trap kit (LT + button, 3D items, cooldowns)
1. **Banana** (LT+X): commons stepping on it slip (short ragdoll / fall, ~2 s down), mediums
   stagger, bosses ignore it. Max 2 on the floor, ~10 s cooldown.
2. **Green shell** (LT+Y, user's idea): kicked along the floor, bounces off walls and enemies,
   gone after **4 hits**; each hit damages and knocks commons down. Hold Y = **red shell**: fewer
   bounces, homes on the nearest enemy.
3. **Bob-omb** (LT+B): thrown, bounces/rolls, fuse ~2 s, AoE blast (Combat::blast), flings smalls.
4. **Spring** (LT+RB): planted; an enemy stepping on it is launched up ready for a juggle; Mario
   gets a super jump.
   (POW block dropped: overlaps ground pound / Mega.) Existing B,Y Mario-shell slide stays.
   Risk: chest vertex buffer nearly full (754 Mario + 396 gear of 1184): items likely go on another
   armour piece (gloves/legs row) — verify first.

### B. Hook (RT)
5. **Home run**: X while a small enemy is being pulled bats it toward the camera direction;
   it bowls over enemies it hits.
6. **Tether** (Just Cause): with one enemy hooked, RT on another slams both together (stun).
7. **Pole swing** (SM64 poles): hold RT on an enemy = Mario swings round it; release = flying kick
   with the momentum.
8. **Boss ride** (Monster Hunter wirebug): hook a big one, hold RT and steer; slammed into a wall
   it is left open. Long cooldown.

### C. Timing and defence
9. **Mikiri stomp** (Sekiro): A just before an enemy blow lands (dodge.rs detection) = Mario lands
   on the weapon/head, enemy staggered, posture build-up toward finishers. Early = plain jump.
10. **Action command** (Paper Mario): X on the impact frame = "Nice!", more damage + style.
11. **Dash Bonk** (Hades): X at the end of the dash = thrust.

### D. Combos
12. **Directional variants** (DMC): back + X = launcher for every weapon; forward + Y = gap-closer.
13. **MAX finishers**: 12+ hits without taking damage -> weapon glows, next finisher is MAX. Hit =
    streak lost. **Chomp Cyclone MAX** (user's idea): wide spin that flings smalls and some
    mediums in semi-ragdoll (they get up). Hammer: cross-shaped quake. Boomerang: three at once.
    Fists: flurry + uppercut.
14. **Fluidity and look & feel (rigorous)**: smooth blends between moves (no pops), anticipation
    and follow-through, squash & stretch, hit freeze + camera punch on finishers, per-weapon trail
    colour, enemy flash on impact. **Chomp spin on held Y**: keep it (user liked it) but give it
    the *Blades of Exile* feel — wide chain arcs with momentum, fire ribbon along the chain, chain
    clinks, wind-up and recovery, the ball trailing behind Mario's hand. Review every existing move
    for stiffness/pops and fix what feels off.

### E. Systems
15. **Skillshots** (Bulletstorm): named style bonuses for creative kills: "Slip!", "Strike!",
    "Home Run", "Carom" (shell knocks 3)...
16. **Freeflow** (Arkham): B toward another enemy mid-combo = leap to it, string kept.
17. **Star Rush** (DMC Devil Trigger): style fills a star meter; R3 = 10 s super armour, a bit
    faster, star sparks, power-up music quiet.
18. **Stopwatch** (BotW Stasis + SMB2): freeze an enemy 3 s, blows store momentum, it flies off
    when time resumes. Riskiest (needs a way to freeze enemy animation).

### F. Co-op (subtle)
19. **Bros. Jump**: landing on the partner's head = high bounce (local only, no new packets).
20. **Partner HUD**: when a second player is connected, a small thematic readout of the partner
    (Luigi/Mario head icon + SM64-style power pie or small bar with their HP, maybe distance arrow
    when off screen). Small, unobtrusive, hidden alone. Needs partner HP in the pose packet (bump
    packet VERSION; old peers must not crash).

### G. Balance and edge cases (every batch + a final pass)
21. For each feature, walk the cases: enemy types (commons, shielded, mediums, big bodies,
    flying, mounted, packs, invisible/helpers, NPCs/friendlies), bosses (multi-phase, huge,
    grab bosses, Reforged HP scaling), terrain (slopes, ledges, water, narrow rooms, stairs, lethal
    falls), states (Mega, Yoshi, hooked, grabbed, status ailments, slow-mo, menus/interact,
    death mid-move), solo vs co-op (host/guest guards, Seamless scaling). Nerf/buff numbers so
    every tool has a use and none trivializes a fight; log the tuning decisions in the spec.
    Single player is the main mode; co-op only has to stay correct.

### H. Quality of life
22. **Shorter game over**: the wait from Mario's death to Bowser's face and the restart is too
    long (gameover.rs): trim it a bit, keep the sequence readable.

## Decisions taken while implementing

### D-pad left: the Rock Mushroom (batch 3)
Options weighed (all had to use the map, be flashy and fun, and not be another "invulnerable"):
1. **Rock Mushroom** (Super Mario Galaxy 2): Mario turns into a rolling boulder with his head
   peeking out. The terrain drives it: downhill he picks up speed from the real slope under him,
   uphill he slows, walls bounce him back (each bounce a "carom"), drops turn into a slam when he
   lands, breakable props shatter, regular enemies are bowled over (the game's knockdown), bosses
   and big bodies bounce him off. A hops, X rams forward (a burst, cooldown).
2. **Cloud Flower** (SMG2): A in the air leaves a cloud to stand on (3 at most) to reach roofs and
   cliffs and come down on enemies from high up. Uses height only; standing out of reach of a
   boss would cheese it and confuse its AI.
3. **Propeller Mushroom** (NSMB Wii): spin 10 m up from anywhere, glide down, drill dive. Uses
   height only and overlaps the Wing Cap (D-pad up) and the new spring.
**Picked: the Rock Mushroom.** It is the one that really plays with the Lands Between (its hills,
slopes, ruins' walls, crates and barrels), it reads at a glance, it is funny (a Mario boulder
bowling soldiers over), and it is fair: it never makes Mario invulnerable (hits cost half and don't
knock him out of it), it only knocks regular enemies down (no kills by itself), bosses take chip
damage and bounce him away. 12 s, back after 75 s; no weapons or moves while rolling.
The Metal Cap is gone from D-pad left (its code stays dormant: nothing puts it on any more).

### Trap kit notes (batch 2)
- The kit's meshes would not fit the chest piece (Mario + gear use 1150 of its 1184 vertices):
  they live in the gauntlets' piece (2405 vertices; the kit takes 942) on 7 leaf finger bones,
  with their own palette albedo and copies of the gauntlets' materials (AM_M_0999).
- The Bob-omb and the spring were built in batch 2 with the rest of the kit (same code, one asset
  rebuild instead of two); batch 3 was the D-pad-left power-up.
- Slips and shell hits use the game's own knockdown: the sweep's attack row gets a knockdown hit
  reaction for a few ticks (`knockdown_level` in er_mario.ini if the guess, 4, is wrong).
- The Bob-omb's blast sends regulars up (juggle), never kills by itself (spec rule), and goes off
  on the first enemy it touches.

## Batches

1. 9, 10, 11, 19 (timing) + 20 (partner HUD)
2. Vertex-budget check, then 1, 2 (banana, shell)
3. 3, 4 (Bob-omb, spring) + new D-pad-left power-up
4. 5, 6, 7 (hook)
5. 12, 13, 14 (combos, MAX, fluidity incl. Blades-of-Exile Chomp)
6. 15, 16 (skillshots, freeflow)
7. 17, 18, 8 (Star Rush, stopwatch, boss ride)

End: build, install in C:\ER-Mario and Reforged, commit + push with README updated, upload the DLL
to pre-release `coop-2026-10-17` (ER-Mario-coop.zip, `--clobber`, new SHA256 in the notes),
update memory, summary of what to test + prompt for the partner's notebook.
