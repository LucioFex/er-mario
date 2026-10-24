//! The effects hud.rs draws in the world around a Mario (his fireballs, the hook's line, the
//! boomerangs' streaks, the hammer's and the Chain Chomp's ribbons, the blows' star bursts, stars
//! and coins, the rings on the ground, the dash's streaks, Yoshi's tongue and eggs), gathered in
//! one place: hud.rs draws ours and each co-op partner's alike, and coop.rs sends ours in a packet
//! of their own (positions around the Tarnished, so the other game puts them around where it shows
//! us).

use glam::Vec3;

/// A fireball (fireball.rs): a shot, or the charge in Mario's hand. flame.rs gives it its looks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Orb {
    pub pos: Vec3,
    /// m/s (to move it between ticks; its tail streams the other way)
    pub vel: Vec3,
    /// across (m)
    pub size: f32,
    pub level: u8,
    /// ticks since it left his hand (the charge: since it began)
    pub age: u32,
    /// ORB_CHARGE, ORB_STUCK, ORB_REFLECTED
    pub flags: u8,
    /// the ground under it (Elden Ring height), if near
    pub ground: Option<f32>,
    /// how much of it the camera sees past the map (0-1; a partner's: 1)
    pub vis: f32,
    /// what it is (element.rs Element): a fireball, the ice block, the dark orb, the poison glob
    pub element: u8,
}

pub const ORB_CHARGE: u8 = 1;
pub const ORB_STUCK: u8 = 2;
pub const ORB_REFLECTED: u8 = 4;

/// A fireball's burst: where, its level, ticks since, its shot's size (m), the ground under it,
/// BLAST_LAUNCH
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blast {
    pub pos: Vec3,
    pub level: u8,
    pub age: u32,
    pub size: f32,
    pub ground: Option<f32>,
    pub flags: u8,
    pub vis: f32,
    pub element: u8,
}

/// the flames at Mario's hand as a big one leaves it: no explosion of its own
pub const BLAST_LAUNCH: u8 = 1;

/// A pool of lava the sun left: ticks since, how long it lasts, its radius (m)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pool {
    pub pos: Vec3,
    pub age: u32,
    pub life: u32,
    pub radius: f32,
    pub vis: f32,
}

/// The poison glob's cloud on the ground (fireball.rs): ticks since, how long it lasts, its radius
/// (m)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cloud {
    pub pos: Vec3,
    pub age: u32,
    pub life: u32,
    pub radius: f32,
    pub vis: f32,
}

/// A status on an enemy (element.rs): its feet, its height and girth (m), the element, ticks since
/// it went off (AURA_LONG ago: long enough for its burst to be over), how much of it is left (0-1:
/// frostbite's, poison's time; 0 for a burst alone)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aura {
    pub pos: Vec3,
    pub height: f32,
    pub radius: f32,
    pub element: u8,
    pub age: u32,
    pub left: f32,
}

/// (an aura's age once its burst is long over)
pub const AURA_LONG: u32 = 255;

/// A mote of a weapon's element (element.rs: the weapon taken, its element round it now and then):
/// where, the element, how far through its life (0-1), its size (m)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mote {
    pub pos: Vec3,
    pub element: u8,
    pub t: f32,
    pub size: f32,
}

/// Mario's fireballs and the other weapons' shots: their shots and charge, the bursts, the lava
/// pools, the poison clouds; the statuses on enemies and the weapons' motes (element.rs)
#[derive(Clone, Default, Debug, PartialEq)]
pub struct FireFx {
    pub orbs: Vec<Orb>,
    pub blasts: Vec<Blast>,
    pub pools: Vec<Pool>,
    pub clouds: Vec<Cloud>,
    pub auras: Vec<Aura>,
    pub motes: Vec<Mote>,
}

impl FireFx {
    pub const EMPTY: FireFx = FireFx { orbs: Vec::new(), blasts: Vec::new(), pools: Vec::new(), clouds: Vec::new(), auras: Vec::new(), motes: Vec::new() };

    pub fn is_empty(&self) -> bool {
        self.orbs.is_empty() && self.blasts.is_empty() && self.pools.is_empty() && self.clouds.is_empty() && self.auras.is_empty() && self.motes.is_empty()
    }
}

/// A bolt of the sword's lightning (bolt.rs): its ends, its zigzag's seed, how far through its life
/// (0-1), how thick (m)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bolt {
    pub from: Vec3,
    pub to: Vec3,
    pub seed: u16,
    pub t: f32,
    pub width: f32,
}

/// The storm cloud (bolt.rs): where, how wide (m), how far through its life (0-1), lit from inside
/// by a bolt (0-1)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Storm {
    pub pos: Vec3,
    pub radius: f32,
    pub t: f32,
    pub flash: f32,
}

/// A crescent of the sword's (wave.rs): where its middle is, the way it goes (yaw, radians), its
/// slant round that way (degrees, 0 flat), how wide (m), how much of it shows (0-1), which (0 the
/// star wave, 1 a JUST one, 2 the sword beam)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crescent {
    pub pos: Vec3,
    pub yaw: f32,
    pub roll: f32,
    pub width: f32,
    pub alpha: f32,
    pub kind: u8,
}

