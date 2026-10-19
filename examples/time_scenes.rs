//! Prints how long each built-in theme takes to paint.
//! `cargo run --release --example time_scenes`
fn main() {
    for (name, kind) in samadhi::scene::ThemeKind::ALL {
        let start = std::time::Instant::now();
        let scene = kind.build();
        println!(
            "{name:14} {:>6.0} ms  ({} layers)",
            start.elapsed().as_secs_f64() * 1000.0,
            scene.layers.len()
        );
    }
}
