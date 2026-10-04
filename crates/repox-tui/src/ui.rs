//! Rendering.
//!
//! Frames are drawn straight into the ratatui [`Buffer`]. Only the rows that are
//! actually on screen are visited, and all text is either borrowed from the model
//! or formatted into stack buffers, so a frame performs no heap allocation.

use std::fmt::Write as _;

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType};

use crate::app::{App, Mode};
use crate::tree::Selection;
use crate::util::{StackBuf, digits, write_compact, write_grouped, write_size};

/// Below this width the preview pane is hidden and the tree takes the full width.
const MIN_SPLIT_WIDTH: u16 = 70;
const GAUGE_WIDTH: usize = 12;
const TAB_WIDTH: u16 = 4;

const CURSOR_BG: Color = Color::Indexed(237);
const ACCENT: Color = Color::Cyan;
const DIM: Color = Color::DarkGray;

const DIM_STYLE: Style = Style::new().fg(DIM);
const BOLD_STYLE: Style = Style::new().add_modifier(Modifier::BOLD);
const KEY_STYLE: Style = Style::new().fg(ACCENT).add_modifier(Modifier::BOLD);
const DIR_STYLE: Style = Style::new().fg(Color::Blue).add_modifier(Modifier::BOLD);
const FILE_STYLE: Style = Style::new().fg(Color::Gray);

#[inline]
fn has_room(area: Rect) -> bool {
    area.width > 0 && area.height > 0
}

/// A tiny left-to-right text cursor over one row of the buffer.
struct Writer<'a> {
    buf: &'a mut Buffer,
    x: u16,
    y: u16,
    right: u16,
}

impl<'a> Writer<'a> {
    fn new(buf: &'a mut Buffer, x: u16, y: u16, right: u16) -> Self {
        Self { buf, x, y, right }
    }

    /// Writes `text`, clipped at the right edge, and advances the cursor.
    fn put(&mut self, text: &str, style: Style) {
        if self.x < self.right {
            let room = usize::from(self.right - self.x);
            self.x = self.buf.set_stringn(self.x, self.y, text, room, style).0;
        }
    }

    fn remaining(&self) -> u16 {
        self.right.saturating_sub(self.x)
    }
}

/// Entry point called once per frame.
pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    if !has_room(area) {
        return;
    }
    let bar_height = u16::from(app.filter_bar_visible());
    let [main, bar, status] = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(bar_height),
        Constraint::Length(2),
    ])
    .areas(area);

    let (tree_area, preview_area) = if main.width >= MIN_SPLIT_WIDTH {
        let [left, right] =
            Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)]).areas(main);
        (left, Some(right))
    } else {
        (main, None)
    };

    draw_tree(frame, tree_area, app);
    if let Some(area) = preview_area {
        draw_preview(frame, area, app);
    }
    let buf = frame.buffer_mut();
    if has_room(bar) {
        draw_filter_bar(buf, bar, app);
    }
    draw_status(buf, status, app);
}

fn bordered(color: Color) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(color))
}

/// Writes a title on the top border, leaving the corners intact.
fn draw_title(buf: &mut Buffer, area: Rect, title: &str, style: Style) {
    if area.width > 6 && has_room(area) {
        let mut w = Writer::new(buf, area.x + 2, area.y, area.right().saturating_sub(2));
        w.put(title, style);
    }
}

// ---------------------------------------------------------------------------
// Tree pane
// ---------------------------------------------------------------------------

fn draw_tree(frame: &mut Frame, area: Rect, app: &mut App) {
    if !has_room(area) {
        return;
    }
    let block = bordered(ACCENT);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    app.set_viewport(usize::from(inner.height));

    let buf = frame.buffer_mut();
    let title = if app.tree.filter_active() {
        " Files (filtered) "
    } else {
        " Files "
    };
    draw_title(buf, area, title, BOLD_STYLE);
    if has_room(inner) {
        draw_rows(buf, inner, app);
    }
}

