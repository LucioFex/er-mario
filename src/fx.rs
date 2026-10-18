//! The effects hud.rs draws in the world around a Mario (his fireballs, the hook's line, the
//! boomerangs' streaks, the hammer's and the Chain Chomp's ribbons, the blows' star bursts, stars
//! and coins, the rings on the ground, the dash's streaks, Yoshi's tongue and eggs), gathered in
//! one place: hud.rs draws ours and each co-op partner's alike, and coop.rs sends ours with every
//! pose (positions around the Tarnished, so the other game puts them around where it shows us).

use glam::Vec3;

/// A flame, glow or orb (fireball.rs's frames: FRAME_GLOW, FRAME_ORB on)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fire {
    pub pos: Vec3,
    pub size: f32,
    pub frame: u8,
    pub tint: [f32; 4],
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
    pub fire: Vec<Fire>,
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
}

/// at most this many of each kind go out (the newest), and points per trail
const MAX: usize = 24;

impl WorldFx {
    /// Ours now (Elden Ring world positions).
    pub fn local() -> Self {
        let (rangs, _) = crate::boomerang::sprites();
        let ride = crate::ride::sprites();
        let chomp_hook = crate::chomp::hooking();
        WorldFx {
            fire: crate::fireball::sprites().into_iter().map(|f| Fire { pos: f.pos, size: f.size, frame: f.frame.min(255) as u8, tint: f.tint }).collect(),
            streaks: crate::impact::enemy_streaks(),
            rangs: rangs.into_iter().map(|r| (r.trail.iter().copied().chain(std::iter::once(r.pos)).collect(), r.size)).collect(),
            tongue: ride.tongue,
            yoshi_fires: ride.fires,
            eggs: ride.eggs,
            chain: crate::hook::live().filter(|_| chomp_hook).map(|h| (h.hand, h.head)),
            hook: crate::hook::sprite().filter(|_| !chomp_hook).map(|h| (h.hand, h.head, h.spin)),
            sparks: crate::impact::sparks().into_iter().map(|(s, t)| Spark { pos: s.pos, size: s.size, tint: s.tint, t }).collect(),
            dash: crate::impact::dash_trail(),
            puffs: crate::impact::dash_puffs(),
            swing: crate::impact::swing_trail(),
            chomp: crate::impact::chomp_trail(),
            flying: crate::impact::flying_sparks(),
            rings: crate::impact::rings().into_iter().map(|(r, t)| Ring { pos: r.pos, radius: r.radius, tint: r.tint, t }).collect(),
            bits: crate::impact::bits().into_iter().map(|(b, pos, size, t, age)| Bit { coin: matches!(b, crate::impact::Bit::Coin), pos, size, t, age }).collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        *self == WorldFx::default()
    }

    /// Every position moved by `d`.
    pub fn shifted(mut self, d: Vec3) -> Self {
        for f in &mut self.fire {
            f.pos += d;
        }
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
        self
    }

    /// Ours for the partner (positions around `here`, our Tarnished), appended to `out`.
    pub fn encode(&self, here: Vec3, out: &mut Vec<u8>) {
        let mut w = Writer { out, here };
        w.list(&self.fire, |w, f| {
            w.pos(f.pos);
            w.unit(f.size / 8.0);
            w.out.push(f.frame);
            w.tint(f.tint);
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
    }

    /// The partner's from `b` (positions around their Tarnished: `shifted` puts them where we show
    /// them). None if it doesn't read.
    pub fn decode(b: &[u8]) -> Option<WorldFx> {
        let mut r = Reader { b, at: 0 };
        let mut fx = WorldFx { fire: r.list(|r| Some(Fire { pos: r.pos()?, size: r.unit()? * 8.0, frame: r.u8()?, tint: r.tint()? }))?, ..Default::default() };
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
        Some(fx)
    }
}

/// Positions as cm around a point (i16: 327 m each way), shares 0-1 as u8, ages as u16 ms.
struct Writer<'a> {
    out: &'a mut Vec<u8>,
    here: Vec3,
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
    fn angle(&mut self, a: f32) {
        self.out.extend_from_slice(&((a.rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU * 65535.0) as u16).to_le_bytes());
    }
    fn tint(&mut self, t: [f32; 4]) {
        for v in t {
            self.unit(v);
        }
    }
    /// (the newest MAX: the lists go oldest first)
    fn list<T>(&mut self, items: &[T], f: impl Fn(&mut Self, &T)) {
        let from = items.len().saturating_sub(MAX);
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
}

impl Reader<'_> {
    fn u8(&mut self) -> Option<u8> {
        let v = *self.b.get(self.at)?;
        self.at += 1;
        Some(v)
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
            fire: vec![Fire { pos: at(3.0), size: 1.5, frame: 9, tint: [1.0, 0.5, 0.0, 0.8] }],
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
        };
        let mut out = Vec::new();
        fx.encode(here, &mut out);
        let got = WorldFx::decode(&out).expect("reads").shifted(here);
        assert!(close(got.fire[0].pos, fx.fire[0].pos) && got.fire[0].frame == 9 && (got.fire[0].size - 1.5).abs() < 0.05);
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
        assert!(none.len() < 24);
        assert!(WorldFx::decode(&none).expect("reads").is_empty());
        // cut short: doesn't read
        assert!(WorldFx::decode(&out[..out.len() - 1]).is_none());
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