/// A small hammer of the Hammer Bros. volley in flight (volley.rs): where, its flight's yaw
/// (radians), its turn end over end (radians), its size (m)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thrown {
    pub pos: Vec3,
    pub yaw: f32,
    pub spin: f32,
    pub size: f32,
}

/// A star burst where a blow landed (`t` 0-1 through its life)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spark {
    pub pos: Vec3,
    pub size: f32,
    pub tint: [f32; 4],
    pub t: f32,
}

/// A ring widening on the ground
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ring {
    pub pos: Vec3,
    pub radius: f32,
    pub tint: [f32; 4],
    pub t: f32,
}

/// A chunk of rubble a heavy blow threw up (impact.rs): where, how it's turned, its size (m), a
/// seed for its shape. Ours only: a partner's blows throw theirs on their own screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rock {
    pub pos: Vec3,
    pub rot: glam::Quat,
    pub size: f32,
    pub seed: u32,
}

/// A star or a coin flying off a blow
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bit {
    pub coin: bool,
    pub pos: Vec3,
    pub size: f32,
    pub t: f32,
    pub age: f32,
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct WorldFx {
    pub fire: FireFx,
    /// enemies flying through the air: points along each one's path and how old (0-1)
    pub streaks: Vec<Vec<(Vec3, f32)>>,
    /// each boomerang's streak: its points up to it, and its size
    pub rangs: Vec<(Vec<Vec3>, f32)>,
    /// Yoshi: his tongue (mouth, tip), its fires, the eggs (where, how old: s)
    pub tongue: Option<(Vec3, Vec3)>,
    pub yoshi_fires: Vec<(Vec3, f32)>,
    pub eggs: Vec<(Vec3, f32)>,
    /// the hook: the Chomp's chain (hand, head), or the cap's line (hand, head, spin)
    pub chain: Option<(Vec3, Vec3)>,
    pub hook: Option<(Vec3, Vec3, f32)>,
    pub sparks: Vec<Spark>,
    pub dash: Vec<(Vec3, f32)>,
    pub puffs: Vec<(Vec3, f32)>,
    /// the hammer's and the Chomp's ribbons (handle or hand, head, how old 0-1), the sparks off the
    /// chain (now, a moment ago, how old)
    pub swing: Vec<(Vec3, Vec3, f32)>,
    pub chomp: Vec<(Vec3, Vec3, f32)>,
    pub flying: Vec<(Vec3, Vec3, f32)>,
    pub rings: Vec<Ring>,
    pub bits: Vec<Bit>,
    /// (not sent: encode leaves it out)
    pub rubble: Vec<Rock>,
    /// the sword's lightning (bolt.rs) and crescents (wave.rs)
    pub bolts: Vec<Bolt>,
    pub storms: Vec<Storm>,
    pub crescents: Vec<Crescent>,
    /// the sword's cut (tip, low on the blade, how faded), as the hammer's `swing`
    pub blade: Vec<(Vec3, Vec3, f32)>,
    /// the hammer's thrown small ones (volley.rs)
    pub thrown: Vec<Thrown>,
}

/// at most this many of each kind go out (the newest), and points per trail; of the statuses on
/// enemies and the weapons' motes, fewer
const MAX: usize = 24;
const AURAS_MAX: usize = 8;
const MOTES_MAX: usize = 10;
const BOLTS_MAX: usize = 12;
const CRESCENTS_MAX: usize = 6;

impl WorldFx {
    /// Ours now (Elden Ring world positions).
    pub fn local() -> Self {
        let (rangs, _) = crate::boomerang::sprites();
        let ride = crate::ride::sprites();
        let chomp_hook = crate::chomp::hooking();
        let mut fire = crate::fireball::looks();
        fire.auras = crate::element::auras();
        fire.motes = crate::element::motes();
        fire.motes.extend(crate::light::motes());
        WorldFx {
            fire,
            streaks: crate::impact::enemy_streaks(),
            rangs: rangs.into_iter().map(|r| (r.trail.iter().copied().chain(std::iter::once(r.pos)).collect(), r.size)).collect(),
            tongue: ride.tongue,
            yoshi_fires: ride.fires,
            eggs: ride.eggs,
            chain: crate::hook::live().filter(|_| chomp_hook).map(|h| (h.hand, h.head)),
            // (the rope thrown from on a colossus has no hook of hook.rs's: its line all the same)
            hook: crate::hook::sprite().filter(|_| !chomp_hook).map(|h| (h.hand, h.head, h.spin)).or_else(|| crate::colossus::rope_line().map(|(hand, q)| (hand, q, crate::flame::clock() * 27.0))),
            sparks: crate::impact::sparks().into_iter().map(|(s, t)| Spark { pos: s.pos, size: s.size, tint: s.tint, t }).collect(),
            dash: crate::impact::dash_trail(),
            puffs: crate::impact::dash_puffs(),
            swing: crate::impact::swing_trail(),
            chomp: crate::impact::chomp_trail(),
            flying: crate::impact::flying_sparks(),
            rings: crate::impact::rings().into_iter().map(|(r, t)| Ring { pos: r.pos, radius: r.radius, tint: r.tint, t }).collect(),
            bits: crate::impact::bits().into_iter().map(|(b, pos, size, t, age)| Bit { coin: matches!(b, crate::impact::Bit::Coin), pos, size, t, age }).collect(),
            rubble: crate::impact::rubble_chunks(),
            bolts: crate::bolt::looks(),
            storms: crate::bolt::storm_looks(),
            crescents: crate::wave::looks(),
            blade: crate::impact::blade_trail(),
            thrown: crate::volley::looks(),
        }
    }

    pub fn is_empty(&self) -> bool {
        *self == WorldFx::default()
    }

    /// Every position moved by `d`.
    pub fn shifted(mut self, d: Vec3) -> Self {
        for o in &mut self.fire.orbs {
            o.pos += d;
            o.ground = o.ground.map(|g| g + d.y);
        }
        for b in &mut self.fire.blasts {
            b.pos += d;
            b.ground = b.ground.map(|g| g + d.y);
        }
        self.fire.pools.iter_mut().for_each(|p| p.pos += d);
        self.fire.clouds.iter_mut().for_each(|c| c.pos += d);
        self.fire.auras.iter_mut().for_each(|a| a.pos += d);
        self.fire.motes.iter_mut().for_each(|m| m.pos += d);
        for b in &mut self.bolts {
            b.from += d;
            b.to += d;
        }
        self.storms.iter_mut().for_each(|s| s.pos += d);
        self.crescents.iter_mut().for_each(|c| c.pos += d);
        for b in &mut self.blade {
            b.0 += d;
            b.1 += d;
        }
        self.thrown.iter_mut().for_each(|t| t.pos += d);
        for s in &mut self.streaks {
            s.iter_mut().for_each(|p| p.0 += d);
        }
        for (pts, _) in &mut self.rangs {
            pts.iter_mut().for_each(|p| *p += d);
        }
        self.tongue = self.tongue.map(|(a, b)| (a + d, b + d));
        self.yoshi_fires.iter_mut().for_each(|p| p.0 += d);
        self.eggs.iter_mut().for_each(|p| p.0 += d);
        self.chain = self.chain.map(|(a, b)| (a + d, b + d));
        self.hook = self.hook.map(|(a, b, s)| (a + d, b + d, s));
        self.sparks.iter_mut().for_each(|s| s.pos += d);
        self.dash.iter_mut().for_each(|p| p.0 += d);
        self.puffs.iter_mut().for_each(|p| p.0 += d);
        for list in [&mut self.swing, &mut self.chomp, &mut self.flying] {
            list.iter_mut().for_each(|p| {
                p.0 += d;
                p.1 += d;
            });
        }
        self.rings.iter_mut().for_each(|r| r.pos += d);
        self.bits.iter_mut().for_each(|b| b.pos += d);
        self.rubble.iter_mut().for_each(|r| r.pos += d);
        self
    }

    /// Ours for the partner (positions around `here`, our Tarnished), appended to `out`.
    pub fn encode(&self, here: Vec3, out: &mut Vec<u8>) {
        self.encode_most(here, out, MAX);
    }

    /// As `encode`, in at most `budget` bytes: fewer of each kind (the newest), down to none.
    pub fn encode_fit(&self, here: Vec3, out: &mut Vec<u8>, budget: usize) {
        let start = out.len();
        for most in [MAX, 12, 6, 3, 1, 0] {
            out.truncate(start);
            self.encode_most(here, out, most);
            if out.len() - start <= budget {
                return;
            }
        }
    }

    /// At most `most` of each kind and points per trail (the newest).
    fn encode_most(&self, here: Vec3, out: &mut Vec<u8>, most: usize) {
        let mut w = Writer { out, here, most };
        // (the fireballs as what they are, flame.rs draws them on the other side: a few bytes a
        // ball; records sized, so later fields can be added and older readers skip them)
        w.records(&self.fire.orbs, |w, o| {
            w.pos(o.pos);
            w.vel(o.vel);
            w.age(o.size);
            w.out.push(o.level);
            w.ticks(o.age);
            w.out.push(o.flags);
            w.drop_to(o.pos, o.ground);
            // (added with the elements: an older reader skips it and draws a fireball)
            w.out.push(o.element);
        });
        w.records(&self.fire.blasts, |w, b| {
            w.pos(b.pos);
            w.out.push(b.level);
            w.ticks(b.age);
            w.age(b.size);
            w.drop_to(b.pos, b.ground);
            w.out.push(b.flags);
            w.out.push(b.element);
        });
        w.records(&self.fire.pools, |w, p| {
            w.pos(p.pos);
            w.ticks(p.age);
            w.ticks(p.life);
            w.age(p.radius);
        });
        w.list(&self.streaks, |w, s| w.list(s, |w, p| {
            w.pos(p.0);
            w.unit(p.1);
        }));
        w.list(&self.rangs, |w, (pts, size)| {
            w.list(pts, |w, p| w.pos(*p));
            w.unit(size / 8.0);
        });
        w.opt(&self.tongue, |w, (a, b)| {
            w.pos(*a);
            w.pos(*b);
        });
        w.list(&self.yoshi_fires, |w, p| {
            w.pos(p.0);
            w.age(p.1);
        });
        w.list(&self.eggs, |w, p| {
            w.pos(p.0);
            w.age(p.1);
        });
        w.opt(&self.chain, |w, (a, b)| {
            w.pos(*a);
            w.pos(*b);
        });
        w.opt(&self.hook, |w, (a, b, s)| {
            w.pos(*a);
            w.pos(*b);
            w.angle(*s);
        });
        w.list(&self.sparks, |w, s| {
            w.pos(s.pos);
            w.unit(s.size / 8.0);
            w.tint(s.tint);
            w.unit(s.t);
        });
        for list in [&self.dash, &self.puffs] {
            w.list(list, |w, p| {
                w.pos(p.0);
                w.unit(p.1);
            });
        }
        for list in [&self.swing, &self.chomp, &self.flying] {
            w.list(list, |w, p| {
                w.pos(p.0);
                w.pos(p.1);
                w.unit(p.2);
            });
        }
        w.list(&self.rings, |w, r| {
            w.pos(r.pos);
            w.unit(r.radius / 32.0);
            w.tint(r.tint);
            w.unit(r.t);
        });
        w.list(&self.bits, |w, b| {
            w.out.push(b.coin as u8);
            w.pos(b.pos);
            w.unit(b.size / 8.0);
            w.unit(b.t);
            w.age(b.age);
        });
        // (added with the elements, at the end: an older reader stops before them; the statuses and
        // the motes, only looks, fewer of them, so a busy fight's packet keeps the rest)
        let most = w.most;
        w.records(&self.fire.clouds, |w, c| {
            w.pos(c.pos);
            w.ticks(c.age);
            w.ticks(c.life);
            w.age(c.radius);
        });
        w.most = most.min(AURAS_MAX);
        w.records(&self.fire.auras, |w, a| {
            w.pos(a.pos);
            w.unit(a.height / 16.0);
            w.unit(a.radius / 8.0);
            w.out.push(a.element);
            w.out.push(a.age.min(AURA_LONG) as u8);
            w.unit(a.left);
        });
        w.most = most.min(MOTES_MAX);
        w.records(&self.fire.motes, |w, m| {
            w.pos(m.pos);
            w.out.push(m.element);
            w.unit(m.t);
            w.unit(m.size);
        });
        // (added with the Star Sword, at the end again)
        w.most = most.min(BOLTS_MAX);
        w.records(&self.bolts, |w, b| {
            w.pos(b.from);
            w.pos(b.to);
            w.out.extend_from_slice(&b.seed.to_le_bytes());
            w.unit(b.t);
            w.unit(b.width);
        });
        w.most = most.min(2);
        w.records(&self.storms, |w, s| {
            w.pos(s.pos);
            w.unit(s.radius / 16.0);
            w.unit(s.t);
            w.unit(s.flash);
        });
        w.most = most.min(CRESCENTS_MAX);
        w.records(&self.crescents, |w, c| {
            w.pos(c.pos);
            w.angle(c.yaw);
            w.unit((c.roll + 90.0) / 180.0);
            w.unit(c.width / 8.0);
            w.unit(c.alpha);
            w.out.push(c.kind);
        });
        w.most = most;
        w.records(&self.blade, |w, b| {
            w.pos(b.0);
            w.pos(b.1);
            w.unit(b.2);
        });
        // (added with the weapons' new moves, Part 3, at the end again)
        w.most = most.min(6);
        w.records(&self.thrown, |w, t| {
            w.pos(t.pos);
            w.angle(t.yaw);
            w.angle(t.spin);
            w.unit(t.size);
        });
    }

    /// The partner's from `b` (positions around their Tarnished: `shifted` puts them where we show
    /// them). None if it doesn't read.
    pub fn decode(b: &[u8]) -> Option<WorldFx> {
        let mut r = Reader { b, at: 0, end: b.len() };
        let orbs = r.records(|r| {
            let pos = r.pos()?;
            let (vel, size, level, age, flags, ground) = (r.vel()?, r.age()?, r.u8()?, r.ticks()?, r.u8()?, r.drop_to(pos)?);
            // (an older sender's: a fireball)
            Some(Orb { pos, vel, size, level, age, flags, ground, vis: 1.0, element: r.u8_or(0) })
        })?;
        let blasts = r.records(|r| {
            let pos = r.pos()?;
            let (level, age, size, ground, flags) = (r.u8()?, r.ticks()?, r.age()?, r.drop_to(pos)?, r.u8()?);
            Some(Blast { pos, level, age, size, ground, flags, vis: 1.0, element: r.u8_or(0) })
        })?;
        let pools = r.records(|r| Some(Pool { pos: r.pos()?, age: r.ticks()?, life: r.ticks()?, radius: r.age()?, vis: 1.0 }))?;
        let mut fx = WorldFx { fire: FireFx { orbs, blasts, pools, ..Default::default() }, ..Default::default() };
        fx.streaks = r.list(|r| r.list(|r| Some((r.pos()?, r.unit()?))))?;
        fx.rangs = r.list(|r| Some((r.list(|r| r.pos())?, r.unit()? * 8.0)))?;
        fx.tongue = r.opt(|r| Some((r.pos()?, r.pos()?)))?;
        fx.yoshi_fires = r.list(|r| Some((r.pos()?, r.age()?)))?;
        fx.eggs = r.list(|r| Some((r.pos()?, r.age()?)))?;
        fx.chain = r.opt(|r| Some((r.pos()?, r.pos()?)))?;
        fx.hook = r.opt(|r| Some((r.pos()?, r.pos()?, r.angle()?)))?;
        fx.sparks = r.list(|r| Some(Spark { pos: r.pos()?, size: r.unit()? * 8.0, tint: r.tint()?, t: r.unit()? }))?;
        fx.dash = r.list(|r| Some((r.pos()?, r.unit()?)))?;
        fx.puffs = r.list(|r| Some((r.pos()?, r.unit()?)))?;
        fx.swing = r.list(|r| Some((r.pos()?, r.pos()?, r.unit()?)))?;
        fx.chomp = r.list(|r| Some((r.pos()?, r.pos()?, r.unit()?)))?;
        fx.flying = r.list(|r| Some((r.pos()?, r.pos()?, r.unit()?)))?;
        fx.rings = r.list(|r| Some(Ring { pos: r.pos()?, radius: r.unit()? * 32.0, tint: r.tint()?, t: r.unit()? }))?;
        fx.bits = r.list(|r| Some(Bit { coin: r.u8()? == 1, pos: r.pos()?, size: r.unit()? * 8.0, t: r.unit()?, age: r.age()? }))?;
        // (an older sender's packet ends here)
        if r.at < r.b.len() {
            fx.fire.clouds = r.records(|r| Some(Cloud { pos: r.pos()?, age: r.ticks()?, life: r.ticks()?, radius: r.age()?, vis: 1.0 }))?;
            fx.fire.auras = r.records(|r| Some(Aura { pos: r.pos()?, height: r.unit()? * 16.0, radius: r.unit()? * 8.0, element: r.u8()?, age: r.u8()? as u32, left: r.unit()? }))?;
            fx.fire.motes = r.records(|r| Some(Mote { pos: r.pos()?, element: r.u8()?, t: r.unit()?, size: r.unit()? }))?;
        }
        // (a sender from before the Star Sword: none)
        if r.at < r.b.len() {
            fx.bolts = r.records(|r| Some(Bolt { from: r.pos()?, to: r.pos()?, seed: u16::from(r.u8()?) | u16::from(r.u8()?) << 8, t: r.unit()?, width: r.unit()? }))?;
            fx.storms = r.records(|r| Some(Storm { pos: r.pos()?, radius: r.unit()? * 16.0, t: r.unit()?, flash: r.unit()? }))?;
        }
        if r.at < r.b.len() {
            fx.crescents =
                r.records(|r| Some(Crescent { pos: r.pos()?, yaw: r.angle()?, roll: r.unit()? * 180.0 - 90.0, width: r.unit()? * 8.0, alpha: r.unit()?, kind: r.u8()? }))?;
        }
        if r.at < r.b.len() {
            fx.blade = r.records(|r| Some((r.pos()?, r.pos()?, r.unit()?)))?;
        }
        if r.at < r.b.len() {
            fx.thrown = r.records(|r| Some(Thrown { pos: r.pos()?, yaw: r.angle()?, spin: r.angle()?, size: r.unit()? }))?;
        }
        Some(fx)
    }
}

/// Positions as cm around a point (i16: 327 m each way), shares 0-1 as u8, ages as u16 ms.
struct Writer<'a> {
    out: &'a mut Vec<u8>,
    here: Vec3,
    most: usize,
}

