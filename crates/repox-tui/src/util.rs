//! Allocation-free formatting helpers used by the render loop.
//!
//! Everything here writes into a fixed-size stack buffer so that drawing a
//! frame never touches the heap.

use std::fmt::{self, Write};

/// A fixed-capacity, stack-allocated string buffer.
///
/// Writes that do not fit are truncated on a `char` boundary instead of
/// failing, which is the desired behaviour for UI labels.
#[derive(Clone)]
pub struct StackBuf<const N: usize> {
    buf: [u8; N],
    len: usize,
}

impl<const N: usize> StackBuf<N> {
    #[inline]
    pub const fn new() -> Self {
        Self {
            buf: [0; N],
            len: 0,
        }
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        // The buffer only ever receives whole `char`s (see `write_str`), so this
        // never fails; the fallback keeps the function total without `unwrap`.
        std::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }

    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }
}

impl<const N: usize> Default for StackBuf<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Write for StackBuf<N> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let room = N - self.len;
        let take = if s.len() <= room {
            s.len()
        } else {
            // Truncate on a char boundary.
            let mut cut = room;
            while cut > 0 && !s.is_char_boundary(cut) {
                cut -= 1;
            }
            cut
        };
        self.buf[self.len..self.len + take].copy_from_slice(&s.as_bytes()[..take]);
        self.len += take;
        Ok(())
    }
}

/// Writes `n` with thousands separators: `18420` -> `18,420`.
pub fn write_grouped(w: &mut impl Write, mut n: u64) -> fmt::Result {
    let mut digits = [0u8; 20];
    let mut count = 0;
    loop {
        digits[count] = b'0' + (n % 10) as u8;
        count += 1;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    for i in (0..count).rev() {
        w.write_char(char::from(digits[i]))?;
        if i > 0 && i % 3 == 0 {
            w.write_char(',')?;
        }
    }
    Ok(())
}

/// Writes a compact count: `245`, `1.2k`, `18k`, `1.2M`.
pub fn write_compact(w: &mut impl Write, n: u64) -> fmt::Result {
    if n < 1_000 {
        write!(w, "{n}")
    } else if n < 10_000 {
        write!(w, "{}.{}k", n / 1_000, (n % 1_000) / 100)
    } else if n < 1_000_000 {
        write!(w, "{}k", n / 1_000)
    } else {
        write!(w, "{}.{}M", n / 1_000_000, (n % 1_000_000) / 100_000)
    }
}

/// Writes a human-readable byte size: `512 B`, `124.5 KB`, `3.2 MB`.
pub fn write_size(w: &mut impl Write, bytes: u64) -> fmt::Result {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if bytes < 1024 {
        write!(w, "{bytes} B")
    } else if b < MB {
        write!(w, "{:.1} KB", b / KB)
    } else if b < GB {
        write!(w, "{:.1} MB", b / MB)
    } else {
        write!(w, "{:.2} GB", b / GB)
    }
}

/// Number of decimal digits in `n` (at least 1).
pub fn digits(mut n: usize) -> usize {
    let mut d = 1;
    while n >= 10 {
        n /= 10;
        d += 1;
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt_with(f: impl FnOnce(&mut StackBuf<64>) -> fmt::Result) -> String {
        let mut b = StackBuf::<64>::new();
        let _ = f(&mut b);
        b.as_str().to_owned()
    }

    #[test]
    fn grouped_numbers() {
        assert_eq!(fmt_with(|w| write_grouped(w, 0)), "0");
        assert_eq!(fmt_with(|w| write_grouped(w, 999)), "999");
        assert_eq!(fmt_with(|w| write_grouped(w, 1_000)), "1,000");
        assert_eq!(fmt_with(|w| write_grouped(w, 18_420)), "18,420");
        assert_eq!(fmt_with(|w| write_grouped(w, 200_000)), "200,000");
        assert_eq!(fmt_with(|w| write_grouped(w, 1_234_567)), "1,234,567");
        assert_eq!(
            fmt_with(|w| write_grouped(w, u64::MAX)),
            "18,446,744,073,709,551,615"
        );
    }

    #[test]
    fn compact_numbers() {
        assert_eq!(fmt_with(|w| write_compact(w, 0)), "0");
        assert_eq!(fmt_with(|w| write_compact(w, 245)), "245");
        assert_eq!(fmt_with(|w| write_compact(w, 1_234)), "1.2k");
        assert_eq!(fmt_with(|w| write_compact(w, 18_420)), "18k");
        assert_eq!(fmt_with(|w| write_compact(w, 999_999)), "999k");
        assert_eq!(fmt_with(|w| write_compact(w, 1_250_000)), "1.2M");
    }

    #[test]
    fn sizes() {
        assert_eq!(fmt_with(|w| write_size(w, 512)), "512 B");
        assert_eq!(fmt_with(|w| write_size(w, 1024)), "1.0 KB");
        assert_eq!(fmt_with(|w| write_size(w, 127_488)), "124.5 KB");
        assert_eq!(fmt_with(|w| write_size(w, 3 * 1024 * 1024)), "3.0 MB");
    }

    #[test]
    fn stack_buf_truncates_on_char_boundary() {
        let mut b = StackBuf::<4>::new();
        // '│' is 3 bytes; the second one does not fit in the remaining byte.
        let _ = b.write_str("││");
        assert_eq!(b.as_str(), "│");
        assert_eq!(b.len(), 3);
    }

    #[test]
    fn digit_count() {
        assert_eq!(digits(0), 1);
        assert_eq!(digits(9), 1);
        assert_eq!(digits(10), 2);
        assert_eq!(digits(12_345), 5);
    }
}
