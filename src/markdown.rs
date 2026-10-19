//! Live markdown styling for the editor, in the manner of Notion or
//! Obsidian's live preview: the text stays plain markdown, but headings,
//! emphasis, quotes, lists and code are drawn styled as you type. Syntax
//! markers (`#`, `**`, `_`…) fade out, except on the line being edited.
//!
//! The editor needs a layout containing every character of the text, so
//! markers are never removed, only drawn invisibly at near-zero width.

use std::ops::Range;

use eframe::egui::{Color32, FontId, Stroke, TextFormat, text::LayoutJob};
use eframe::epaint::text::VariationCoords;

use crate::fonts::WritingFonts;

/// What part of the markdown a span of text is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Role {
    #[default]
    Text,
    /// Syntax that disappears when not editing its line (`#`, `**`, `[`…).
    Marker,
    /// List bullets and numbers: always visible, but quiet.
    ListMarker,
    /// Inline code or a line inside a fenced code block.
    Code,
    /// A ``` fence line or a horizontal rule.
    Fence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)] // independent inline flags
pub struct Style {
    pub role: Role,
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub link: bool,
    pub quote: bool,
    /// Heading level 1–6, or 0 for body text.
    pub heading: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub range: (usize, usize),
    pub style: Style,
}

/// Splits `text` into styled spans that together cover every byte.
pub fn parse(text: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut in_fence = false;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let body = line.strip_suffix('\n').unwrap_or(line);
        let newline = line.len() - body.len();
        let base = parse_line(body, offset, &mut in_fence, &mut spans);
        if newline > 0 {
            push(&mut spans, offset + body.len(), offset + line.len(), base);
        }
        offset += line.len();
    }
    spans
}

fn push(spans: &mut Vec<Span>, start: usize, end: usize, style: Style) {
    if start >= end {
        return;
    }
    if let Some(last) = spans.last_mut()
        && last.range.1 == start
        && last.style == style
    {
        last.range.1 = end;
        return;
    }
    spans.push(Span {
        range: (start, end),
        style,
    });
}

/// Parses one line (without its newline); returns the line's base style,
/// which its newline inherits so the row keeps its height.
fn parse_line(line: &str, at: usize, in_fence: &mut bool, out: &mut Vec<Span>) -> Style {
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();

    if trimmed.starts_with("```") {
        *in_fence = !*in_fence;
        let style = Style {
            role: Role::Fence,
            ..Style::default()
        };
        push(out, at, at + line.len(), style);
        return style;
    }
    if *in_fence {
        let style = Style {
            role: Role::Code,
            ..Style::default()
        };
        push(out, at, at + line.len(), style);
        return style;
    }
    if is_rule(trimmed) {
        let style = Style {
            role: Role::Fence,
            ..Style::default()
        };
        push(out, at, at + line.len(), style);
        return style;
    }

    // Headings: `# ` … `###### `.
    let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
    if indent == 0 && (1..=6).contains(&hashes) {
        let after = &trimmed[hashes..];
        if after.is_empty() || after.starts_with(' ') {
            let marker_len = hashes + usize::from(after.starts_with(' '));
            let base = Style {
                heading: hashes as u8,
                bold: true,
                ..Style::default()
            };
            push(
                out,
                at,
                at + marker_len,
                Style {
                    role: Role::Marker,
                    ..base
                },
            );
            inline(&line[marker_len..], at + marker_len, base, out);
            return base;
        }
    }

    // Block quotes: `> `.
    if let Some(rest) = trimmed.strip_prefix('>') {
        let marker_len = indent + 1 + usize::from(rest.starts_with(' '));
        let base = Style {
            quote: true,
            italic: true,
            ..Style::default()
        };
        push(
            out,
            at,
            at + marker_len,
            Style {
                role: Role::Marker,
                ..base
            },
        );
        inline(&line[marker_len..], at + marker_len, base, out);
        return base;
    }

    // List items: `- `, `* `, `+ `, `1. `, `1) `.
    if let Some(marker_len) = list_marker(trimmed) {
        let end = indent + marker_len;
        push(
            out,
            at,
            at + end,
            Style {
                role: Role::ListMarker,
                ..Style::default()
            },
        );
        inline(&line[end..], at + end, Style::default(), out);
        return Style::default();
    }

    inline(line, at, Style::default(), out);
    Style::default()
}

