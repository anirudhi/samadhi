//! Live markdown: parsing and layout of arbitrary text.

use std::ops::Range;
use std::time::{Duration, Instant};

use eframe::egui::Color32;
use proptest::prelude::*;
use samadhi::markdown::{Look, Span, layout, parse};

use crate::{config, tricky_text};

fn look() -> Look {
    Look {
        fonts: samadhi::fonts::TYPEFACES[0].writing_fonts(16.0),
        size: 16.0,
        line_spacing: 1.6,
        ink: Color32::BLACK,
        accent: Color32::BLUE,
    }
}

/// Spans are non-empty, in order, without gaps or overlaps, on character
/// boundaries, and cover the whole text.
fn check_spans(text: &str, spans: &[Span]) -> Result<(), TestCaseError> {
    let mut expect = 0;
    for s in spans {
        let (start, end) = s.range;
        prop_assert_eq!(start, expect, "gap or overlap in {:?}", text);
        prop_assert!(start < end, "empty span in {:?}", text);
        prop_assert!(end <= text.len());
        prop_assert!(text.is_char_boundary(start) && text.is_char_boundary(end));
        prop_assert!(s.style.heading <= 6);
        expect = end;
    }
    prop_assert_eq!(expect, text.len());
    // Neighbours with the same style are merged.
    for pair in spans.windows(2) {
        prop_assert!(
            pair[0].style != pair[1].style,
            "unmerged spans in {:?}",
            text
        );
    }
    Ok(())
}

/// Any range at all, including inverted, empty and out-of-bounds ones and
/// ones that split a character.
fn any_range() -> impl Strategy<Value = Option<Range<usize>>> {
    prop::option::of((0usize..400, 0usize..400).prop_map(|(a, b)| a..b))
}

proptest! {
    #![proptest_config(config(512))]

    #[test]
    fn parse_covers_every_byte(text in tricky_text(120)) {
        check_spans(&text, &parse(&text))?;
    }

    /// Lines are independent outside code fences: parsing a line on its own
    /// styles it the same as parsing it after other non-fence lines.
    #[test]
    fn lines_parse_independently_outside_fences(
        before in "[a-z *_#>-]{0,30}",
        // One line: line breaks become spaces (generated, not filtered, so
        // long runs don't give up on discarded cases).
        line in tricky_text(30).prop_map(|t| t.replace(['\n', '\r'], " ")),
    ) {
        let alone = parse(&line);
        let text = format!("{before}\n{line}");
        let offset = before.len() + 1;
        let after: Vec<Span> = parse(&text)
            .into_iter()
            .filter(|s| s.range.1 > offset)
            .map(|s| Span {
                range: (s.range.0.max(offset) - offset, s.range.1 - offset),
                ..s
            })
            .collect();
        prop_assert_eq!(alone, after);
    }

    #[test]
    fn layout_keeps_every_character(
        text in tricky_text(100),
        enabled in any::<bool>(),
        active in any_range(),
    ) {
        let job = layout(&text, &look(), enabled, active.as_ref());
        prop_assert_eq!(&job.text, &text);
        let mut expect = 0;
        for section in &job.sections {
            let r = section.byte_range.start.0..section.byte_range.end.0;
            prop_assert_eq!(r.start, expect);
            prop_assert!(r.end <= text.len());
            prop_assert!(text.is_char_boundary(r.start) && text.is_char_boundary(r.end));
            prop_assert!(section.format.font_id.size > 0.0);
            prop_assert!(section.format.line_height.is_some_and(|h| h > 0.0));
            expect = r.end;
        }
        prop_assert_eq!(expect, text.len());
    }
}

proptest! {
    #![proptest_config(config(16))]

    /// Very long lines made of one repeated fragment still parse, and
    /// quickly enough to do on every keystroke.
    #[test]
    fn long_repetitive_lines_parse(
        unit in tricky_text(4).prop_map(|u| if u.is_empty() { "*".to_owned() } else { u }),
        repeat in 1000usize..4000,
    ) {
        let text = unit.repeat(repeat);
        let started = Instant::now();
        let spans = parse(&text);
        check_spans(&text, &spans)?;
        prop_assert!(started.elapsed() < Duration::from_secs(2), "{:?} x {}", unit, repeat);
    }
}

/// The worst case for the inline parser: many openers that never close.
/// Each one scans the rest of the line for a closer, so this is quadratic.
fn time_parse(text: &str) -> Duration {
    let started = Instant::now();
    let spans = parse(text);
    let elapsed = started.elapsed();
    assert_eq!(spans.last().map(|s| s.range.1), Some(text.len()));
    elapsed
}

#[test]
fn unclosed_openers_on_a_long_line_parse_in_time() {
    for unit in ["*a ", "[", "_a ", "**a ", "~~a ", "`"] {
        let text = unit.repeat(20_000 / unit.len());
        let elapsed = time_parse(&text);
        assert!(elapsed < Duration::from_secs(2), "{unit:?}: {elapsed:?}");
    }
}

#[test]
fn layout_accepts_degenerate_active_ranges() {
    let text = "é **b** `c`\n# 中";
    #[allow(clippy::reversed_empty_ranges)]
    for active in [
        0..0,
        1..2,
        5..1,
        0..usize::MAX,
        usize::MAX..usize::MAX,
        2..200,
    ] {
        assert_eq!(layout(text, &look(), true, Some(&active)).text, text);
    }
}
