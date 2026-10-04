//! Application state machine: cursor, scrolling, modes and key handling.
//!
//! Nothing in here touches the terminal, which keeps every behaviour unit-testable.

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use repox_core::RepoFile;

use crate::TuiOptions;
use crate::preview::Preview;
use crate::tree::FlatTree;

/// Input mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    /// The `/` prompt is capturing keystrokes.
    Filter,
}

/// Why the event loop stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Abort,
    Copy,
    Output,
}

const MSG_NOTHING_SELECTED: &str = "Nothing selected - press Space to pick files";

pub struct App {
    pub(crate) tree: FlatTree,
    pub(crate) options: TuiOptions,
    pub(crate) mode: Mode,
    /// Raw text of the filter prompt.
    pub(crate) query: String,
    /// Index into `tree.visible()`.
    pub(crate) cursor: usize,
    /// First visible row index shown in the tree pane.
    pub(crate) top: usize,
    pub(crate) viewport_rows: usize,
    pub(crate) preview: Preview,
    pub(crate) preview_scroll: usize,
    pub(crate) preview_rows: usize,
    /// One-shot status line message, cleared on the next key press.
    pub(crate) message: Option<&'static str>,
    matcher: Matcher,
}

impl App {
    pub fn new(files: Vec<RepoFile>, options: TuiOptions) -> Self {
        let tree = FlatTree::new(files, options.expand_depth);
        let mut app = Self {
            tree,
            options,
            mode: Mode::Normal,
            query: String::new(),
            cursor: 0,
            top: 0,
            viewport_rows: 0,
            preview: Preview::default(),
            preview_scroll: 0,
            preview_rows: 0,
            message: None,
            matcher: Matcher::new(Config::DEFAULT.match_paths()),
        };
        app.sync_preview();
        app
    }

    /// Consumes the app, returning the selected files in scanner order.
    pub fn into_selected(self) -> Vec<RepoFile> {
        self.tree.into_selected()
    }

    /// Node index under the cursor.
    pub(crate) fn hovered(&self) -> Option<usize> {
        self.tree.visible().get(self.cursor).map(|&i| i as usize)
    }

    pub(crate) fn filter_bar_visible(&self) -> bool {
        self.mode == Mode::Filter || !self.query.is_empty()
    }

    // ----- viewport --------------------------------------------------------

    /// Called by the renderer with the real pane height; keeps the cursor in view.
    pub(crate) fn set_viewport(&mut self, rows: usize) {
        self.viewport_rows = rows;
        if rows == 0 {
            return;
        }
        if self.cursor < self.top {
            self.top = self.cursor;
        } else if self.cursor >= self.top + rows {
            self.top = self.cursor + 1 - rows;
        }
        let max_top = self.tree.visible().len().saturating_sub(rows);
        self.top = self.top.min(max_top);
    }

    pub(crate) fn set_preview_rows(&mut self, rows: usize) {
        self.preview_rows = rows;
        let max = self.preview.line_count().saturating_sub(rows.max(1));
        self.preview_scroll = self.preview_scroll.min(max);
    }

    // ----- events ----------------------------------------------------------

