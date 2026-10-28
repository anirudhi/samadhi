//! Background music and key-click playback.
//!
//! Audio is optional: if no output device can be opened the app keeps
//! working silently instead of failing to start.

pub mod synth;
pub mod tracks;

use std::fs::File;
use std::io::BufReader;
use std::num::NonZero;
use std::path::Path;
use std::time::Duration;

use anyhow::Context as _;
use rodio::buffer::SamplesBuffer;
use rodio::{ChannelCount, Decoder, MixerDeviceSink, Player, SampleRate, Source};

use crate::catalog;
pub use synth::{BuiltinClick, BuiltinMusic};

/// Built-in music: a generated soundscape or a bundled track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Music {
    Soundscape(BuiltinMusic),
    Track(usize),
}

/// Every built-in piece of music, soundscapes first, with display names.
pub fn builtin_music() -> Vec<(String, Music)> {
    BuiltinMusic::ALL
        .iter()
        .map(|&(name, m)| (name.to_owned(), Music::Soundscape(m)))
        .chain(
            tracks::TRACKS
                .iter()
                .enumerate()
                .map(|(i, t)| (t.name.to_owned(), Music::Track(i))),
        )
        .collect()
}

pub type MusicSource = catalog::Source<Music>;
pub type ClickSource = catalog::Source<BuiltinClick>;

/// Audio file extensions accepted for music and clicks.
pub const AUDIO_EXTENSIONS: &[&str] = &["mp3", "wav", "ogg", "oga", "flac"];

/// Longest key-click accepted from a user file; anything longer is cut.
const MAX_CLICK: Duration = Duration::from_secs(2);
const MUSIC_FADE_IN: Duration = Duration::from_millis(1500);

/// A fully decoded key-click, kept in memory so every keystroke plays with
/// no disk access or decoding latency.
#[derive(Debug, Clone)]
struct Click {
    channels: ChannelCount,
    rate: SampleRate,
    samples: Vec<f32>,
}

impl Click {
    fn load(source: &ClickSource) -> anyhow::Result<Self> {
        match source {
            catalog::Source::Builtin(kind) => Ok(Self {
                channels: NonZero::new(1).expect("1 is non-zero"),
                rate: NonZero::new(synth::SAMPLE_RATE).expect("non-zero"),
                samples: kind.samples(),
            }),
            catalog::Source::File(path) => {
                let decoder = open_decoder(path)?;
                let channels = decoder.channels();
                let rate = decoder.sample_rate();
                let samples = decoder.take_duration(MAX_CLICK).collect();
                Ok(Self {
                    channels,
                    rate,
                    samples,
                })
            }
        }
    }
}

fn open_decoder(path: &Path) -> anyhow::Result<Decoder<BufReader<File>>> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    Decoder::try_from(file).with_context(|| format!("decoding {}", path.display()))
}

pub struct AudioEngine {
    device: Option<MixerDeviceSink>,
    music: Option<Player>,
    click: Option<Click>,
    music_volume: f32,
    click_volume: f32,
    music_muted: bool,
    click_muted: bool,
    /// Cheap LCG used to vary each click slightly so fast typing sounds
    /// natural rather than machine-gun-like.
    jitter: u32,
}

impl std::fmt::Debug for AudioEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioEngine")
            .field("has_device", &self.device.is_some())
            .field("music_muted", &self.music_muted)
            .field("click_muted", &self.click_muted)
            .finish_non_exhaustive()
    }
}

impl Default for AudioEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioEngine {
    /// An engine that never makes a sound (used in tests).
    pub fn silent() -> Self {
        Self::with_device(None)
    }

    pub fn new() -> Self {
        let device = match rodio::DeviceSinkBuilder::open_default_sink() {
            Ok(mut sink) => {
                sink.log_on_drop(false);
                Some(sink)
            }
            Err(err) => {
                log::warn!("no audio output available, continuing without sound: {err}");
                None
            }
        };
        Self::with_device(device)
    }

    fn with_device(device: Option<MixerDeviceSink>) -> Self {
        Self {
            device,
            music: None,
            click: None,
            music_volume: 1.0,
            click_volume: 1.0,
            music_muted: false,
            click_muted: false,
            jitter: 0x2545_F491,
        }
    }