fn is_rule(trimmed: &str) -> bool {
    let compact: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
    compact.len() >= 3
        && ['-', '*', '_']
            .iter()
            .any(|&c| compact.chars().all(|x| x == c))
}

fn list_marker(trimmed: &str) -> Option<usize> {
    let bytes = trimmed.as_bytes();
    match bytes.first()? {
        b'-' | b'*' | b'+' if bytes.get(1) == Some(&b' ') => Some(2),
        b'0'..=b'9' => {
            let digits = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
            let punct = *bytes.get(digits)?;
            (digits <= 3
                && (punct == b'.' || punct == b')')
                && bytes.get(digits + 1) == Some(&b' '))
            .then_some(digits + 2)
        }
        _ => None,
    }
}

/// Sets the style flag a paired delimiter turns on.
type Apply = fn(&mut Style);

/// Parses inline markup in `s` (starting at byte `at` of the document).
fn inline(s: &str, at: usize, base: Style, out: &mut Vec<Span>) {
    let marker = Style {
        role: Role::Marker,
        ..base
    };
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut plain_start = 0;
    let flush = |out: &mut Vec<Span>, from: usize, to: usize| push(out, at + from, at + to, base);
    while i < bytes.len() {
        let rest = &s[i..];
        let word_before = i > 0 && is_word(s[..i].chars().next_back());

        // `code`
        if rest.starts_with('`')
            && let Some(len) = rest[1..].find('`')
            && len > 0
        {
            flush(out, plain_start, i);
            push(out, at + i, at + i + 1, marker);
            let code = Style {
                role: Role::Code,
                ..base
            };
            push(out, at + i + 1, at + i + 1 + len, code);
            push(out, at + i + 1 + len, at + i + 2 + len, marker);
            i += len + 2;
            plain_start = i;
            continue;
        }
        // **bold**, __bold__, ~~strike~~
        let doubles: [(&str, Apply); 3] = [
            ("**", |s| s.bold = true),
            ("__", |s| s.bold = true),
            ("~~", |s| s.strike = true),
        ];
        if let Some((delim, apply)) = doubles.iter().find(|(d, _)| rest.starts_with(d))
            && !(delim.starts_with('_') && word_before)
            && let Some(len) = find_closing(&rest[2..], delim)
        {
            flush(out, plain_start, i);
            let mut inner = base;
            apply(&mut inner);
            let edge = Style {
                role: Role::Marker,
                ..inner
            };
            push(out, at + i, at + i + 2, edge);
            inline(&rest[2..2 + len], at + i + 2, inner, out);
            push(out, at + i + 2 + len, at + i + 4 + len, edge);
            i += len + 4;
            plain_start = i;
            continue;
        }
        // *italic*, _italic_
        if let Some(delim) = ["*", "_"].into_iter().find(|d| rest.starts_with(d))
            && !rest[1..].starts_with(delim)
            && !(delim == "_" && word_before)
            && let Some(len) = find_closing(&rest[1..], delim)
            && !(delim == "_" && is_word(rest[1 + len + 1..].chars().next()))
        {
            flush(out, plain_start, i);
            push(out, at + i, at + i + 1, marker);
            inline(
                &rest[1..=len],
                at + i + 1,
                Style {
                    italic: true,
                    ..base
                },
                out,
            );
            push(out, at + i + 1 + len, at + i + 2 + len, marker);
            i += len + 2;
            plain_start = i;
            continue;
        }
        // [text](url)
        if rest.starts_with('[')
            && let Some(close) = rest.find("](")
            && close > 1
            && let Some(end) = rest[close + 2..].find(')')
        {
            flush(out, plain_start, i);
            push(out, at + i, at + i + 1, marker);
            inline(
                &rest[1..close],
                at + i + 1,
                Style { link: true, ..base },
                out,
            );
            let url_end = i + close + 2 + end + 1;
            push(out, at + i + close, at + url_end, marker);
            i = url_end;
            plain_start = i;
            continue;
        }
        i += rest.chars().next().map_or(1, char::len_utf8);
    }
    flush(out, plain_start, bytes.len());
}

