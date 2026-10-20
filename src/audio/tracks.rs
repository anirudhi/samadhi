//! Music tracks bundled with the app (generated with Google Gemini; see
//! `NOTICE.md`). They are embedded in the binary so they play on any
//! machine without installing extra files.

pub struct Track {
    pub name: &'static str,
    pub bytes: &'static [u8],
}

impl std::fmt::Debug for Track {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Track")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

macro_rules! track {
    ($name:literal, $file:literal) => {
        Track {
            name: $name,
            bytes: include_bytes!(concat!("../../assets/music/", $file)),
        }
    };
}

pub const TRACKS: [Track; 5] = [
    track!("Before the Sun Wakes", "Before_the_Sun_Wakes.mp3"),
    track!("Slow Light on Water", "Slow_Light_on_Water.mp3"),
    track!("Stone and Bamboo", "Stone_and_Bamboo.mp3"),
    track!("Water Over the Stone", "Water_Over_the_Stone.mp3"),
    track!("Where Water Meets Ink", "Where_Water_Meets_Ink.mp3"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::Source as _;

    #[test]
    fn every_bundled_track_decodes() {
        for track in &TRACKS {
            let decoder = rodio::Decoder::try_from(std::io::Cursor::new(track.bytes))
                .unwrap_or_else(|e| panic!("{}: {e}", track.name));
            assert_eq!(decoder.channels().get(), 2, "{}", track.name);
            // A few seconds decode cleanly.
            assert_eq!(
                decoder.take(44_100 * 2 * 3).count(),
                44_100 * 2 * 3,
                "{}",
                track.name
            );
        }
    }
}
