//! TTML parser for lyrics.

use crate::{LyricLine, LyricWord, Lyrics};
use monad_core::Error;
use quick_xml::events::Event;
use quick_xml::Reader;
use tracing::debug;

/// Parse TTML lyrics into our Lyrics structure.
pub fn parse_ttml(ttml: &str, artist: &str, song: &str) -> Result<Lyrics, Error> {
    let mut reader = Reader::from_str(ttml);
    // Don't trim text - we need spaces between spans
    reader.config_mut().trim_text(false);

    let mut lines = Vec::new();
    let mut duration = None;

    // Parse the body duration if present
    if let Some(dur_str) = extract_body_duration(ttml) {
        duration = parse_duration(&dur_str);
    }

    let mut buf = Vec::new();
    let mut current_line: Option<TtmlLine> = None;
    let mut in_span = false;
    let mut current_word = TtmlWord::default();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = e.name();
                let name_str = std::str::from_utf8(name.as_ref()).unwrap_or("");

                match name_str {
                    "p" => {
                        // Start of a lyric line
                        let mut line = TtmlLine::default();
                        for attr in e.attributes().flatten() {
                            let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                            let value = std::str::from_utf8(&attr.value).unwrap_or("");
                            match key {
                                "begin" => line.start = parse_time(value),
                                "end" => line.end = parse_time(value),
                                _ => {}
                            }
                        }
                        current_line = Some(line);
                    }
                    "span" => {
                        // Start of a word
                        in_span = true;
                        current_word = TtmlWord::default();
                        for attr in e.attributes().flatten() {
                            let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
                            let value = std::str::from_utf8(&attr.value).unwrap_or("");
                            match key {
                                "begin" => current_word.start = parse_time(value),
                                "end" => current_word.end = parse_time(value),
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                if in_span {
                    current_word.text.push_str(&text);
                } else if let Some(ref mut line) = current_line {
                    // Text between spans (spaces) or directly in <p>
                    // Only add if it contains a space (not just newlines/indentation)
                    if text.contains(' ') {
                        line.text.push(' ');
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name = e.name();
                let name_str = std::str::from_utf8(name.as_ref()).unwrap_or("");

                match name_str {
                    "span" => {
                        in_span = false;
                        if let Some(ref mut line) = current_line {
                            if !current_word.text.is_empty() {
                                // Don't add space - spaces come from text nodes between spans
                                line.text.push_str(&current_word.text);
                                line.words.push(LyricWord {
                                    text: current_word.text.clone(),
                                    start: current_word.start,
                                    end: current_word.end,
                                });
                            }
                        }
                        current_word = TtmlWord::default();
                    }
                    "p" => {
                        if let Some(line) = current_line.take() {
                            if !line.text.trim().is_empty() {
                                lines.push(LyricLine {
                                    text: line.text.trim().to_string(),
                                    start: line.start,
                                    end: line.end,
                                    words: line.words,
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                debug!("XML parse error: {}", e);
                // Continue parsing, some errors are recoverable
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(Lyrics {
        title: song.to_string(),
        artist: artist.to_string(),
        duration,
        lines,
    })
}

/// Temporary struct for parsing a line.
#[derive(Default)]
struct TtmlLine {
    text: String,
    start: f64,
    end: f64,
    words: Vec<LyricWord>,
}

/// Temporary struct for parsing a word.
#[derive(Default)]
struct TtmlWord {
    text: String,
    start: f64,
    end: f64,
}

/// Parse a time string like "9.731" or "3:53.713" into seconds.
fn parse_time(s: &str) -> f64 {
    // Handle mm:ss.ms format
    if let Some((mins, rest)) = s.split_once(':') {
        let minutes: f64 = mins.parse().unwrap_or(0.0);
        let seconds: f64 = rest.parse().unwrap_or(0.0);
        return minutes * 60.0 + seconds;
    }

    // Handle simple seconds format
    s.parse().unwrap_or(0.0)
}

/// Parse a duration string like "3:53.713" into seconds.
fn parse_duration(s: &str) -> Option<f64> {
    let time = parse_time(s);
    if time > 0.0 {
        Some(time)
    } else {
        None
    }
}

/// Extract the body duration from TTML using simple string matching.
fn extract_body_duration(ttml: &str) -> Option<String> {
    // Look for dur="..." in the body tag
    let body_start = ttml.find("<body")?;
    let body_end = ttml[body_start..].find('>')? + body_start;
    let body_tag = &ttml[body_start..=body_end];

    // Find dur attribute
    let dur_start = body_tag.find("dur=\"")? + 5;
    let dur_end = body_tag[dur_start..].find('"')? + dur_start;
    Some(body_tag[dur_start..dur_end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_time() {
        assert!((parse_time("9.731") - 9.731).abs() < 0.001);
        assert!((parse_time("3:53.713") - 233.713).abs() < 0.001);
        assert!((parse_time("0") - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_simple_ttml() {
        let ttml = r#"
        <tt xmlns="http://www.w3.org/ns/ttml">
            <body dur="1:30.000">
                <div>
                    <p begin="0.5" end="3.0">
                        <span begin="0.5" end="1.0">Hello</span> <span begin="1.0" end="2.0">world</span>
                    </p>
                </div>
            </body>
        </tt>
        "#;

        let lyrics = parse_ttml(ttml, "Test Artist", "Test Song").unwrap();
        assert_eq!(lyrics.lines.len(), 1);
        assert_eq!(lyrics.lines[0].text, "Hello world");
        assert_eq!(lyrics.lines[0].words.len(), 2);
        assert_eq!(lyrics.lines[0].words[0].text, "Hello");
    }

    #[test]
    fn test_parse_split_word() {
        // Test case where a word is split across spans (like "bo" + "dy" = "body")
        let ttml = r#"
        <tt xmlns="http://www.w3.org/ns/ttml">
            <body dur="1:00.000">
                <div>
                    <p begin="0.5" end="3.0">
                        <span begin="0.5" end="0.8">I'm</span> <span begin="0.8" end="1.0">in</span> <span begin="1.0" end="1.2">love</span> <span begin="1.2" end="1.4">with</span> <span begin="1.4" end="1.6">your</span> <span begin="1.6" end="1.8">bo</span><span begin="1.8" end="2.0">dy</span>
                    </p>
                </div>
            </body>
        </tt>
        "#;

        let lyrics = parse_ttml(ttml, "Test Artist", "Test Song").unwrap();
        assert_eq!(lyrics.lines.len(), 1);
        assert_eq!(lyrics.lines[0].text, "I'm in love with your body");
    }
}
