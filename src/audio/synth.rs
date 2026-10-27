//! Sounds generated in code: key clicks and endless ambient soundscapes.
//!
//! Generating them means the app ships with no third-party audio files (the
//! app it was ported from bundled tracks under mixed licences), never runs out
//! of music, and loops without a seam.

use std::f32::consts::TAU;
use std::num::NonZero;
use std::time::Duration;

use rodio::{ChannelCount, Sample, SampleRate, Source};

pub const SAMPLE_RATE: u32 = 44_100;
const SR: f32 = SAMPLE_RATE as f32;

/// Small, fast, deterministic PRNG (xorshift32); audio noise needs no more.
#[derive(Debug, Clone)]
struct Noise(u32);

impl Noise {
    fn new(seed: u32) -> Self {
        Self(seed.max(1))
    }

    /// Uniform in [-1, 1).
    fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    /// Uniform in [0, 1).
    fn unit(&mut self) -> f32 {
        self.next() * 0.5 + 0.5
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinClick {
    Felt,
    Whisper,
    Pencil,
    Raindrop,
    WoodTap,
    SoftTick,
    Typewriter,
    Bamboo,
    Droplet,
}

impl BuiltinClick {
    /// Quietest first; the first is the default.
    pub const ALL: [(&'static str, Self); 9] = [
        ("Felt", Self::Felt),
        ("Whisper", Self::Whisper),
        ("Pencil", Self::Pencil),
        ("Raindrop", Self::Raindrop),
        ("Wood tap", Self::WoodTap),
        ("Soft tick", Self::SoftTick),
        ("Typewriter", Self::Typewriter),
        ("Bamboo", Self::Bamboo),
        ("Droplet", Self::Droplet),
    ];

    /// Mono samples at [`SAMPLE_RATE`].
    pub fn samples(self) -> Vec<f32> {
        let len = |secs: f32| (secs * SR) as usize;
        let decay = |t: f32, tau: f32| (-t / tau).exp();
        let mut noise = Noise::new(0x5EED);
        let mut lp = 0.0_f32;
        let mut lp2 = 0.0_f32;
        let out: Vec<f32> = match self {
            Self::Felt => (0..len(0.04))
                .map(|i| {
                    let t = i as f32 / SR;
                    // Heavily low-passed noise with a little body: a key
                    // pressed through felt.
                    lp += 0.06 * (noise.next() - lp);
                    lp2 += 0.2 * (lp - lp2);
                    let thump = (TAU * 95.0 * t).sin() * decay(t, 0.006) * 0.25;
                    let attack = (t / 0.000_8).min(1.0);
                    (lp2 * 2.2 * decay(t, 0.004) + thump) * attack * 0.55
                })
                .collect(),
            Self::Whisper => (0..len(0.035))
                .map(|i| {
                    let t = i as f32 / SR;
                    // Band-limited air: low-pass minus a slower low-pass.
                    let n = noise.next();
                    lp += 0.3 * (n - lp);
                    lp2 += 0.04 * (lp - lp2);
                    let envelope = (t / 0.003).min(1.0) * decay(t, 0.006);
                    (lp - lp2) * envelope * 0.32
                })
                .collect(),
            Self::Pencil => (0..len(0.05))
                .map(|i| {
                    let t = i as f32 / SR;
                    // A short scratch: bright noise, fluttering slightly.
                    let n = noise.next();
                    lp += 0.5 * (n - lp);
                    let high = n - lp;
                    let grain = 0.7 + 0.3 * (TAU * 180.0 * t).sin();
                    let envelope = (t / 0.004).min(1.0) * decay(t, 0.012);
                    high * grain * envelope * 0.16
                })
                .collect(),
            Self::Raindrop => {
                let mut phase = 0.0_f32;
                (0..len(0.04))
                    .map(|i| {
                        let t = i as f32 / SR;
                        let f = 1_500.0 + 700.0 * decay(t, 0.004);
                        phase = (phase + f / SR).fract();
                        let attack = (t / 0.000_6).min(1.0);
                        (TAU * phase).sin() * decay(t, 0.007) * attack * 0.22
                    })
                    .collect()
            }
            Self::WoodTap => (0..len(0.06))
                .map(|i| {
                    let t = i as f32 / SR;
                    let tone = (TAU * 320.0 * t).sin() * decay(t, 0.012) * 0.6
                        + (TAU * 760.0 * t).sin() * decay(t, 0.006) * 0.25;
                    lp += 0.2 * (noise.next() - lp);
                    let knock = lp * decay(t, 0.001_5) * 0.6;
                    let attack = (t / 0.000_5).min(1.0);
                    (tone + knock) * attack * 0.42
                })
                .collect(),
            Self::SoftTick => (0..len(0.03))
                .map(|i| {
                    let t = i as f32 / SR;
                    // Low-passed noise burst: a muted, paper-like tap.
                    lp += 0.35 * (noise.next() - lp);
                    lp * decay(t, 0.002_5) * 0.9
                })
                .collect(),
            Self::Typewriter => (0..len(0.06))
                .map(|i| {
                    let t = i as f32 / SR;
                    let strike = noise.next() * decay(t, 0.001_5);
                    let body = (TAU * 150.0 * t).sin() * decay(t, 0.012) * 0.6;
                    let ring = (TAU * 3_150.0 * t).sin() * decay(t, 0.006) * 0.15;
                    (strike + body + ring) * 0.7
                })
                .collect(),
            Self::Bamboo => (0..len(0.12))
                .map(|i| {
                    let t = i as f32 / SR;
                    let modes = [
                        (520.0, 0.030, 0.6),
                        (1_430.0, 0.012, 0.3),
                        (2_900.0, 0.006, 0.15),
                    ];
                    let tone: f32 = modes
                        .iter()
                        .map(|&(f, tau, a)| (TAU * f * t).sin() * decay(t, tau) * a)
                        .sum();
                    let attack = (t / 0.000_5).min(1.0);
                    tone * attack * 0.8
                })
                .collect(),
            Self::Droplet => {
                let mut phase = 0.0_f32;
                (0..len(0.08))
                    .map(|i| {
                        let t = i as f32 / SR;
                        // Pitch falls quickly, like a drop hitting water.
                        let f = 700.0 + 900.0 * decay(t, 0.008);
                        phase = (phase + f / SR).fract();
                        let attack = (t / 0.001).min(1.0);
                        (TAU * phase).sin() * decay(t, 0.02) * attack * 0.6
                    })
                    .collect()
            }
        };
        fade_out(out, 0.005)
    }
}

/// A soft singing-bowl tone for the end of a timer: struck once, then
/// again more quietly, ringing out over a few seconds. Mono, at
/// [`SAMPLE_RATE`].
pub fn chime() -> Vec<f32> {
    const PITCH: f32 = 392.0; // G4
    // Bowls ring with inharmonic overtones; higher ones die away sooner.
    const MODES: [(f32, f32, f32); 4] = [
        (1.0, 2.6, 0.55),
        (2.71, 1.3, 0.22),
        (5.15, 0.6, 0.1),
        (8.5, 0.25, 0.04),
    ];
    let strike = |t: f32| -> f32 {
        if t < 0.0 {
            return 0.0;
        }
        let attack = (t / 0.004).min(1.0);
        MODES
            .iter()
            .map(|&(ratio, tau, amp)| {
                // A slight beat between two close partials makes it shimmer.
                let f = PITCH * ratio;
                let beat = 1.0 + 0.25 * (TAU * 1.3 * t).sin();
                (TAU * f * t).sin() * (-t / tau).exp() * amp * beat
            })
            .sum::<f32>()
            * attack
    };
    let len = (4.5 * SR) as usize;
    let out = (0..len)
        .map(|i| {
            let t = i as f32 / SR;
            f32::midpoint(strike(t), 0.6 * strike(t - 1.4))
        })
        .collect();
    fade_out(out, 0.3)
}

/// Applies a linear fade over the last `secs`, so a sound never ends on a
/// non-zero sample (which is heard as a pop).
fn fade_out(mut samples: Vec<f32>, secs: f32) -> Vec<f32> {
    let n = ((secs * SR) as usize).min(samples.len());
    let start = samples.len() - n;
    for (i, s) in samples[start..].iter_mut().enumerate() {
        *s *= 1.0 - (i + 1) as f32 / n as f32;
    }
    samples
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinMusic {
    Drift,
    Rain,
    Ocean,
}

impl BuiltinMusic {
    pub const ALL: [(&'static str, Self); 3] = [
        ("Drift (ambient pad)", Self::Drift),
        ("Gentle rain", Self::Rain),
        ("Ocean swell", Self::Ocean),
    ];

    pub fn source(self) -> Ambient {
        Ambient::new(self)
    }
}

/// A slowly evolving pad voice made of two slightly detuned sines.
#[derive(Debug, Clone)]
struct Voice {
    freq: f32,
    target: f32,
    phase_a: f32,
    phase_b: f32,
    lfo_phase: f32,
    lfo_rate: f32,
    pan_l: f32,
    pan_r: f32,
}

const CHORDS: [[u8; 5]; 4] = [
    [45, 52, 57, 59, 64], // A minor (add 9)
    [41, 48, 55, 57, 64], // F major 7
    [43, 50, 55, 59, 62], // G major
    [40, 47, 55, 59, 62], // E minor 7
];
const CHORD_SECONDS: f32 = 18.0;

fn midi_to_hz(note: u8) -> f32 {
    440.0 * 2.0_f32.powf((f32::from(note) - 69.0) / 12.0)
}

/// An infinite stereo soundscape.
#[derive(Debug, Clone)]
pub struct Ambient {
    kind: BuiltinMusic,
    noise_l: Noise,
    noise_r: Noise,
    t: u64,
    pending_right: Option<f32>,
    voices: Vec<Voice>,
    chord: usize,
    // Filter states (left, right).
    lp1: [f32; 2],
    lp2: [f32; 2],
    brown: [f32; 2],
    drop_phase: f32,
    drop_freq: f32,
    drop_env: f32,
    drop_pan: f32,
}

impl Ambient {
    fn new(kind: BuiltinMusic) -> Self {
        let voices = CHORDS[0]
            .iter()
            .enumerate()
            .map(|(i, &n)| {
                let pan = 0.15 + 0.7 * (i as f32 / 4.0);
                let angle = pan * std::f32::consts::FRAC_PI_2;
                Voice {
                    freq: midi_to_hz(n),
                    target: midi_to_hz(n),
                    phase_a: i as f32 * 0.21,
                    phase_b: i as f32 * 0.37,
                    lfo_phase: i as f32 * 0.19,
                    lfo_rate: 1.0 / (9.0 + 3.7 * i as f32),
                    pan_l: angle.cos(),
                    pan_r: angle.sin(),
                }
            })
            .collect();
        Self {
            kind,
            noise_l: Noise::new(0x1234_5678),
            noise_r: Noise::new(0x9E37_79B9),
            t: 0,
            pending_right: None,
            voices,
            chord: 0,
            lp1: [0.0; 2],
            lp2: [0.0; 2],
            brown: [0.0; 2],
            drop_phase: 0.0,
            drop_freq: 0.0,
            drop_env: 0.0,
            drop_pan: 0.5,
        }
    }

    fn frame(&mut self) -> (f32, f32) {
        let secs = self.t as f32 / SR;
        self.t += 1;
        // Fade in over the first two seconds.
        let fade = (secs / 2.0).min(1.0);
        let (l, r) = match self.kind {
            BuiltinMusic::Drift => self.drift(secs),
            BuiltinMusic::Rain => self.rain(secs),
            BuiltinMusic::Ocean => self.ocean(secs),
        };
        // Level the soundscapes so switching between them keeps a steady volume.
        let gain = match self.kind {
            BuiltinMusic::Drift => 0.9,
            BuiltinMusic::Rain => 1.55,
            BuiltinMusic::Ocean => 0.34,
        };
        (l * fade * gain, r * fade * gain)
    }

    fn drift(&mut self, secs: f32) -> (f32, f32) {
        let chord = (secs / CHORD_SECONDS) as usize % CHORDS.len();
        if chord != self.chord {
            self.chord = chord;
            for (v, &n) in self.voices.iter_mut().zip(&CHORDS[chord]) {
                v.target = midi_to_hz(n);
            }
        }
        // Notes glide to the next chord with a ~1.5 s time constant.
        let glide = 1.0 / (SR * 1.5);
        let (mut l, mut r) = (0.0, 0.0);
        for v in &mut self.voices {
            v.freq += (v.target - v.freq) * glide;
            v.phase_a = (v.phase_a + (v.freq + 0.18) / SR).fract();
            v.phase_b = (v.phase_b + (v.freq - 0.18) / SR).fract();
            v.lfo_phase = (v.lfo_phase + v.lfo_rate / SR).fract();
            let amp = 0.35 + 0.65 * (0.5 + 0.5 * (TAU * v.lfo_phase).sin());
            let s = ((TAU * v.phase_a).sin()
                + (TAU * v.phase_b).sin()
                + 0.15 * (TAU * v.phase_a * 2.0).sin())
                * amp;
            l += s * v.pan_l;
            r += s * v.pan_r;
        }
        (l * 0.055, r * 0.055)
    }

    fn rain(&mut self, secs: f32) -> (f32, f32) {
        let swell = 0.75 + 0.15 * (TAU * secs / 13.0).sin() + 0.1 * (TAU * secs / 5.3).sin();
        let mut ch = |i: usize, n: f32| {
            // Band-limited hiss: low-pass then remove rumble.
            self.lp1[i] += 0.25 * (n - self.lp1[i]);
            self.lp2[i] += 0.02 * (self.lp1[i] - self.lp2[i]);
            (self.lp1[i] - self.lp2[i]) * 0.22 * swell
        };
        let nl = self.noise_l.next();
        let nr = self.noise_r.next();
        let (mut l, mut r) = (ch(0, nl), ch(1, nr));

        // Occasional droplets on the window.
        if self.drop_env < 0.001 && self.noise_l.unit() < 3.0 / SR {
            self.drop_freq = 1_800.0 + 2_500.0 * self.noise_r.unit();
            self.drop_env = 0.05 + 0.08 * self.noise_r.unit();
            self.drop_pan = self.noise_l.unit();
        }
        if self.drop_env >= 0.001 {
            self.drop_phase = (self.drop_phase + self.drop_freq / SR).fract();
            let d = (TAU * self.drop_phase).sin() * self.drop_env;
            self.drop_env *= 1.0 - 1.0 / (0.012 * SR);
            l += d * (1.0 - self.drop_pan);
            r += d * self.drop_pan;
        }
        (l, r)
    }

    fn ocean(&mut self, secs: f32) -> (f32, f32) {
        let wave = |period: f32, offset: f32| {
            let s = 0.5 + 0.5 * (TAU * (secs / period + offset)).sin();
            s * s
        };
        let swell_l = 0.15 + 0.85 * wave(9.5, 0.0);
        let swell_r = 0.15 + 0.85 * wave(10.3, 0.3);
        let mut ch = |i: usize, n: f32, swell: f32| {
            // Brown noise, filtered more brightly as the wave breaks.
            self.brown[i] = (self.brown[i] + n * 0.02) * 0.998;
            let alpha = 0.01 + 0.12 * swell;
            self.lp1[i] += alpha * (self.brown[i] + n * 0.05 * swell - self.lp1[i]);
            self.lp1[i] * swell * 1.6
        };
        let nl = self.noise_l.next();
        let nr = self.noise_r.next();
        (ch(0, nl, swell_l), ch(1, nr, swell_r))
    }
}

impl Iterator for Ambient {
    type Item = Sample;

    fn next(&mut self) -> Option<Sample> {
        if let Some(r) = self.pending_right.take() {
            return Some(r);
        }
        let (l, r) = self.frame();
        self.pending_right = Some(r.clamp(-1.0, 1.0));
        Some(l.clamp(-1.0, 1.0))
    }
}

impl Source for Ambient {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZero::new(2).expect("2 is non-zero")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZero::new(SAMPLE_RATE).expect("sample rate is non-zero")
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(s: &[f32]) -> f32 {
        (s.iter().map(|x| x * x).sum::<f32>() / s.len() as f32).sqrt()
    }

    #[test]
    fn clicks_are_short_audible_and_unclipped() {
        for (name, click) in BuiltinClick::ALL {
            let s = click.samples();
            assert!(
                !s.is_empty() && s.len() < SAMPLE_RATE as usize / 4,
                "{name} length"
            );
            assert!(s.iter().all(|x| x.abs() <= 1.0), "{name} clips");
            assert!(rms(&s) > 0.005, "{name} is silent");
            assert!(s.last().unwrap().abs() < 0.01, "{name} ends with a pop");
        }
    }

    #[test]
    fn the_chime_is_gentle_and_ends_cleanly() {
        let s = chime();
        let peak = s.iter().fold(0.0_f32, |m, x| m.max(x.abs()));
        assert!(peak > 0.1 && peak < 0.6, "peak {peak}");
        assert!(s.last().unwrap().abs() < 1e-3, "ends with a pop");
    }

    #[test]
    fn subtle_clicks_are_quieter_than_the_classic_ones() {
        let peak = |c: BuiltinClick| c.samples().iter().fold(0.0_f32, |m, x| m.max(x.abs()));
        let loudest_subtle = [
            BuiltinClick::Felt,
            BuiltinClick::Whisper,
            BuiltinClick::Pencil,
            BuiltinClick::Raindrop,
            BuiltinClick::WoodTap,
        ]
        .map(|c| (c, peak(c), rms(&c.samples())));
        let classic_rms = rms(&BuiltinClick::Typewriter.samples());
        for (c, p, r) in loudest_subtle {
            assert!(p < 0.5, "{c:?} peak {p}");
            assert!(r < classic_rms, "{c:?} rms {r} vs typewriter {classic_rms}");
        }
    }

    #[test]
    fn ambient_sources_are_stereo_endless_and_in_range() {
        for (name, music) in BuiltinMusic::ALL {
            let src = music.source();
            assert_eq!(src.channels().get(), 2);
            assert_eq!(src.total_duration(), None);
            // Skip the fade-in, then check five seconds of audio.
            let s: Vec<f32> = src
                .skip(2 * 2 * SAMPLE_RATE as usize)
                .take(5 * 2 * SAMPLE_RATE as usize)
                .collect();
            assert_eq!(s.len(), 5 * 2 * SAMPLE_RATE as usize, "{name} ended");
            assert!(
                s.iter().all(|x| x.is_finite() && x.abs() <= 1.0),
                "{name} range"
            );
            let level = rms(&s);
            assert!(level > 0.005 && level < 0.5, "{name} level {level}");
        }
    }

    #[test]
    fn soundscapes_have_similar_loudness() {
        let levels: Vec<(BuiltinMusic, f32)> = BuiltinMusic::ALL
            .iter()
            .map(|&(_, m)| {
                let s: Vec<f32> = m
                    .source()
                    .skip(2 * 2 * SAMPLE_RATE as usize)
                    .take(20 * 2 * SAMPLE_RATE as usize)
                    .collect();
                (m, rms(&s))
            })
            .collect();
        let loudest = levels.iter().map(|l| l.1).fold(0.0, f32::max);
        let quietest = levels.iter().map(|l| l.1).fold(f32::MAX, f32::min);
        assert!(loudest / quietest < 1.5, "{levels:?}");
    }

    #[test]
    fn noise_is_deterministic_and_bounded() {
        let mut a = Noise::new(7);
        let mut b = Noise::new(7);
        for _ in 0..10_000 {
            let x = a.next();
            assert!((-1.0..=1.0).contains(&x));
            assert!((x - b.next()).abs() < f32::EPSILON);
        }
    }
}