impl Writer<'_> {
    fn pos(&mut self, p: Vec3) {
        let d = (p - self.here) * 100.0;
        for v in [d.x, d.y, d.z] {
            let v = if v.is_finite() { v.round().clamp(i16::MIN as f32, i16::MAX as f32) } else { 0.0 };
            self.out.extend_from_slice(&(v as i16).to_le_bytes());
        }
    }
    fn unit(&mut self, v: f32) {
        self.out.push((v.clamp(0.0, 1.0) * 255.0).round() as u8);
    }
    fn age(&mut self, s: f32) {
        self.out.extend_from_slice(&((s.max(0.0) * 1000.0).min(u16::MAX as f32) as u16).to_le_bytes());
    }
    /// a velocity (m/s, to 3276 each way, in 0.1)
    fn vel(&mut self, v: Vec3) {
        for c in [v.x, v.y, v.z] {
            let c = if c.is_finite() { (c * 10.0).round().clamp(i16::MIN as f32, i16::MAX as f32) } else { 0.0 };
            self.out.extend_from_slice(&(c as i16).to_le_bytes());
        }
    }
    /// (an age in ticks: wraps after 36 min, which only turns a flame's flicker)
    fn ticks(&mut self, t: u32) {
        self.out.extend_from_slice(&(t as u16).to_le_bytes());
    }
    /// how far below `p` the ground is (to 31.75 m in 1/8; 255: none near)
    fn drop_to(&mut self, p: Vec3, ground: Option<f32>) {
        self.out.push(ground.map_or(255, |g| ((p.y - g).max(0.0) * 8.0).round().min(254.0) as u8));
    }
    /// A list whose records all take the same bytes, that size after the count: a reader that
    /// knows fewer of their fields skips the rest (the newest `most`, as `list`)
    fn records<T>(&mut self, items: &[T], f: impl Fn(&mut Self, &T)) {
        let from = items.len().saturating_sub(self.most);
        self.out.push((items.len() - from) as u8);
        let size_at = self.out.len();
        self.out.push(0);
        let start = self.out.len();
        for it in &items[from..] {
            f(self, it);
        }
        let n = items.len() - from;
        if n > 0 {
            self.out[size_at] = ((self.out.len() - start) / n).min(255) as u8;
        }
    }
    fn angle(&mut self, a: f32) {
        self.out.extend_from_slice(&((a.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU * 65535.0) as u16).to_le_bytes());
    }
    fn tint(&mut self, t: [f32; 4]) {
        for v in t {
            self.unit(v);
        }
    }
    /// (the newest `most`: the lists go oldest first)
    fn list<T>(&mut self, items: &[T], f: impl Fn(&mut Self, &T)) {
        let from = items.len().saturating_sub(self.most);
        self.out.push((items.len() - from) as u8);
        for it in &items[from..] {
            f(self, it);
        }
    }
    fn opt<T>(&mut self, item: &Option<T>, f: impl Fn(&mut Self, &T)) {
        self.out.push(item.is_some() as u8);
        if let Some(it) = item {
            f(self, it);
        }
    }
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
    /// where the record being read ends (`records`), else the end
    end: usize,
}