    /// Handles one terminal event. Returns `Some` when the UI should exit.
    pub fn handle_event(&mut self, event: Event) -> Option<Exit> {
        match event {
            // Windows also reports key releases; ignore them.
            Event::Key(key) if key.kind != KeyEventKind::Release => self.handle_key(key),
            // Resizes need no state change: the next frame re-lays everything out.
            _ => None,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Exit> {
        self.message = None;
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && key.code == KeyCode::Char('c') {
            return Some(Exit::Abort);
        }
        match self.mode {
            Mode::Normal => self.handle_normal(key.code, ctrl),
            Mode::Filter => self.handle_filter(key.code, ctrl),
        }
    }

    fn handle_normal(&mut self, code: KeyCode, ctrl: bool) -> Option<Exit> {
        if ctrl {
            match code {
                KeyCode::Char('d') => self.scroll_preview(1),
                KeyCode::Char('u') => self.scroll_preview(-1),
                _ => {}
            }
            return None;
        }
        match code {
            KeyCode::Char('q') => return Some(Exit::Abort),
            KeyCode::Esc => {
                if self.query.is_empty() {
                    return Some(Exit::Abort);
                }
                self.clear_filter();
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_cursor(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_cursor(1),
            KeyCode::Home | KeyCode::Char('g') => self.set_cursor(0),
            KeyCode::End | KeyCode::Char('G') => self.set_cursor(usize::MAX),
            KeyCode::Right | KeyCode::Char('l') => self.expand_or_descend(),
            KeyCode::Left | KeyCode::Char('h') => self.collapse_or_ascend(),
            KeyCode::Enter => return self.activate(),
            KeyCode::Char(' ') => {
                if let Some(node) = self.hovered() {
                    self.tree.toggle_selection(node);
                }
            }
            KeyCode::Char('a') => self.tree.toggle_all(),
            KeyCode::Char('i') => self.tree.invert_selection(),
            KeyCode::Char('/') => self.mode = Mode::Filter,
            KeyCode::Char('c') => return self.confirm(Exit::Copy),
            KeyCode::PageDown => self.scroll_preview(1),
            KeyCode::PageUp => self.scroll_preview(-1),
            _ => {}
        }
        None
    }

    fn handle_filter(&mut self, code: KeyCode, ctrl: bool) -> Option<Exit> {
        if ctrl {
            match code {
                KeyCode::Char('u') => {
                    self.query.clear();
                    self.refresh_filter();
                }
                KeyCode::Char('n') => self.move_cursor(1),
                KeyCode::Char('p') => self.move_cursor(-1),
                _ => {}
            }
            return None;
        }
        match code {
            KeyCode::Esc => {
                self.clear_filter();
                self.mode = Mode::Normal;
            }
            // Keep the filter applied and go back to navigating the results.
            KeyCode::Enter => self.mode = Mode::Normal,
            KeyCode::Backspace => {
                if self.query.pop().is_some() {
                    self.refresh_filter();
                } else {
                    self.mode = Mode::Normal;
                }
            }
            KeyCode::Up => self.move_cursor(-1),
            KeyCode::Down => self.move_cursor(1),
            KeyCode::Char(c) => {
                self.query.push(c);
                self.refresh_filter();
            }
            _ => {}
        }
        None
    }

    // ----- actions ---------------------------------------------------------

    /// `Enter`: expand/collapse folders, otherwise confirm and output.
    fn activate(&mut self) -> Option<Exit> {
        let node = self.hovered()?;
        let (is_dir, expanded) = self
            .tree
            .node(node)
            .map_or((false, false), |n| (n.is_dir(), n.expanded));
        if !is_dir {
            return self.confirm(Exit::Output);
        }
        if self.tree.filter_active() {
            self.move_cursor(1); // folders are always open while filtering
        } else {
            self.tree.set_expanded(node, !expanded);
            self.reanchor(node);
        }
        None
    }

    /// Refuses to leave with an empty selection.
    fn confirm(&mut self, exit: Exit) -> Option<Exit> {
        if self.tree.selected().files == 0 {
            self.message = Some(MSG_NOTHING_SELECTED);
            None
        } else {
            Some(exit)
        }
    }

    /// `l` / `→`: expand a closed folder, or step into an open one.
    fn expand_or_descend(&mut self) {
        let Some(node) = self.hovered() else { return };
        let Some((is_dir, expanded)) = self.tree.node(node).map(|n| (n.is_dir(), n.expanded))
        else {
            return;
        };
        if !is_dir {
            return;
        }
        if self.tree.filter_active() || expanded {
            self.move_cursor(1); // first child is the next visible row
        } else {
            self.tree.set_expanded(node, true);
            self.reanchor(node);
        }
    }

    /// `h` / `←`: collapse an open folder, otherwise jump to the parent.
    fn collapse_or_ascend(&mut self) {
        let Some(node) = self.hovered() else { return };
        let Some((is_dir, expanded)) = self.tree.node(node).map(|n| (n.is_dir(), n.expanded))
        else {
            return;
        };
        if is_dir && expanded && !self.tree.filter_active() {
            self.tree.set_expanded(node, false);
            self.reanchor(node);
        } else if let Some(pos) = self
            .tree
            .parent(node)
            .and_then(|p| self.tree.visible_position(p))
        {
            self.set_cursor(pos);
        }
    }

    fn scroll_preview(&mut self, direction: isize) {
        let page = (self.preview_rows / 2).max(1) as isize;
        let max = self.preview.line_count().saturating_sub(self.preview_rows.max(1));
        self.preview_scroll = self
            .preview_scroll
            .saturating_add_signed(direction * page)
            .min(max);
    }

    // ----- cursor ----------------------------------------------------------

    fn move_cursor(&mut self, delta: isize) {
        let last = self.tree.visible().len().saturating_sub(1);
        self.cursor = self.cursor.saturating_add_signed(delta).min(last);
        self.sync_preview();
    }

    fn set_cursor(&mut self, pos: usize) {
        let last = self.tree.visible().len().saturating_sub(1);
        self.cursor = pos.min(last);
        self.sync_preview();
    }

    /// Re-points the cursor at `node` (or its closest visible ancestor) after the
    /// row list was rebuilt.
    fn reanchor(&mut self, node: usize) {
        self.cursor = self.tree.nearest_visible(node);
        self.sync_preview();
    }

    /// Reloads the preview cache when the hovered node changed.
    fn sync_preview(&mut self) {
        let hovered = self.hovered();
        if hovered == self.preview.node() {
            return;
        }
        self.preview_scroll = 0;
        match hovered {
            Some(node) => match self.tree.file(node) {
                Some(file) => self.preview.load(node, &file.content),
                None => self.preview.clear_for(Some(node)),
            },
            None => self.preview.clear_for(None),
        }
    }

    // ----- filter ----------------------------------------------------------

    fn clear_filter(&mut self) {
        self.query.clear();
        self.refresh_filter();
    }

    /// Re-runs the fuzzy match for the current query and re-anchors the cursor:
    /// on the best match while filtering, on the previous node otherwise.
    fn refresh_filter(&mut self) {
        let anchor = self.hovered();
        if self.query.trim().is_empty() {
            self.tree.clear_filter();
        } else {
            let pattern = Pattern::parse(&self.query, CaseMatching::Smart, Normalization::Smart);
            let matcher = &mut self.matcher;
            let mut buf = Vec::new();
            self.tree
                .set_filter(|path| pattern.score(Utf32Str::new(path, &mut buf), matcher));
        }
        let target = if self.tree.filter_active() {
            self.tree.filter_best().or(anchor)
        } else {
            anchor
        };
        self.cursor = target.map_or(0, |n| self.tree.nearest_visible(n));
        self.top = 0;
        self.sync_preview();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn rf(path: &str, tokens: usize) -> RepoFile {
        let mut f = RepoFile::new(
            PathBuf::from(path),
            PathBuf::from("/abs").join(path),
            tokens as u64 * 4,
            format!("// {path}\nline two\nline three\n"),
        );
        f.token_count = Some(tokens);
        f
    }

    fn app() -> App {
        App::new(
            vec![
                rf("Cargo.toml", 10),
                rf("crates/a/x.rs", 20),
                rf("crates/a/y.rs", 30),
                rf("src/lib.rs", 50),
                rf("src/main.rs", 60),
            ],
            TuiOptions::new("Test", 100_000),
        )
    }

    fn press(app: &mut App, code: KeyCode) -> Option<Exit> {
        app.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn type_str(app: &mut App, s: &str) {
        for c in s.chars() {
            press(app, KeyCode::Char(c));
        }
    }

    fn hovered_path(app: &App) -> String {
        app.hovered()
            .and_then(|n| app.tree.node(n))
            .map(|n| n.path.to_string())
            .unwrap_or_default()
    }

    fn visible_paths(app: &App) -> Vec<String> {
        app.tree
            .visible()
            .iter()
            .filter_map(|&i| app.tree.node(i as usize))
            .map(|n| n.path.to_string())
            .collect()
    }

    #[test]
    fn navigation_moves_and_clamps() {
        let mut app = app();
        assert_eq!(hovered_path(&app), "crates");
        press(&mut app, KeyCode::Char('k'));
        assert_eq!(app.cursor, 0, "clamped at the top");
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(hovered_path(&app), "crates/a");
        press(&mut app, KeyCode::End);
        assert_eq!(hovered_path(&app), "Cargo.toml");
        press(&mut app, KeyCode::Down);
        assert_eq!(hovered_path(&app), "Cargo.toml", "clamped at the bottom");
        press(&mut app, KeyCode::Char('g'));
        assert_eq!(app.cursor, 0);
        press(&mut app, KeyCode::Char('G'));
        assert_eq!(hovered_path(&app), "Cargo.toml");
    }

    #[test]
    fn space_toggles_files_and_folders_with_live_counters() {
        let mut app = app();
        assert_eq!(app.tree.selected().tokens, 170);

        press(&mut app, KeyCode::Char(' ')); // `crates` folder
        assert_eq!(app.tree.selected().files, 3);
        assert_eq!(app.tree.selected().tokens, 120);

        press(&mut app, KeyCode::Char(' '));
        assert_eq!(app.tree.selected(), app.tree.total());
    }

    #[test]
    fn right_expands_then_descends_and_left_collapses_then_ascends() {
        let mut app = app();
        // Default expand depth is 2 -> `crates` and `crates/a` are open.
        press(&mut app, KeyCode::Char('h'));
        assert_eq!(hovered_path(&app), "crates");
        assert!(!visible_paths(&app).contains(&"crates/a".to_string()), "collapsed");

        press(&mut app, KeyCode::Char('l'));
        assert!(visible_paths(&app).contains(&"crates/a".to_string()), "expanded");
        assert_eq!(hovered_path(&app), "crates", "expanding keeps the cursor");

        press(&mut app, KeyCode::Char('l')); // already open -> first child
        assert_eq!(hovered_path(&app), "crates/a");
        press(&mut app, KeyCode::Right); // open folder -> first child
        assert_eq!(hovered_path(&app), "crates/a/x.rs");

        press(&mut app, KeyCode::Left); // file -> parent
        assert_eq!(hovered_path(&app), "crates/a");
        press(&mut app, KeyCode::Left); // open folder -> collapse
        assert!(!visible_paths(&app).contains(&"crates/a/x.rs".to_string()));
        press(&mut app, KeyCode::Left); // closed folder -> parent
        assert_eq!(hovered_path(&app), "crates");
    }

    #[test]
    fn enter_toggles_folders_and_outputs_on_files() {
        let mut app = app();
        assert_eq!(press(&mut app, KeyCode::Enter), None, "folder: toggles expansion");
        assert!(!visible_paths(&app).contains(&"crates/a".to_string()));
        press(&mut app, KeyCode::Enter);
        assert!(visible_paths(&app).contains(&"crates/a".to_string()));

        press(&mut app, KeyCode::End); // Cargo.toml (a file)
        assert_eq!(press(&mut app, KeyCode::Enter), Some(Exit::Output));
    }

    #[test]
    fn confirming_with_an_empty_selection_is_refused() {
        let mut app = app();
        press(&mut app, KeyCode::Char('a')); // deselect everything
        assert_eq!(app.tree.selected().files, 0);

        press(&mut app, KeyCode::End);
        assert_eq!(press(&mut app, KeyCode::Enter), None);
        assert_eq!(app.message, Some(MSG_NOTHING_SELECTED));
        assert_eq!(press(&mut app, KeyCode::Char('c')), None);

        // The message disappears on the next key press.
        press(&mut app, KeyCode::Char('k'));
        assert_eq!(app.message, None);

        press(&mut app, KeyCode::End);
        press(&mut app, KeyCode::Char(' '));
        assert_eq!(press(&mut app, KeyCode::Char('c')), Some(Exit::Copy));
    }

    #[test]
    fn quit_keys() {
        let mut app = app();
        assert_eq!(press(&mut app, KeyCode::Char('q')), Some(Exit::Abort));
        assert_eq!(press(&mut app, KeyCode::Esc), Some(Exit::Abort));
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(app.handle_key(ctrl_c), Some(Exit::Abort));

        press(&mut app, KeyCode::Char('/'));
        assert_eq!(app.handle_key(ctrl_c), Some(Exit::Abort), "also while typing a filter");
    }

    #[test]
    fn select_all_and_invert() {
        let mut app = app();
        press(&mut app, KeyCode::Char('a'));
        assert_eq!(app.tree.selected().files, 0);
        press(&mut app, KeyCode::Char('a'));
        assert_eq!(app.tree.selected(), app.tree.total());

        press(&mut app, KeyCode::End);
        press(&mut app, KeyCode::Char(' ')); // drop Cargo.toml
        press(&mut app, KeyCode::Char('i'));
        assert_eq!(app.tree.selected().files, 1);
        assert_eq!(app.tree.selected().tokens, 10);
    }

    #[test]
    fn filter_jumps_to_best_match_and_keeps_tree_context() {
        let mut app = app();
        press(&mut app, KeyCode::Char('/'));
        assert_eq!(app.mode, Mode::Filter);
        type_str(&mut app, "main");

        assert_eq!(hovered_path(&app), "src/main.rs");
        let rows = visible_paths(&app);
        assert!(rows.contains(&"src".to_string()), "ancestors stay for context");
        assert!(!rows.contains(&"Cargo.toml".to_string()));
        assert!(app.filter_bar_visible());

        press(&mut app, KeyCode::Enter); // accept: keep the filter, leave typing mode
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.tree.filter_active());
        assert!(app.filter_bar_visible());
    }

    #[test]
    fn typing_a_space_in_the_filter_does_not_toggle_selection() {
        let mut app = app();
        press(&mut app, KeyCode::Char('/'));
        type_str(&mut app, "src ma");
        assert_eq!(app.query, "src ma");
        assert_eq!(app.tree.selected(), app.tree.total());
        assert_eq!(hovered_path(&app), "src/main.rs");
    }

    #[test]
    fn escape_clears_the_filter_before_it_quits() {
        let mut app = app();
        press(&mut app, KeyCode::Char('/'));
        type_str(&mut app, "lib");
        press(&mut app, KeyCode::Enter);
        assert_eq!(hovered_path(&app), "src/lib.rs");

        assert_eq!(press(&mut app, KeyCode::Esc), None, "first Esc clears");
        assert!(!app.tree.filter_active());
        assert!(app.query.is_empty());
        assert_eq!(hovered_path(&app), "src/lib.rs", "cursor stays on the same file");
        assert_eq!(press(&mut app, KeyCode::Esc), Some(Exit::Abort), "second Esc quits");
    }

    #[test]
    fn backspace_edits_and_empties_the_filter() {
        let mut app = app();
        press(&mut app, KeyCode::Char('/'));
        type_str(&mut app, "zz");
        assert_eq!(app.tree.filter_match_count(), 0);
        assert!(app.tree.visible().is_empty());

        press(&mut app, KeyCode::Backspace);
        press(&mut app, KeyCode::Backspace);
        assert!(!app.tree.filter_active());
        assert_eq!(app.mode, Mode::Filter);
        press(&mut app, KeyCode::Backspace); // empty prompt -> leave filter mode
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn selection_actions_apply_to_matches_only_while_filtering() {
        let mut app = app();
        press(&mut app, KeyCode::Char('/'));
        type_str(&mut app, "src/");
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Char('a')); // deselect the matching files
        assert_eq!(app.tree.selected().files, 3);
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.tree.selected().files, 3);
    }

    #[test]
    fn empty_filter_result_is_safe_to_navigate() {
        let mut app = app();
        press(&mut app, KeyCode::Char('/'));
        type_str(&mut app, "qqqq");
        for code in [KeyCode::Down, KeyCode::Up, KeyCode::End, KeyCode::Char(' ')] {
            press(&mut app, code);
        }
        assert_eq!(app.hovered(), None);
        assert_eq!(app.preview.node(), None);
    }

    #[test]
    fn preview_follows_the_cursor_and_resets_scroll() {
        let mut app = app();
        press(&mut app, KeyCode::End); // Cargo.toml
        assert_eq!(app.preview.node(), app.hovered());
        assert_eq!(app.preview.line_count(), 3);

        app.set_preview_rows(2);
        app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
        assert_eq!(app.preview_scroll, 1, "clamped to line_count - rows");
        press(&mut app, KeyCode::PageUp);
        assert_eq!(app.preview_scroll, 0);

        press(&mut app, KeyCode::PageDown);
        press(&mut app, KeyCode::Char('k'));
        assert_eq!(app.preview_scroll, 0, "new file -> scroll resets");
    }

    #[test]
    fn viewport_keeps_the_cursor_visible() {
        let files: Vec<_> = (0..50).map(|i| rf(&format!("f{i:02}.rs"), 1)).collect();
        let mut app = App::new(files, TuiOptions::new("Test", 1000));
        app.set_viewport(10);
        press(&mut app, KeyCode::Char('G'));
        app.set_viewport(10);
        assert_eq!(app.top, 40);
        assert!(app.cursor >= app.top && app.cursor < app.top + 10);

        press(&mut app, KeyCode::Char('g'));
        app.set_viewport(10);
        assert_eq!(app.top, 0);
    }

    #[test]
    fn release_events_and_resizes_are_ignored() {
        let mut app = app();
        let mut release = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        release.kind = KeyEventKind::Release;
        assert_eq!(app.handle_event(Event::Key(release)), None);
        assert_eq!(app.handle_event(Event::Resize(80, 24)), None);
    }

    #[test]
    fn into_selected_returns_only_checked_files() {
        let mut app = app();
        press(&mut app, KeyCode::Char(' ')); // drop `crates`
        let files = app.into_selected();
        let names: Vec<_> = files
            .iter()
            .map(|f| f.relative_path.to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["Cargo.toml", "src/lib.rs", "src/main.rs"]);
    }

    #[test]
    fn empty_app_does_not_panic() {
        let mut app = App::new(Vec::new(), TuiOptions::new("Test", 1000));
        for code in [
            KeyCode::Down,
            KeyCode::Char('l'),
            KeyCode::Char('h'),
            KeyCode::Enter,
            KeyCode::Char(' '),
            KeyCode::Char('a'),
            KeyCode::Char('c'),
        ] {
            let _ = press(&mut app, code);
        }
        assert!(app.into_selected().is_empty());
    }
}