fn is_word(c: Option<char>) -> bool {
    c.is_some_and(char::is_alphanumeric)
}

/// Length of the content before the closing `delim`, which must not be
/// empty or start/end with a space (so `2 * 3 * 4` stays plain).
fn find_closing(s: &str, delim: &str) -> Option<usize> {
    if s.starts_with(' ') || s.starts_with(delim) {
        return None;
    }
    let mut from = 0;
    while let Some(pos) = s[from..].find(delim) {
        let len = from + pos;
        // A single `*` must not be half of a `**`.
        let doubled = delim.len() == 1 && s[len + 1..].starts_with(delim);
        if len > 0 && !s[..len].ends_with(' ') && !doubled {
            return Some(len);
        }
        from = len + delim.len() + usize::from(doubled);
    }
    None
}

// ----- layout ------------------------------------------------------------------

/// Colours and sizes for drawing markdown.
#[derive(Debug, Clone)]
pub struct Look {
    pub fonts: WritingFonts,
    pub size: f32,
    pub line_spacing: f32,
    pub ink: Color32,
    pub accent: Color32,
}

const HEADING_SCALE: [f32; 7] = [1.0, 1.7, 1.38, 1.18, 1.06, 1.0, 1.0];

/// Builds the editor layout for `text`. With `enabled` false everything is
/// plain body text. `active` is the byte range of the line being edited,
/// whose markers stay visible.
pub fn layout(text: &str, look: &Look, enabled: bool, active: Option<&Range<usize>>) -> LayoutJob {
    let body = TextFormat {
        font_id: look.fonts.body.clone(),
        coords: look.fonts.body_coords.clone(),
        color: look.ink,
        line_height: Some(look.size * look.line_spacing),
        ..TextFormat::default()
    };
    if !enabled {
        return LayoutJob::single_section(text.to_owned(), body);
    }
    let mut job = LayoutJob::default();
    for span in parse(text) {
        let (start, end) = span.range;
        let editing = active.is_some_and(|line| start >= line.start && start <= line.end);
        job.append(
            &text[start..end],
            0.0,
            format(span.style, look, &body, editing),
        );
    }
    job
}

