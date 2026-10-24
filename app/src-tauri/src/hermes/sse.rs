//! SSE parser following the rules in M1 §6, tested against `fixtures/`.
//!
//! Incremental: bytes go in as they arrive from the network, complete frames come out. Bytes are
//! buffered until a full line is available, so a chunk boundary inside a multi-byte UTF-8
//! character or inside a line is harmless.

/// One dispatched SSE event: the fields of a block that ended with a blank line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseFrame {
    pub id: Option<String>,
    pub event: Option<String>,
    pub data: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SseItem {
    Frame(SseFrame),
    /// A `:` line. Hermes sends `open`, `keepalive` and `stream closed`.
    Comment(String),
}

impl SseItem {
    /// Hermes's last line before it closes the connection.
    pub fn is_stream_closed(&self) -> bool {
        matches!(self, SseItem::Comment(text) if text == "stream closed")
    }
}

#[derive(Debug, Default)]
pub struct SseParser {
    buffer: Vec<u8>,
    id: Option<String>,
    event: Option<String>,
    data: Vec<String>,
    // True once any field line of the current block has been read; used by `finish`.
    in_block: bool,
}

impl SseParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds raw bytes and returns every item completed by them, in order.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<SseItem> {
        self.buffer.extend_from_slice(bytes);
        let mut items = Vec::new();
        while let Some(newline) = self.buffer.iter().position(|&b| b == b'\n') {
            let mut line: Vec<u8> = self.buffer.drain(..=newline).collect();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            let line = String::from_utf8_lossy(&line).into_owned();
            if let Some(item) = self.process_line(&line) {
                items.push(item);
            }
        }
        items
    }

    /// Ends the stream. Per the SSE standard, a block that never got its closing blank line is
    /// discarded rather than dispatched half-read. Returns true if such a block was dropped, so
    /// the caller can tell a cut connection from a clean end.
    pub fn finish(self) -> bool {
        self.in_block || !self.buffer.is_empty()
    }

    fn process_line(&mut self, line: &str) -> Option<SseItem> {
        if line.is_empty() {
            return self.dispatch();
        }
        if let Some(comment) = line.strip_prefix(':') {
            return Some(SseItem::Comment(
                comment.strip_prefix(' ').unwrap_or(comment).to_string(),
            ));
        }
        let (field, value) = match line.split_once(':') {
            Some((field, value)) => (field, value.strip_prefix(' ').unwrap_or(value)),
            None => (line, ""),
        };
        self.in_block = true;
        match field {
            "data" => self.data.push(value.to_string()),
            "id" => self.id = Some(value.to_string()),
            "event" => self.event = Some(value.to_string()),
            // `retry` and unknown fields carry nothing Kairos uses.
            _ => {}
        }
        None
    }

    fn dispatch(&mut self) -> Option<SseItem> {
        self.in_block = false;
        let id = self.id.take();
        let event = self.event.take();
        if self.data.is_empty() {
            // A block with no data line is not an event (SSE standard); skip it.
            return None;
        }
        let data = std::mem::take(&mut self.data).join("\n");
        Some(SseItem::Frame(SseFrame { id, event, data }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(items: &[SseItem]) -> Vec<&SseFrame> {
        items
            .iter()
            .filter_map(|item| match item {
                SseItem::Frame(frame) => Some(frame),
                SseItem::Comment(_) => None,
            })
            .collect()
    }

    #[test]
    fn comments_are_reported_and_never_become_frames() {
        let items = SseParser::new().push(b": open\n\n: keepalive\n\n: stream closed\n\n");
        assert_eq!(
            items,
            vec![
                SseItem::Comment("open".into()),
                SseItem::Comment("keepalive".into()),
                SseItem::Comment("stream closed".into()),
            ]
        );
        assert!(items[2].is_stream_closed());
    }

    #[test]
    fn a_block_with_an_id_but_no_data_is_skipped() {
        let items = SseParser::new().push(b"id: 7\n\nid: 8\ndata: {}\n\n");
        let frames = frames(&items);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].id.as_deref(), Some("8"));
    }

    #[test]
    fn crlf_line_endings_parse_like_lf() {
        let items = SseParser::new().push(b"id: 1\r\ndata: {\"a\":1}\r\n\r\n");
        assert_eq!(frames(&items)[0].data, "{\"a\":1}");
    }

    #[test]
    fn a_frame_split_across_chunks_arrives_once_complete() {
        let mut parser = SseParser::new();
        assert!(parser.push(b"id: 1\nda").is_empty());
        assert!(parser.push(b"ta: {\"x\":").is_empty());
        let items = parser.push(b"1}\n\n");
        assert_eq!(frames(&items)[0].data, "{\"x\":1}");
    }

    #[test]
    fn a_multibyte_character_split_across_chunks_is_not_corrupted() {
        let text = "data: \u{2014}\n\n".as_bytes();
        let mut parser = SseParser::new();
        // The em dash is 3 bytes; cut it in the middle.
        assert!(parser.push(&text[..7]).is_empty());
        let items = parser.push(&text[7..]);
        assert_eq!(frames(&items)[0].data, "\u{2014}");
    }

    #[test]
    fn multiple_data_lines_join_with_newlines() {
        let items = SseParser::new().push(b"data: one\ndata: two\n\n");
        assert_eq!(frames(&items)[0].data, "one\ntwo");
    }

    #[test]
    fn a_stream_cut_mid_event_drops_the_partial_event_and_says_so() {
        let mut parser = SseParser::new();
        let items = parser.push(b"id: 1\ndata: {\"event\": \"run.completed\"");
        assert!(frames(&items).is_empty());
        assert!(
            parser.finish(),
            "finish must report the dropped partial event"
        );
    }

    #[test]
    fn a_clean_end_reports_nothing_dropped() {
        let mut parser = SseParser::new();
        parser.push(b"id: 1\ndata: {}\n\n: stream closed\n\n");
        assert!(!parser.finish());
    }
}