fn draw_rows(buf: &mut Buffer, inner: Rect, app: &App) {
    let visible = app.tree.visible();
    if visible.is_empty() {
        let msg = if app.tree.filter_active() {
            " no matching files"
        } else {
            " no files to show"
        };
        Writer::new(buf, inner.x, inner.y, inner.right()).put(msg, DIM_STYLE);
        return;
    }

    let filtering = app.tree.filter_active();
    // Reserve a right-hand column for the token count when there is room.
    let token_col: u16 = if inner.width >= 28 { 8 } else { 0 };
    let name_right = inner.right() - token_col;

    for row in 0..inner.height {
        let pos = app.top + usize::from(row);
        let Some(&node_idx) = visible.get(pos) else {
            break;
        };
        let node_idx = node_idx as usize;
        let Some(node) = app.tree.node(node_idx) else {
            continue;
        };
        let y = inner.y + row;

        if pos == app.cursor {
            buf.set_style(
                Rect::new(inner.x, y, inner.width, 1),
                Style::new().bg(CURSOR_BG),
            );
        }

        let state = app.tree.selection(node_idx);
        let mut w = Writer::new(buf, inner.x, y, name_right);

        // Indent guides.
        for _ in 0..node.depth {
            if w.remaining() < 2 {
                break;
            }
            w.put("│ ", DIM_STYLE);
        }

        // Fold arrow (files get blank padding so names line up).
        if node.is_dir() {
            let open = filtering || node.expanded;
            w.put(if open { "▾ " } else { "▸ " }, Style::new().fg(ACCENT));
        } else {
            w.put("  ", Style::new());
        }

        // Tri-state checkbox.
        let (checkbox, check_color) = match state {
            Selection::Selected => ("[x] ", Color::Green),
            Selection::Partial => ("[-] ", Color::Yellow),
            Selection::Unselected => ("[ ] ", DIM),
        };
        w.put(checkbox, Style::new().fg(check_color));

        // Name.
        let name_style = match (node.is_dir(), state) {
            (_, Selection::Unselected) => DIM_STYLE,
            (true, _) => DIR_STYLE,
            (false, _) => FILE_STYLE,
        };
        w.put(&node.name, name_style);
        if node.is_dir() {
            w.put("/", name_style);
        }

        // Cached aggregate token count, right-aligned.
        if token_col > 0 {
            let mut tokens = StackBuf::<16>::new();
            let _ = write_compact(&mut tokens, node.total.tokens);
            let len = u16::try_from(tokens.len()).unwrap_or(0);
            let x = inner.right().saturating_sub(len + 1);
            let color = match state {
                Selection::Selected => Color::Gray,
                Selection::Partial => Color::Yellow,
                Selection::Unselected => DIM,
            };
            Writer::new(buf, x, y, inner.right()).put(tokens.as_str(), Style::new().fg(color));
        }
    }
}

// ---------------------------------------------------------------------------
// Preview pane
// ---------------------------------------------------------------------------

fn draw_preview(frame: &mut Frame, area: Rect, app: &mut App) {
    if !has_room(area) {
        return;
    }
    let block = bordered(DIM);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    app.set_preview_rows(usize::from(inner.height));

    let buf = frame.buffer_mut();
    let Some(node_idx) = app.preview.node() else {
        if has_room(inner) {
            Writer::new(buf, inner.x, inner.y, inner.right()).put(" nothing to preview", DIM_STYLE);
        }
        return;
    };
    let Some(node) = app.tree.node(node_idx) else {
        return;
    };

    // Title: the tail of the path (the file name matters most) plus stats.
    let mut stats = StackBuf::<64>::new();
    let _ = write!(stats, " · ");
    let _ = write_compact(&mut stats, node.total.tokens);
    let _ = write!(stats, " tok · ");
    let _ = write_size(&mut stats, node.total.bytes);
    let _ = write!(stats, " ");
    let avail = usize::from(area.width).saturating_sub(4 + stats.len() + 2);
    let (path, clipped) = tail_fit(&node.path, avail);
    if area.width > 6 {
        let mut w = Writer::new(buf, area.x + 2, area.y, area.right().saturating_sub(2));
        w.put(" ", Style::new());
        if clipped {
            w.put("…", DIM_STYLE);
        }
        w.put(path, BOLD_STYLE);
        w.put(stats.as_str(), DIM_STYLE);
    }

    if !has_room(inner) {
        return;
    }
    match app.tree.file(node_idx) {
        Some(file) => draw_file_preview(buf, area, inner, app, &file.content),
        None => draw_dir_summary(buf, inner, app, node_idx),
    }
}