    pub fn has_device(&self) -> bool {
        self.device.is_some()
    }

    /// Starts `source` looping forever, replacing any current track.
    pub fn set_music(&mut self, source: &MusicSource) -> anyhow::Result<()> {
        // Dropping the old player stops it.
        self.music = None;
        let Some(device) = &self.device else {
            return Ok(());
        };
        let player = Player::connect_new(device.mixer());
        match source {
            catalog::Source::Builtin(Music::Soundscape(kind)) => player.append(kind.source()),
            catalog::Source::Builtin(Music::Track(i)) => {
                let track = &tracks::TRACKS[*i];
                let looped = Decoder::new_looped(std::io::Cursor::new(track.bytes))
                    .with_context(|| format!("decoding {}", track.name))?;
                player.append(looped.fade_in(MUSIC_FADE_IN));
            }
            catalog::Source::File(path) => {
                let file =
                    File::open(path).with_context(|| format!("opening {}", path.display()))?;
                let looped = Decoder::new_looped(BufReader::new(file))
                    .with_context(|| format!("decoding {}", path.display()))?;
                player.append(looped.fade_in(MUSIC_FADE_IN));
            }
        }
        player.set_volume(self.music_volume);
        if self.music_muted {
            player.pause();
        }
        self.music = Some(player);
        Ok(())
    }

    /// Muting pauses the track, so unmuting resumes where it left off.
    pub fn set_music_muted(&mut self, muted: bool) {
        self.music_muted = muted;
        if let Some(player) = &self.music {
            if muted {
                player.pause();
            } else {
                player.play();
            }
        }
    }

    pub fn set_music_volume(&mut self, volume: f32) {
        self.music_volume = volume;
        if let Some(player) = &self.music {
            player.set_volume(volume);
        }
    }

    pub fn set_click(&mut self, source: &ClickSource) -> anyhow::Result<()> {
        self.click = Some(Click::load(source)?);
        Ok(())
    }

    pub fn set_click_muted(&mut self, muted: bool) {
        self.click_muted = muted;
    }

    pub fn set_click_volume(&mut self, volume: f32) {
        self.click_volume = volume;
    }

    /// Plays the key-click. Clicks overlap rather than cut each other off.
    pub fn play_click(&mut self) {
        if self.click_muted {
            return;
        }
        if let Some(click) = self.click.take() {
            self.play(&click);
            self.click = Some(click);
        }
    }

    /// Plays `source` once (even when typing sounds are muted), without
    /// changing the chosen sound. Used to preview sounds on hover.
    pub fn preview_click(&mut self, source: &ClickSource) -> anyhow::Result<()> {
        let click = Click::load(source)?;
        self.play(&click);
        Ok(())
    }

    /// Plays the timer's end chime. It sounds even when typing sounds are
    /// muted, since it is the point of setting a timer.
    pub fn play_chime(&mut self) {
        let Some(device) = &self.device else {
            return;
        };
        let samples = synth::chime();
        let buffer = SamplesBuffer::new(
            NonZero::new(1).expect("1 is non-zero"),
            NonZero::new(synth::SAMPLE_RATE).expect("non-zero"),
            samples,
        );
        device.mixer().add(buffer.amplify(0.8));
    }

