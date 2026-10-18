//! Plays libsm64's audio (32 kHz stereo s16) through the default output device.
//!
//! The libsm64 thread pushes samples after each tick; a cpal output stream pulls them and
//! resamples linearly to the device rate.

use std::collections::VecDeque;
use std::sync::Mutex;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::log;

pub const SM64_RATE: f32 = 32_000.0;
const MAX_QUEUED_FRAMES: usize = 6000;

static RING: Mutex<VecDeque<[i16; 2]>> = Mutex::new(VecDeque::new());

/// Left/right gains for positional audio, stored as f32 bits (set from the game thread).
static GAIN_L: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x3f35_04f3); // ~0.707
static GAIN_R: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x3f35_04f3);

/// All of SM64's sound (blows, voices, chimes, music) is this loud: 5% under what it was
/// (2026-10-18, the combos' noises were a bit much).
const MASTER: f32 = 0.95;

/// Places Mario's voice: `pan` -1 (left) .. 1 (right), `volume` 0..1. Equal-power panning.
pub fn set_position(pan: f32, volume: f32) {
    let angle = (pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
    let v = volume.clamp(0.0, 1.0) * MASTER;
    GAIN_L.store((angle.cos() * v).to_bits(), std::sync::atomic::Ordering::Relaxed);
    GAIN_R.store((angle.sin() * v).to_bits(), std::sync::atomic::Ordering::Relaxed);
}

/// Frames waiting to be played (libsm64 uses this to decide how much to generate).
pub fn queued() -> u32 {
    RING.lock().map(|r| r.len() as u32).unwrap_or(0)
}

pub fn push(samples: &[i16]) {
    let Ok(mut r) = RING.lock() else { return };
    if r.len() > MAX_QUEUED_FRAMES {
        return;
    }
    r.extend(samples.chunks_exact(2).map(|c| [c[0], c[1]]));
}

/// Set by a stream's error callback (its device was unplugged or switched off): the output
/// thread opens a new one.
static LOST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Starts the output on its own thread (the stream must stay alive there). A lost device, or a
/// new default one (headphones plugged in), gets a new stream: Mario went silent for the rest
/// of the session when the device went away.
pub fn start() {
    std::thread::spawn(|| {
        let host = cpal::default_host();
        let mut current: Option<(cpal::Stream, String)> = None;
        let mut failed = false;
        loop {
            let lost = LOST.swap(false, std::sync::atomic::Ordering::Relaxed);
            let default = host.default_output_device();
            let name = default.as_ref().and_then(|d| d.name().ok()).unwrap_or_default();
            let switched = current.as_ref().is_some_and(|(_, n)| *n != name);
            if lost || switched {
                log(if lost { "audio: device lost, reopening" } else { "audio: default device changed, switching" });
                current = None;
            }
            if current.is_none() {
                match default.map(|d| open(&d)) {
                    Some(Ok(stream)) => {
                        RING.lock().unwrap_or_else(|e| e.into_inner()).clear();
                        LOST.store(false, std::sync::atomic::Ordering::Relaxed);
                        log(format!("audio: stream started ({name})"));
                        current = Some((stream, name));
                        failed = false;
                    }
                    // (no device right now: tried again every second, logged once)
                    Some(Err(e)) if !failed => {
                        log(format!("audio: {e}"));
                        failed = true;
                    }
                    None if !failed => {
                        log("audio: no output device");
                        failed = true;
                    }
                    _ => {}
                }
            }
            std::thread::sleep(std::time::Duration::from_secs(if current.is_some() { 2 } else { 1 }));
        }
    });
}

/// An output stream on `device`, playing the ring.
fn open(device: &cpal::Device) -> Result<cpal::Stream, String> {
    let config = device.default_output_config().map_err(|e| format!("no output config: {e}"))?;
    let rate = config.sample_rate().0 as f32;
    let channels = config.channels() as usize;
    log(format!("audio: output {} Hz, {channels} ch, {:?}", rate, config.sample_format()));
    let step = SM64_RATE / rate;
    let mut pos = 0.0f32;
    let mut cur = [0i16; 2];
    let mut next = [0i16; 2];
    let stream = device
        .build_output_stream(
            &config.into(),
            move |out: &mut [f32], _| {
                let mut ring = RING.lock().unwrap_or_else(|e| e.into_inner());
                let gl = f32::from_bits(GAIN_L.load(std::sync::atomic::Ordering::Relaxed));
                let gr = f32::from_bits(GAIN_R.load(std::sync::atomic::Ordering::Relaxed));
                for frame in out.chunks_mut(channels) {
                    pos += step;
                    while pos >= 1.0 {
                        pos -= 1.0;
                        cur = next;
                        next = ring.pop_front().unwrap_or([0, 0]);
                    }
                    let l = cur[0] as f32 + (next[0] as f32 - cur[0] as f32) * pos;
                    let r = cur[1] as f32 + (next[1] as f32 - cur[1] as f32) * pos;
                    // fold SM64's (mostly centred) mix to mono and place it where Mario is
                    let mono = (l + r) * 0.5 / 32768.0 * std::f32::consts::SQRT_2;
                    for (c, s) in frame.iter_mut().enumerate() {
                        *s = mono * if c % 2 == 0 { gl } else { gr };
                    }
                }
            },
            |e| {
                // (once per stream: the callback can fire over and over)
                if !LOST.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    log(format!("audio stream error: {e}"));
                }
            },
            None,
        )
        .map_err(|e| format!("could not open stream: {e}"))?;
    stream.play().map_err(|e| format!("play failed: {e}"))?;
    Ok(stream)
}