/// Returns the longest suffix of `path` that fits in `max` columns (counted in
/// chars) and whether anything was cut off.
fn tail_fit(path: &str, max: usize) -> (&str, bool) {
    let count = path.chars().count();
    if count <= max {
        return (path, false);
    }
    let skip = count - max.saturating_sub(1).min(count);
    let start = path
        .char_indices()
        .nth(skip)
        .map_or(path.len(), |(i, _)| i);
    (&path[start..], true)
}

fn draw_file_preview(buf: &mut Buffer, area: Rect, inner: Rect, app: &App, content: &str) {
    let total = app.preview.line_count();
    if total == 0 {
        Writer::new(buf, inner.x, inner.y, inner.right()).put(" (empty file)", DIM_STYLE);
        return;
    }
    let rows = usize::from(inner.height);
    let gutter = digits(total);
    let gutter_w = u16::try_from(gutter).unwrap_or(1);
    // Gutter = 1 pad + number + " │ " (3 columns).
    let text_x = inner.x + 1 + gutter_w + 3;

    for row in 0..rows {
        let line_no = app.preview_scroll + row;
        let Some(line) = app.preview.line(content, line_no) else {
            break;
        };
        let y = inner.y + u16::try_from(row).unwrap_or(0);

        let mut number = StackBuf::<24>::new();
        let _ = write!(number, "{:>gutter$}", line_no + 1);
        let mut w = Writer::new(buf, inner.x + 1, y, inner.right());
        w.put(number.as_str(), DIM_STYLE);
        w.put(" │ ", DIM_STYLE);
        draw_expanded(buf, text_x, y, inner.right(), line, Style::new());
    }

    // Footer note when the preview was cut and the end is on screen.
    let shown_end = app.preview_scroll + rows;
    if app.preview.truncated() && shown_end >= total {
        let row = total - app.preview_scroll;
        if row < rows {
            let y = inner.y + u16::try_from(row).unwrap_or(0);
            Writer::new(buf, inner.x + 1, y, inner.right())
                .put("… preview truncated (file is larger than 256 KB)", Style::new().fg(Color::Yellow));
        }
    }

    // Scroll position on the bottom border.
    if total > rows && area.height > 1 {
        let mut pos = StackBuf::<48>::new();
        let last = (app.preview_scroll + rows).min(total);
        let _ = write!(pos, " {}-{} / {} ", app.preview_scroll + 1, last, total);
        if usize::from(area.width) > pos.len() + 4 {
            let x = area.right() - u16::try_from(pos.len()).unwrap_or(0) - 2;
            Writer::new(buf, x, area.bottom() - 1, area.right() - 1).put(pos.as_str(), DIM_STYLE);
        }
    }
}

/// Draws `line` with tabs expanded to `TAB_WIDTH` columns. (`set_stringn` drops
/// control characters, which would otherwise swallow the indentation.)
fn draw_expanded(buf: &mut Buffer, x: u16, y: u16, right: u16, line: &str, style: Style) {
    let mut cx = x;
    for (i, segment) in line.split('\t').enumerate() {
        if i > 0 {
            let col = cx - x;
            cx = cx.saturating_add(TAB_WIDTH - col % TAB_WIDTH);
        }
        if cx >= right {
            return;
        }
        cx = buf
            .set_stringn(cx, y, segment, usize::from(right - cx), style)
            .0;
    }
}