impl Reader<'_> {
    fn u8(&mut self) -> Option<u8> {
        let v = *self.b.get(self.at)?;
        self.at += 1;
        Some(v)
    }
    /// a byte a newer sender's record has at its end (`records` keeps the read inside it), `or`
    /// when it's not there
    fn u8_or(&mut self, or: u8) -> u8 {
        if self.at < self.end {
            self.u8().unwrap_or(or)
        } else {
            or
        }
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes([self.u8()?, self.u8()?]))
    }
    fn pos(&mut self) -> Option<Vec3> {
        let mut v = [0.0f32; 3];
        for c in &mut v {
            *c = self.u16()? as i16 as f32 / 100.0;
        }
        Some(Vec3::from(v))
    }
    fn unit(&mut self) -> Option<f32> {
        Some(self.u8()? as f32 / 255.0)
    }
    fn age(&mut self) -> Option<f32> {
        Some(self.u16()? as f32 / 1000.0)
    }
    fn angle(&mut self) -> Option<f32> {
        Some(self.u16()? as f32 / 65535.0 * std::f32::consts::TAU)
    }
    fn vel(&mut self) -> Option<Vec3> {
        Some(Vec3::new(self.u16()? as i16 as f32, self.u16()? as i16 as f32, self.u16()? as i16 as f32) / 10.0)
    }
    fn ticks(&mut self) -> Option<u32> {
        Some(self.u16()? as u32)
    }
    fn drop_to(&mut self, p: Vec3) -> Option<Option<f32>> {
        let d = self.u8()?;
        Some((d != 255).then(|| p.y - d as f32 / 8.0))
    }
    /// `records`'s: each record read where it starts, the bytes it has past what's read skipped
    /// (a newer sender's fields); one shorter than what's read doesn't read
    fn records<T>(&mut self, f: impl Fn(&mut Self) -> Option<T>) -> Option<Vec<T>> {
        let n = self.u8()? as usize;
        let size = self.u8()? as usize;
        let mut out = Vec::with_capacity(n);
        let outer = self.end;
        for _ in 0..n {
            let start = self.at;
            let end = start.checked_add(size).filter(|&e| e <= self.b.len())?;
            self.end = end;
            let got = f(self);
            self.end = outer;
            out.push(got?);
            if self.at > end {
                return None;
            }
            self.at = end;
        }
        Some(out)
    }
    fn tint(&mut self) -> Option<[f32; 4]> {
        Some([self.unit()?, self.unit()?, self.unit()?, self.unit()?])
    }
    fn list<T>(&mut self, f: impl Fn(&mut Self) -> Option<T>) -> Option<Vec<T>> {
        let n = self.u8()? as usize;
        (0..n).map(|_| f(self)).collect()
    }
    fn opt<T>(&mut self, f: impl Fn(&mut Self) -> Option<T>) -> Option<Option<T>> {
        Some(if self.u8()? == 1 { Some(f(self)?) } else { None })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        a.distance(b) < 0.01
    }

    #[test]
    fn round_trip_around_the_tarnished() {
        let here = Vec3::new(1000.0, 50.0, -300.0);
        let at = |x: f32| here + Vec3::new(x, 1.0, -x);
        let fx = WorldFx {
            fire: FireFx {
                orbs: vec![Orb { pos: at(3.0), vel: Vec3::new(-12.5, 0.3, 40.0), size: 7.0, level: 5, age: 77, flags: ORB_REFLECTED, ground: Some(at(3.0).y - 4.25), vis: 0.3, element: 2 }],
                blasts: vec![Blast { pos: at(1.0), level: 4, age: 20, size: 4.2, ground: None, flags: BLAST_LAUNCH, vis: 1.0, element: 1 }],
                pools: vec![Pool { pos: at(2.0), age: 30, life: 81, radius: 1.3, vis: 1.0 }],
                clouds: vec![Cloud { pos: at(4.0), age: 12, life: 90, radius: 2.0, vis: 1.0 }],
                auras: vec![Aura { pos: at(5.0), height: 2.2, radius: 0.6, element: 3, age: 7, left: 0.5 }],
                motes: vec![Mote { pos: at(0.4), element: 1, t: 0.25, size: 0.1 }],
            },
            streaks: vec![vec![(at(1.0), 0.2), (at(2.0), 0.6)]],
            rangs: vec![(vec![at(4.0), at(5.0)], 0.55)],
            tongue: Some((at(0.5), at(6.0))),
            yoshi_fires: vec![(at(2.0), 0.4)],
            eggs: vec![(at(7.0), 1.25)],
            chain: None,
            hook: Some((at(0.2), at(15.0), 2.0)),
            sparks: vec![Spark { pos: at(1.2), size: 0.8, tint: [1.0; 4], t: 0.5 }],
            dash: vec![(at(-1.0), 0.3)],
            puffs: vec![],
            swing: vec![(at(0.3), at(0.9), 0.1)],
            chomp: vec![(at(1.0), at(3.0), 0.7)],
            flying: vec![(at(2.0), at(2.1), 0.9)],
            rings: vec![Ring { pos: at(0.0), radius: 6.0, tint: [0.9, 0.8, 0.6, 1.0], t: 0.25 }],
            bits: vec![Bit { coin: true, pos: at(1.5), size: 0.4, t: 0.3, age: 0.7 }],
            rubble: Vec::new(),
            bolts: vec![Bolt { from: at(20.0), to: at(1.0), seed: 4321, t: 0.4, width: 0.3 }],
            storms: vec![Storm { pos: at(9.0), radius: 5.0, t: 0.6, flash: 0.8 }],
            crescents: vec![Crescent { pos: at(3.5), yaw: -2.0, roll: -35.0, width: 2.8, alpha: 0.6, kind: 1 }],
            blade: vec![(at(0.6), at(0.2), 0.3), (at(0.9), at(0.3), 0.1)],
            thrown: vec![Thrown { pos: at(2.5), yaw: 1.0, spin: 3.0, size: 0.42 }],
        };
        let mut out = Vec::new();
        fx.encode(here, &mut out);
        let got = WorldFx::decode(&out).expect("reads").shifted(here);
        let c = got.crescents[0];
        let yaw_off = (c.yaw - -2.0f32).rem_euclid(std::f32::consts::TAU);
        assert!(close(c.pos, at(3.5)) && yaw_off.min(std::f32::consts::TAU - yaw_off) < 0.001 && (c.roll + 35.0).abs() < 1.0, "{c:?}");
        assert!((c.width - 2.8).abs() < 0.05 && (c.alpha - 0.6).abs() < 0.01 && c.kind == 1);
        assert_eq!(got.blade.len(), 2);
        let t = got.thrown[0];
        assert!(close(t.pos, at(2.5)) && (t.yaw - 1.0).abs() < 0.01 && (t.spin - 3.0).abs() < 0.01 && (t.size - 0.42).abs() < 0.01, "{t:?}");
        assert!(close(got.blade[1].0, at(0.9)) && close(got.blade[1].1, at(0.3)) && (got.blade[0].2 - 0.3).abs() < 0.01);
        let bolt = got.bolts[0];
        assert!(close(bolt.from, at(20.0)) && close(bolt.to, at(1.0)) && bolt.seed == 4321 && (bolt.t - 0.4).abs() < 0.01 && (bolt.width - 0.3).abs() < 0.01);
        let storm = got.storms[0];
        assert!(close(storm.pos, at(9.0)) && (storm.radius - 5.0).abs() < 0.1 && (storm.flash - 0.8).abs() < 0.01);
        let (o, want) = (got.fire.orbs[0], fx.fire.orbs[0]);
        assert!(close(o.pos, want.pos) && close(o.vel, want.vel) && (o.size - 7.0).abs() < 0.002 && o.level == 5 && o.age == 77 && o.flags == ORB_REFLECTED && o.element == 2);
        assert!((o.ground.expect("ground") - want.ground.expect("ground")).abs() < 0.07 && o.vis == 1.0);
        let b = got.fire.blasts[0];
        assert!(close(b.pos, at(1.0)) && b.level == 4 && b.age == 20 && b.ground.is_none() && b.flags == BLAST_LAUNCH && b.element == 1);
        let c = got.fire.clouds[0];
        assert!(close(c.pos, at(4.0)) && c.age == 12 && c.life == 90 && (c.radius - 2.0).abs() < 0.002);
        let a = got.fire.auras[0];
        assert!(close(a.pos, at(5.0)) && (a.height - 2.2).abs() < 0.07 && a.element == 3 && a.age == 7 && (a.left - 0.5).abs() < 0.01);
        let m = got.fire.motes[0];
        assert!(close(m.pos, at(0.4)) && m.element == 1 && (m.t - 0.25).abs() < 0.01);
        let p = got.fire.pools[0];
        assert!(close(p.pos, at(2.0)) && p.age == 30 && p.life == 81 && (p.radius - 1.3).abs() < 0.002);
        assert!(close(got.streaks[0][1].0, fx.streaks[0][1].0));
        assert!(close(got.rangs[0].0[1], at(5.0)) && (got.rangs[0].1 - 0.55).abs() < 0.05);
        assert!(close(got.tongue.expect("tongue").1, at(6.0)));
        assert!((got.eggs[0].1 - 1.25).abs() < 0.002);
        assert!(got.chain.is_none());
        let (_, head, spin) = got.hook.expect("hook");
        assert!(close(head, at(15.0)) && (spin - 2.0).abs() < 0.001);
        assert!(close(got.rings[0].pos, at(0.0)) && (got.rings[0].radius - 6.0).abs() < 0.2);
        assert!(got.bits[0].coin && (got.bits[0].age - 0.7).abs() < 0.002);
        // nothing: tiny, and empty again
        let mut none = Vec::new();
        WorldFx::default().encode(here, &mut none);
        // (two bytes for each empty record list at the end)
        assert!(none.len() < 40, "{}", none.len());
        assert!(WorldFx::decode(&none).expect("reads").is_empty());
        // cut short: doesn't read
        assert!(WorldFx::decode(&out[..out.len() - 1]).is_none());
    }

    #[test]
    fn a_big_fight_fits_its_budget() {
        // everything at once, every list full: cut down to the budget, still readable, newest kept
        let p = |k: usize| Vec3::new(k as f32 * 0.1, 1.0, 0.0);
        let fx = WorldFx {
            fire: FireFx { orbs: (0..30).map(|k| Orb { pos: p(k), vel: Vec3::X, size: 1.0, level: 1, age: 0, flags: 0, ground: None, vis: 1.0, element: 0 }).collect(), ..Default::default() },
            streaks: (0..30).map(|_| (0..30).map(|k| (p(k), 0.5)).collect()).collect(),
            rangs: (0..3).map(|_| ((0..30).map(p).collect(), 0.5)).collect(),
            sparks: (0..30).map(|k| Spark { pos: p(k), size: 1.0, tint: [1.0; 4], t: 0.5 }).collect(),
            dash: (0..30).map(|k| (p(k), 0.5)).collect(),
            swing: (0..30).map(|k| (p(k), p(k + 1), 0.5)).collect(),
            bits: (0..40).map(|k| Bit { coin: k % 2 == 0, pos: p(k), size: 0.4, t: 0.5, age: 0.2 }).collect(),
            ..Default::default()
        };
        let mut full = Vec::new();
        fx.encode(Vec3::ZERO, &mut full);
        assert!(full.len() > 1000, "full {}", full.len());
        let mut out = vec![9u8; 3];
        fx.encode_fit(Vec3::ZERO, &mut out, 1000);
        assert!(out.len() - 3 <= 1000, "fit {}", out.len());
        let got = WorldFx::decode(&out[3..]).expect("reads");
        assert!(!got.bits.is_empty() && close(got.bits.last().expect("bit").pos, p(39)));
    }

    #[test]
    fn a_newer_senders_longer_records_still_read() {
        // (a later build adding a field to each ball: this one reads what it knows, skips the rest)
        let fx = WorldFx { fire: FireFx { orbs: vec![Orb { pos: Vec3::ONE, vel: Vec3::Z, size: 2.0, level: 3, age: 5, flags: 0, ground: None, vis: 1.0, element: 1 }; 2], ..Default::default() }, ..Default::default() };
        let mut out = Vec::new();
        fx.encode(Vec3::ZERO, &mut out);
        let (n, size) = (out[0] as usize, out[1] as usize);
        assert_eq!(n, 2);
        let mut longer = vec![out[0], out[1] + 2];
        for k in 0..n {
            longer.extend_from_slice(&out[2 + k * size..2 + (k + 1) * size]);
            longer.extend_from_slice(&[0xAB, 0xCD]);
        }
        longer.extend_from_slice(&out[2 + n * size..]);
        let got = WorldFx::decode(&longer).expect("reads");
        assert_eq!(got.fire.orbs.len(), 2);
        assert!(close(got.fire.orbs[1].pos, Vec3::ONE) && got.fire.orbs[1].level == 3 && got.fire.orbs[1].element == 1);
    }

    #[test]
    fn an_older_senders_packet_reads_as_fire() {
        // (before the elements: balls one byte shorter, no clouds, auras, motes at the end)
        let fx = WorldFx { fire: FireFx { orbs: vec![Orb { pos: Vec3::ONE, vel: Vec3::Z, size: 2.0, level: 3, age: 5, flags: 0, ground: None, vis: 1.0, element: 0 }], ..Default::default() }, ..Default::default() };
        let mut out = Vec::new();
        fx.encode(Vec3::ZERO, &mut out);
        let size = out[1] as usize;
        let mut older = vec![out[0], out[1] - 1];
        older.extend_from_slice(&out[2..2 + size - 1]);
        older.extend_from_slice(&out[2 + size..]);
        // (and without the eight lists at the end: each an empty record list, two bytes)
        older.truncate(older.len() - 16);
        let got = WorldFx::decode(&older).expect("reads");
        assert_eq!(got.fire.orbs.len(), 1);
        assert!(got.fire.orbs[0].element == 0 && got.fire.orbs[0].level == 3);
        assert!(got.fire.clouds.is_empty() && got.fire.auras.is_empty() && got.fire.motes.is_empty());
        assert!(got.bolts.is_empty() && got.storms.is_empty() && got.crescents.is_empty() && got.blade.is_empty() && got.thrown.is_empty());
    }

    #[test]
    fn long_trails_keep_the_newest() {
        let fx = WorldFx { dash: (0..60).map(|k| (Vec3::X * k as f32, k as f32 / 60.0)).collect(), ..Default::default() };
        let mut out = Vec::new();
        fx.encode(Vec3::ZERO, &mut out);
        let got = WorldFx::decode(&out).expect("reads");
        assert_eq!(got.dash.len(), MAX);
        assert!(close(got.dash.last().expect("last").0, Vec3::X * 59.0));
    }
}