    fn play(&mut self, click: &Click) {
        let Some(device) = &self.device else {
            return;
        };
        self.jitter = self
            .jitter
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        let r = (self.jitter >> 8) as f32 / (1u32 << 24) as f32; // [0, 1)
        let speed = 0.96 + 0.08 * r;
        let gain = self.click_volume * (0.85 + 0.15 * r);
        let buffer = SamplesBuffer::new(click.channels, click.rate, click.samples.clone());
        device.mixer().add(buffer.speed(speed).amplify(gain));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A mono 16-bit PCM WAV file of `secs` seconds of a quiet ramp.
    fn wav(dir: &Path, name: &str, rate: u32, secs: u32) -> PathBuf {
        let samples = rate * secs;
        let data_len = samples * 2;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
        bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
        bytes.extend_from_slice(&rate.to_le_bytes());
        bytes.extend_from_slice(&(rate * 2).to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        for i in 0..samples {
            bytes.extend_from_slice(&((i % 200) as i16 * 10).to_le_bytes());
        }
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn builtin_music_lists_soundscapes_then_tracks_with_unique_names() {
        let music = builtin_music();
        assert_eq!(music.len(), BuiltinMusic::ALL.len() + tracks::TRACKS.len());
        let soundscapes = BuiltinMusic::ALL.len();
        assert!(
            music[..soundscapes]
                .iter()
                .all(|(_, m)| matches!(m, Music::Soundscape(_)))
        );
        for (i, (name, m)) in music[soundscapes..].iter().enumerate() {
            assert_eq!(*m, Music::Track(i));
            assert_eq!(name, tracks::TRACKS[i].name);
        }
        let names: std::collections::HashSet<_> = music.iter().map(|(n, _)| n).collect();
        assert_eq!(names.len(), music.len());
    }

    #[test]
    fn a_silent_engine_accepts_everything_quietly() {
        let mut audio = AudioEngine::silent();
        assert!(!audio.has_device());
        assert!(format!("{audio:?}").contains("has_device: false"));
        for (_, music) in builtin_music() {
            audio
                .set_music(&catalog::Source::Builtin(music))
                .expect("silent engines ignore music");
        }
        // With no device there is nothing to decode a missing file for.
        audio
            .set_music(&catalog::Source::File("/no/such/track.mp3".into()))
            .unwrap();
        assert!(audio.music.is_none());
        for (_, click) in BuiltinClick::ALL {
            audio.set_click(&catalog::Source::Builtin(click)).unwrap();
            audio.play_click();
            audio
                .preview_click(&catalog::Source::Builtin(click))
                .unwrap();
        }
        audio.set_music_muted(true);
        audio.set_music_volume(0.3);
        audio.set_click_muted(true);
        audio.set_click_volume(0.0);
        audio.play_click();
        audio.play_chime();
        audio.set_music_muted(false);
        assert!(!audio.music_muted);
        assert!(audio.click.is_some(), "playing a click keeps it selected");
    }

    #[test]
    fn builtin_clicks_load_as_mono_at_the_synth_rate() {
        for (name, kind) in BuiltinClick::ALL {
            let click = Click::load(&catalog::Source::Builtin(kind)).unwrap();
            assert_eq!(click.channels.get(), 1, "{name}");
            assert_eq!(click.rate.get(), synth::SAMPLE_RATE, "{name}");
            assert!(!click.samples.is_empty(), "{name}");
        }
    }

    #[test]
    fn click_files_are_decoded_and_cut_short() {
        let dir = tempfile::tempdir().unwrap();
        let short = wav(dir.path(), "short.wav", 8000, 1);
        let click = Click::load(&catalog::Source::File(short)).unwrap();
        assert_eq!(click.rate.get(), 8000);
        assert_eq!(click.samples.len(), 8000);

        let long = wav(dir.path(), "long.wav", 8000, 5);
        let click = Click::load(&catalog::Source::File(long)).unwrap();
        let max = (MAX_CLICK.as_secs_f32() * 8000.0) as usize;
        assert!(
            click.samples.len().abs_diff(max) <= 1,
            "{}",
            click.samples.len()
        );
    }

    #[test]
    fn unreadable_click_files_are_errors_and_keep_the_old_click() {
        let dir = tempfile::tempdir().unwrap();
        let garbage = dir.path().join("noise.wav");
        std::fs::write(&garbage, b"not audio at all").unwrap();
        let mut audio = AudioEngine::silent();
        audio
            .set_click(&catalog::Source::Builtin(BuiltinClick::Felt))
            .unwrap();
        for path in [garbage, dir.path().join("missing.wav")] {
            let source = catalog::Source::File(path);
            assert!(audio.set_click(&source).is_err());
            assert!(audio.preview_click(&source).is_err());
        }
        let kept = audio.click.as_ref().unwrap();
        assert_eq!(kept.samples, BuiltinClick::Felt.samples());
    }

    #[test]
    fn audio_extensions_are_lowercase_without_dots() {
        for ext in AUDIO_EXTENSIONS {
            assert!(!ext.starts_with('.'));
            assert_eq!(*ext, ext.to_ascii_lowercase());
        }
    }
}