fn draw_dir_summary(buf: &mut Buffer, inner: Rect, app: &App, node_idx: usize) {
    let Some(node) = app.tree.node(node_idx) else {
        return;
    };
    let mut line = |row: u16, label: &str, value: &StackBuf<64>| {
        if row < inner.height {
            let mut w = Writer::new(buf, inner.x + 1, inner.y + row, inner.right());
            w.put(label, DIM_STYLE);
            w.put(value.as_str(), Style::new());
        }
    };

    let mut v = StackBuf::<64>::new();
    let _ = write!(v, "{} / {} selected", node.selected.files, node.total.files);
    line(1, "Files   ", &v);

    let mut v = StackBuf::<64>::new();
    let _ = write_grouped(&mut v, node.selected.tokens);
    let _ = write!(v, " / ");
    let _ = write_grouped(&mut v, node.total.tokens);
    line(2, "Tokens  ", &v);

    let mut v = StackBuf::<64>::new();
    let _ = write_size(&mut v, node.selected.bytes);
    let _ = write!(v, " / ");
    let _ = write_size(&mut v, node.total.bytes);
    line(3, "Size    ", &v);
}

// ---------------------------------------------------------------------------
// Filter bar & status bar
// ---------------------------------------------------------------------------

fn draw_filter_bar(buf: &mut Buffer, area: Rect, app: &App) {
    let mut count = StackBuf::<32>::new();
    let matches = app.tree.filter_match_count();
    let no_matches = app.tree.filter_active() && matches == 0;
    if app.tree.filter_active() {
        let _ = write!(count, "{matches} match{} ", if matches == 1 { "" } else { "es" });
    }
    let count_w = u16::try_from(count.len()).unwrap_or(0);
    let count_x = area.right().saturating_sub(count_w);

    let mut w = Writer::new(buf, area.x + 1, area.y, count_x.max(area.x + 1));
    w.put("/ ", Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD));
    w.put(&app.query, Style::new());
    if app.mode == Mode::Filter {
        w.put("█", Style::new().fg(ACCENT));
    }
    let style = if no_matches {
        Style::new().fg(Color::Red)
    } else {
        DIM_STYLE
    };
    Writer::new(buf, count_x, area.y, area.right()).put(count.as_str(), style);
}

fn gauge_color(ratio: f64) -> Color {
    if ratio >= 1.0 {
        Color::Red
    } else if ratio >= 0.9 {
        Color::LightRed
    } else if ratio >= 0.7 {
        Color::Yellow
    } else {
        Color::Green
    }
}

fn draw_status(buf: &mut Buffer, area: Rect, app: &App) {
    if !has_room(area) {
        return;
    }
    draw_metrics(buf, Rect::new(area.x, area.y, area.width, 1), app);
    if area.height > 1 {
        draw_hints(buf, Rect::new(area.x, area.y + 1, area.width, 1), app);
    }
}

fn draw_metrics(buf: &mut Buffer, row: Rect, app: &App) {
    let selected = app.tree.selected();
    let total = app.tree.total();
    let limit = app.options.context_limit as u64;
    let ratio = if limit == 0 {
        0.0
    } else {
        selected.tokens as f64 / limit as f64
    };
    let color = gauge_color(ratio);
    let separator = Style::new().fg(DIM);

    let mut w = Writer::new(buf, row.x + 1, row.y, row.right());

    // Files selected / found.
    let mut s = StackBuf::<48>::new();
    let _ = write!(s, "{}/{} files", selected.files, total.files);
    w.put(s.as_str(), BOLD_STYLE);
    w.put("  │  ", separator);

    // Percentage bar.
    let filled = ((ratio.clamp(0.0, 1.0) * GAUGE_WIDTH as f64).round() as usize).min(GAUGE_WIDTH);
    for i in 0..GAUGE_WIDTH {
        if i < filled {
            w.put("█", Style::new().fg(color));
        } else {
            w.put("░", DIM_STYLE);
        }
    }

    // `18,420 / 200,000 tokens (9.2%) [Claude 3.5 Sonnet]`
    let mut s = StackBuf::<96>::new();
    let _ = write!(s, " ");
    let _ = write_grouped(&mut s, selected.tokens);
    let _ = write!(s, " / ");
    let _ = write_grouped(&mut s, limit);
    let _ = write!(s, " tokens ({:.1}%)", ratio * 100.0);
    w.put(s.as_str(), Style::new().fg(color));
    w.put(" [", DIM_STYLE);
    w.put(&app.options.model_name, Style::new().fg(ACCENT));
    w.put("]", DIM_STYLE);
    if ratio > 1.0 {
        w.put(" OVER LIMIT", Style::new().fg(Color::Red).add_modifier(Modifier::BOLD));
    }

    // Payload size.
    w.put("  │  ", separator);
    let mut s = StackBuf::<32>::new();
    let _ = write_size(&mut s, selected.bytes);
    w.put(s.as_str(), BOLD_STYLE);
}