fn format(style: Style, look: &Look, body: &TextFormat, editing: bool) -> TextFormat {
    let scale = HEADING_SCALE[usize::from(style.heading.min(6))];
    let size = look.size * scale;
    let line_height = if style.heading > 0 {
        size * 1.35
    } else {
        look.size * look.line_spacing
    };
    let (font, coords) = if style.bold {
        (look.fonts.bold.clone(), look.fonts.bold_coords.clone())
    } else {
        (look.fonts.body.clone(), look.fonts.body_coords.clone())
    };
    let mut f = TextFormat {
        font_id: FontId::new(size, font.family),
        coords,
        line_height: Some(line_height),
        italics: style.italic,
        color: if style.quote {
            look.ink.gamma_multiply(0.72)
        } else {
            look.ink
        },
        ..body.clone()
    };
    if style.strike {
        f.color = look.ink.gamma_multiply(0.55);
        f.strikethrough = Stroke::new(1.0, f.color);
    }
    if style.link {
        f.color = look.accent;
        f.underline = Stroke::new(1.0, look.accent.gamma_multiply(0.5));
    }
    match style.role {
        Role::Text => {}
        Role::Marker => {
            if editing {
                f.color = look.ink.gamma_multiply(0.32);
                f.italics = false;
                f.underline = Stroke::NONE;
                f.strikethrough = Stroke::NONE;
            } else {
                // Invisible and almost zero-width, keeping the row height.
                f.color = Color32::TRANSPARENT;
                f.font_id = FontId::new(1.0, f.font_id.family);
                f.underline = Stroke::NONE;
                f.strikethrough = Stroke::NONE;
            }
        }
        Role::ListMarker => f.color = look.ink.gamma_multiply(0.5),
        Role::Code => {
            f.font_id = FontId::new(look.fonts.mono.size * scale, look.fonts.mono.family.clone());
            f.coords = VariationCoords::default();
            f.color = look.ink.gamma_multiply(0.85);
            f.background = look.ink.gamma_multiply(0.07);
            f.italics = false;
        }
        Role::Fence => {
            f.font_id = look.fonts.mono.clone();
            f.coords = VariationCoords::default();
            f.color = look.ink.gamma_multiply(0.35);
        }
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (text, role, bold, italic) for each span, for readable assertions.
    fn spans(text: &str) -> Vec<(&str, Role, bool, bool)> {
        parse(text)
            .into_iter()
            .map(|s| {
                (
                    &text[s.range.0..s.range.1],
                    s.style.role,
                    s.style.bold,
                    s.style.italic,
                )
            })
            .collect()
    }

    #[test]
    fn spans_cover_every_byte_in_order() {
        let text = "# Title\n\nSome **bold** and *it* `code` [l](u)\n- item\n> quote\n```\nx\n```\n---\nend";
        let mut expect = 0;
        for s in parse(text) {
            assert_eq!(s.range.0, expect);
            assert!(s.range.1 > s.range.0);
            expect = s.range.1;
        }
        assert_eq!(expect, text.len());
    }

    #[test]
    fn headings_need_a_space_and_hide_their_hashes() {
        let s = parse("## Hello");
        assert_eq!(s[0].style.role, Role::Marker);
        assert_eq!(s[0].range, (0, 3));
        assert_eq!(s[1].style.heading, 2);
        assert_eq!(parse("#hashtag")[0].style.heading, 0);
        assert_eq!(parse("#######x")[0].style.heading, 0);
    }

    #[test]
    fn emphasis_and_code() {
        assert_eq!(
            spans("a **b** c"),
            [
                ("a ", Role::Text, false, false),
                ("**", Role::Marker, true, false),
                ("b", Role::Text, true, false),
                ("**", Role::Marker, true, false),
                (" c", Role::Text, false, false),
            ]
        );
        assert_eq!(spans("*i*")[1], ("i", Role::Text, false, true));
        assert!(spans("**b *bi* b**").contains(&("bi", Role::Text, true, true)));
        assert_eq!(spans("`x*y*`")[1], ("x*y*", Role::Code, false, false));
    }

    #[test]
    fn things_that_are_not_markup_stay_plain() {
        for text in [
            "2 * 3 * 4",
            "snake_case_name",
            "** not bold**",
            "a*",
            "[x]",
            "_ x_",
        ] {
            assert!(
                parse(text)
                    .iter()
                    .all(|s| s.style.role == Role::Text && !s.style.italic),
                "{text}"
            );
        }
    }

    #[test]
    fn lists_quotes_rules_and_fences() {
        assert_eq!(spans("- a")[0], ("- ", Role::ListMarker, false, false));
        assert_eq!(spans("12. a")[0], ("12. ", Role::ListMarker, false, false));
        assert!(parse("> q").iter().any(|s| s.style.quote));
        assert_eq!(parse("---")[0].style.role, Role::Fence);
        let fenced = parse("```\n**x**\n```");
        assert!(fenced.iter().any(|s| s.style.role == Role::Code));
        assert!(fenced.iter().all(|s| s.style.role != Role::Marker));
    }

    #[test]
    fn links_style_the_text_and_hide_the_url() {
        let s = parse("see [docs](https://x.y) now");
        let link: Vec<_> = s.iter().filter(|s| s.style.link).collect();
        assert!(link.iter().any(|s| s.style.role == Role::Text));
        let url = s.iter().find(|s| s.range == (9, 23)).expect("url marker");
        assert_eq!(url.style.role, Role::Marker);
    }

    #[test]
    fn layout_keeps_all_text_whether_enabled_or_not() {
        let look = Look {
            fonts: crate::fonts::TYPEFACES[0].writing_fonts(16.0),
            size: 16.0,
            line_spacing: 1.6,
            ink: Color32::BLACK,
            accent: Color32::BLUE,
        };
        let text = "# A\n**b** _c_";
        for enabled in [true, false] {
            assert_eq!(layout(text, &look, enabled, Some(&(0..3))).text, text);
        }
    }
}
