//! Turn terminal-oriented output into data the browser renders safely.

use pulldown_cmark::{html, Event, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use serde::Serialize;
use ts_rs::TS;

/// One run of text with the color that `erd` gave it.
#[derive(Serialize, TS)]
pub struct StyledSegment {
    pub text: String,
    /// A CSS color value, or `None` for the default text color.
    pub color: Option<String>,
    pub bold: bool,
}

/// Parse ANSI output (from `erd --color force`) into styled lines.
pub fn tree_lines(output: &str, max_lines: usize) -> Vec<Vec<StyledSegment>> {
    gator::text::lines_from_ansi_output(output, Style::default(), max_lines)
        .into_iter()
        .map(|line| {
            line.spans
                .into_iter()
                .filter(|span| !span.content.is_empty())
                .map(|span| StyledSegment {
                    text: span.content.into_owned(),
                    color: span.style.fg.and_then(css_color),
                    bold: span.style.add_modifier.contains(Modifier::BOLD),
                })
                .collect()
        })
        .collect()
}

/// The 16 named ANSI colors map to `--ansi-*` variables so the theme decides them.
fn css_color(color: Color) -> Option<String> {
    let named = |name: &str| Some(format!("var(--ansi-{name})"));
    match color {
        Color::Reset => None,
        Color::Black => named("black"),
        Color::Red => named("red"),
        Color::Green => named("green"),
        Color::Yellow => named("yellow"),
        Color::Blue => named("blue"),
        Color::Magenta => named("magenta"),
        Color::Cyan => named("cyan"),
        Color::Gray => named("white"),
        Color::DarkGray => named("bright-black"),
        Color::LightRed => named("bright-red"),
        Color::LightGreen => named("bright-green"),
        Color::LightYellow => named("bright-yellow"),
        Color::LightBlue => named("bright-blue"),
        Color::LightMagenta => named("bright-magenta"),
        Color::LightCyan => named("bright-cyan"),
        Color::White => named("bright-white"),
        Color::Rgb(red, green, blue) => Some(format!("#{red:02x}{green:02x}{blue:02x}")),
        Color::Indexed(index) => Some(indexed_color(index)),
    }
}

/// xterm 256-color palette for indexes past the 16 named colors.
fn indexed_color(index: u8) -> String {
    const NAMED: [&str; 16] = [
        "black",
        "red",
        "green",
        "yellow",
        "blue",
        "magenta",
        "cyan",
        "white",
        "bright-black",
        "bright-red",
        "bright-green",
        "bright-yellow",
        "bright-blue",
        "bright-magenta",
        "bright-cyan",
        "bright-white",
    ];
    match index {
        0..=15 => format!("var(--ansi-{})", NAMED[usize::from(index)]),
        16..=231 => {
            let cube = index - 16;
            let level = |value: u8| if value == 0 { 0 } else { 55 + value * 40 };
            format!(
                "#{:02x}{:02x}{:02x}",
                level(cube / 36),
                level((cube / 6) % 6),
                level(cube % 6)
            )
        }
        232..=255 => {
            let gray = 8 + (index - 232) * 10;
            format!("#{gray:02x}{gray:02x}{gray:02x}")
        }
    }
}

/// Render README markdown to HTML for direct insertion in the page.
/// Raw HTML is dropped and only http(s), mailto and in-page links survive,
/// because the page can call the local API that runs commands.
pub fn markdown_to_html(markdown: &str) -> String {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_GFM;
    let mut dropped_links = Vec::new();
    let mut dropped_images = Vec::new();
    let events = Parser::new_ext(markdown, options).filter_map(|event| match event {
        Event::Html(_) | Event::InlineHtml(_) => None,
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            let keep = safe_url(&dest_url, true);
            dropped_links.push(!keep);
            keep.then_some(Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }))
        }
        Event::End(TagEnd::Link) => {
            let dropped = dropped_links.pop().expect("link end follows link start");
            (!dropped).then_some(Event::End(TagEnd::Link))
        }
        Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            // An unsafe image keeps its alt text as plain text.
            let keep = safe_url(&dest_url, false);
            dropped_images.push(!keep);
            keep.then_some(Event::Start(Tag::Image {
                link_type,
                dest_url,
                title,
                id,
            }))
        }
        Event::End(TagEnd::Image) => {
            let dropped = dropped_images.pop().expect("image end follows image start");
            (!dropped).then_some(Event::End(TagEnd::Image))
        }
        other => Some(other),
    });
    let mut out = String::new();
    html::push_html(&mut out, events);
    out
}

fn safe_url(url: &str, allow_page_links: bool) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    lower.starts_with("https://")
        || lower.starts_with("http://")
        || (allow_page_links && (lower.starts_with("mailto:") || lower.starts_with('#')))
}

#[cfg(test)]
mod tests {
    use super::{indexed_color, markdown_to_html};

    #[test]
    fn markdown_drops_raw_html_and_script_links() {
        let html = markdown_to_html(
            "# Title\n\n<script>alert(1)</script>\n\n[bad](javascript:alert(1)) [good](https://example.com)",
        );
        assert!(html.contains("<h1>Title</h1>"));
        assert!(!html.contains("<script"));
        assert!(!html.contains("javascript:"));
        assert!(html.contains("bad"));
        assert!(html.contains(r#"<a href="https://example.com">good</a>"#));
    }

    #[test]
    fn markdown_keeps_alt_text_of_relative_images() {
        let html =
            markdown_to_html("![Screenshot](docs/shot.png) ![Badge](https://img.example/b.svg)");
        assert!(html.contains("Screenshot"));
        assert!(!html.contains("docs/shot.png"));
        assert!(html.contains(r#"<img src="https://img.example/b.svg" alt="Badge" />"#));
    }

    #[test]
    fn xterm_palette_covers_cube_and_grays() {
        assert_eq!(indexed_color(16), "#000000");
        assert_eq!(indexed_color(231), "#ffffff");
        assert_eq!(indexed_color(232), "#080808");
        assert_eq!(indexed_color(1), "var(--ansi-red)");
    }
}