const NORMAL_HINTS: &[(&str, &str)] = &[
    ("↑↓/jk", "move"),
    ("←→/hl", "fold"),
    ("Space", "select"),
    ("a", "all"),
    ("i", "invert"),
    ("/", "filter"),
    ("PgUp/Dn", "preview"),
    ("c", "copy"),
    ("Enter", "output"),
    ("q", "quit"),
];

const FILTER_HINTS: &[(&str, &str)] = &[
    ("type", "fuzzy search"),
    ("↑↓", "move"),
    ("Enter", "accept"),
    ("Esc", "clear"),
];

fn draw_hints(buf: &mut Buffer, row: Rect, app: &App) {
    let mut w = Writer::new(buf, row.x + 1, row.y, row.right());
    if let Some(message) = app.message {
        w.put(message, Style::new().fg(Color::Red).add_modifier(Modifier::BOLD));
        return;
    }
    let hints = if app.mode == Mode::Filter {
        FILTER_HINTS
    } else {
        NORMAL_HINTS
    };
    for (key, description) in hints {
        w.put(key, KEY_STYLE);
        w.put(" ", Style::new());
        w.put(description, DIM_STYLE);
        w.put("  ", Style::new());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TuiOptions;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use repox_core::RepoFile;
    use std::path::PathBuf;

    fn rf(path: &str, tokens: usize, content: &str) -> RepoFile {
        let mut f = RepoFile::new(
            PathBuf::from(path),
            PathBuf::from("/abs").join(path),
            content.len() as u64,
            content.to_owned(),
        );
        f.token_count = Some(tokens);
        f
    }

    fn app() -> App {
        App::new(
            vec![
                rf("Cargo.toml", 100, "[package]\nname = \"demo\"\n"),
                rf("src/lib.rs", 18_000, "fn a() {}\n\tfn tabbed() {}\n"),
                rf("src/main.rs", 320, "fn main() {\n    println!(\"hi\");\n}\n"),
            ],
            TuiOptions::new("Claude 3.5 Sonnet", 200_000),
        )
    }

    fn press(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    fn draw(app: &mut App, width: u16, height: u16) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test backend");
        terminal.draw(|f| render(f, app)).expect("draw");
        let buffer = terminal.backend().buffer().clone();
        (0..height)
            .map(|y| (0..width).map(|x| buffer[(x, y)].symbol().to_owned()).collect::<String>())
            .collect()
    }

    fn contains(screen: &[String], needle: &str) -> bool {
        screen.iter().any(|row| row.contains(needle))
    }

    #[test]
    fn renders_tree_preview_and_status() {
        let mut app = app();
        let screen = draw(&mut app, 110, 24);

        assert!(contains(&screen, " Files "));
        assert!(contains(&screen, "▾ [x] src/"), "folder row with checkbox and arrow");
        assert!(contains(&screen, "[x] main.rs"));
        assert!(contains(&screen, "3/3 files"));
        assert!(contains(&screen, "18,420 / 200,000 tokens (9.2%)"));
        assert!(contains(&screen, "[Claude 3.5 Sonnet]"));
        assert!(contains(&screen, "Space select"));
        // Folder token column shows the cached aggregate: 18000 + 320 = 18320.
        assert!(contains(&screen, "18k"));
    }

    #[test]
    fn tri_state_checkboxes_and_live_counters() {
        let mut app = app();
        // Cursor starts on `src/`; go to lib.rs and deselect it.
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Char(' '));
        let screen = draw(&mut app, 110, 24);

        assert!(contains(&screen, "[-] src/"), "parent becomes partial");
        assert!(contains(&screen, "[ ] lib.rs"));
        assert!(contains(&screen, "2/3 files"));
        assert!(contains(&screen, "420 / 200,000 tokens (0.2%)"));
    }

    #[test]
    fn preview_shows_line_numbers_and_expands_tabs() {
        let mut app = app();
        press(&mut app, KeyCode::Down); // src/lib.rs
        let screen = draw(&mut app, 110, 24);

        assert!(contains(&screen, "1 │ fn a() {}"));
        assert!(contains(&screen, "2 │     fn tabbed() {}"), "tab became 4 spaces");
        assert!(contains(&screen, "src/lib.rs"), "path in the pane title");
    }

    #[test]
    fn directory_hover_shows_a_summary() {
        let mut app = app();
        let screen = draw(&mut app, 110, 24); // cursor on `src`
        assert!(contains(&screen, "Files   2 / 2 selected"));
        assert!(contains(&screen, "Tokens  18,320 / 18,320"));
    }

    #[test]
    fn filter_bar_shows_query_and_match_count() {
        let mut app = app();
        press(&mut app, KeyCode::Char('/'));
        for c in "main".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        let screen = draw(&mut app, 110, 24);
        assert!(contains(&screen, "/ main█"));
        assert!(contains(&screen, "1 match"));
        assert!(contains(&screen, " Files (filtered) "));
        assert!(contains(&screen, "fuzzy search"), "filter-mode hints");
        assert!(!contains(&screen, "Cargo.toml  "), "non-matches are hidden");
    }

    #[test]
    fn no_match_message_is_shown() {
        let mut app = app();
        press(&mut app, KeyCode::Char('/'));
        for c in "qqqq".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        let screen = draw(&mut app, 110, 24);
        assert!(contains(&screen, "no matching files"));
        assert!(contains(&screen, "0 matches"));
    }

    #[test]
    fn over_limit_is_flagged() {
        let mut app = App::new(
            vec![rf("big.rs", 5_000, "x\n")],
            TuiOptions::new("Tiny", 1_000),
        );
        let screen = draw(&mut app, 110, 12);
        assert!(contains(&screen, "OVER LIMIT"));
        assert!(contains(&screen, "(500.0%)"));
    }

    #[test]
    fn message_replaces_the_hint_row() {
        let mut app = app();
        press(&mut app, KeyCode::Char('a')); // deselect all
        press(&mut app, KeyCode::Char('c'));
        let screen = draw(&mut app, 110, 24);
        assert!(contains(&screen, "Nothing selected"));
    }

    #[test]
    fn narrow_terminals_drop_the_preview_pane() {
        let mut app = app();
        press(&mut app, KeyCode::Down);
        let screen = draw(&mut app, 50, 16);
        assert!(contains(&screen, "[x] main.rs"));
        assert!(!contains(&screen, "1 │ fn a()"), "no preview below the split threshold");
    }

    #[test]
    fn survives_degenerate_terminal_sizes() {
        for (w, h) in [(1, 1), (2, 2), (5, 3), (12, 4), (20, 5), (71, 6), (200, 3)] {
            let mut app = app();
            let _ = draw(&mut app, w, h);
            press(&mut app, KeyCode::Char('/'));
            press(&mut app, KeyCode::Char('m'));
            let _ = draw(&mut app, w, h);
        }
    }

    #[test]
    fn tail_fit_keeps_the_file_name() {
        assert_eq!(tail_fit("a/b/c.rs", 20), ("a/b/c.rs", false));
        let (tail, clipped) = tail_fit("very/long/path/to/main.rs", 10);
        assert!(clipped);
        assert!(tail.ends_with("main.rs"));
        assert!(tail.chars().count() <= 10);
        assert!(tail_fit("日本語/ファイル.rs", 6).1);
        assert_eq!(tail_fit("abc", 0), ("", true));
    }

    #[test]
    fn large_trees_render_only_the_viewport() {
        let files: Vec<_> = (0..20_000)
            .map(|i| rf(&format!("d{}/f{i}.rs", i % 40), 10, "x\n"))
            .collect();
        let mut app = App::new(files, TuiOptions::new("Test", 1_000_000));
        press(&mut app, KeyCode::Char('G'));
        let screen = draw(&mut app, 100, 20);
        assert!(contains(&screen, "20000/20000 files"));
    }
}
