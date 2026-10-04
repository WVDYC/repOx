//! Cached line index for the preview pane.
//!
//! Building the index is `O(bytes)` and happens once per hovered file. After
//! that every frame only slices the handful of lines that are on screen.

/// Preview at most this many bytes of a file (cut on a `char` boundary).
pub const MAX_PREVIEW_BYTES: usize = 256 * 1024;

#[derive(Debug, Default)]
pub struct Preview {
    /// Node the cache was built for.
    node: Option<usize>,
    /// Byte offset of the start of every line.
    line_starts: Vec<u32>,
    /// Exclusive byte end of the previewed region.
    end: usize,
    /// `true` when the file was cut at [`MAX_PREVIEW_BYTES`].
    truncated: bool,
}

impl Preview {
    #[inline]
    pub fn node(&self) -> Option<usize> {
        self.node
    }

    /// Resets the cache for a node that has no text to show (e.g. a folder).
    pub fn clear_for(&mut self, node: Option<usize>) {
        self.node = node;
        self.line_starts.clear();
        self.end = 0;
        self.truncated = false;
    }

    /// (Re)builds the index for `content`, reusing the existing allocation.
    pub fn load(&mut self, node: usize, content: &str) {
        self.node = Some(node);
        self.line_starts.clear();

        let mut end = content.len().min(MAX_PREVIEW_BYTES);
        while end > 0 && !content.is_char_boundary(end) {
            end -= 1;
        }
        self.end = end;
        self.truncated = end < content.len();

        if end == 0 {
            return;
        }
        self.line_starts.push(0);
        for (i, byte) in content.as_bytes()[..end].iter().enumerate() {
            // A trailing newline does not start another (empty) line.
            if *byte == b'\n' && i + 1 < end {
                // `end <= 256 KiB`, so this always fits in a u32.
                self.line_starts
                    .push(u32::try_from(i + 1).unwrap_or(u32::MAX));
            }
        }
    }

    #[inline]
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    #[inline]
    pub fn truncated(&self) -> bool {
        self.truncated
    }

    /// Text of line `i` without its line terminator.
    pub fn line<'a>(&self, content: &'a str, i: usize) -> Option<&'a str> {
        let start = *self.line_starts.get(i)? as usize;
        let end = self
            .line_starts
            .get(i + 1)
            .map_or(self.end, |next| *next as usize);
        let line = content.get(start..end)?;
        Some(line.trim_end_matches(['\n', '\r']))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_lines_without_a_phantom_trailing_line() {
        let content = "alpha\nbeta\r\ngamma\n";
        let mut p = Preview::default();
        p.load(3, content);
        assert_eq!(p.node(), Some(3));
        assert_eq!(p.line_count(), 3);
        assert_eq!(p.line(content, 0), Some("alpha"));
        assert_eq!(p.line(content, 1), Some("beta"));
        assert_eq!(p.line(content, 2), Some("gamma"));
        assert_eq!(p.line(content, 3), None);
        assert!(!p.truncated());
    }

    #[test]
    fn handles_missing_trailing_newline_and_empty_files() {
        let mut p = Preview::default();
        p.load(0, "one\ntwo");
        assert_eq!(p.line_count(), 2);
        assert_eq!(p.line("one\ntwo", 1), Some("two"));

        p.load(1, "");
        assert_eq!(p.line_count(), 0);
        assert_eq!(p.line("", 0), None);
    }

    #[test]
    fn keeps_blank_lines() {
        let content = "a\n\nb\n";
        let mut p = Preview::default();
        p.load(0, content);
        assert_eq!(p.line_count(), 3);
        assert_eq!(p.line(content, 1), Some(""));
    }

    #[test]
    fn truncates_large_files_on_a_char_boundary() {
        // 'é' is 2 bytes; make the cut land in the middle of one.
        let content = "é".repeat(MAX_PREVIEW_BYTES);
        let mut p = Preview::default();
        p.load(0, &content);
        assert!(p.truncated());
        assert_eq!(p.line_count(), 1);
        let line = p.line(&content, 0);
        assert!(line.is_some_and(|l| l.len() <= MAX_PREVIEW_BYTES && l.chars().all(|c| c == 'é')));
    }

    #[test]
    fn clear_resets_everything() {
        let mut p = Preview::default();
        p.load(7, "x\ny\n");
        p.clear_for(Some(9));
        assert_eq!(p.node(), Some(9));
        assert_eq!(p.line_count(), 0);
        assert!(!p.truncated());
    }
}
