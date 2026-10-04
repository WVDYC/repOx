//! Flat, arena-based tree model.
//!
//! The hierarchy is stored as a `Vec<Node>` in **pre-order**, which gives two
//! useful properties:
//!
//! * the subtree of node `i` is the contiguous range `i..nodes[i].subtree_end`,
//!   so cascading selection is a linear pass over a flat slice (no pointer
//!   chasing, no recursion);
//! * walking the *visible* rows only needs to skip `subtree_end` for collapsed
//!   folders, so rebuilding the row list is `O(visible)`.
//!
//! Selection aggregates (files / tokens / bytes) are cached on every node and
//! maintained incrementally, so the status bar reads them in `O(1)` and never
//! re-tokenizes anything.

use std::collections::BTreeMap;
use std::path::Component;

use repox_core::RepoFile;

/// Sentinel parent index for top-level nodes.
pub const NO_PARENT: u32 = u32::MAX;

/// Additive aggregate of the numbers the status bar cares about.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Totals {
    pub files: u32,
    pub tokens: u64,
    pub bytes: u64,
}

impl Totals {
    pub const ZERO: Self = Self {
        files: 0,
        tokens: 0,
        bytes: 0,
    };

    #[inline]
    fn add(self, other: Self) -> Self {
        Self {
            files: self.files.saturating_add(other.files),
            tokens: self.tokens.saturating_add(other.tokens),
            bytes: self.bytes.saturating_add(other.bytes),
        }
    }

    #[inline]
    fn sub(self, other: Self) -> Self {
        Self {
            files: self.files.saturating_sub(other.files),
            tokens: self.tokens.saturating_sub(other.tokens),
            bytes: self.bytes.saturating_sub(other.bytes),
        }
    }
}

/// Tri-state checkbox value, derived from the cached aggregates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    Unselected,
    Partial,
    Selected,
}

/// One row-capable entry of the flattened tree.
#[derive(Debug)]
pub struct Node {
    /// Last path component, used as the display name.
    pub name: Box<str>,
    /// Full `/`-separated relative path (used for fuzzy matching and titles).
    pub path: Box<str>,
    /// Index of the parent node, or [`NO_PARENT`].
    pub parent: u32,
    /// Nesting level (0 for top-level entries).
    pub depth: u16,
    /// Index into the file table for leaves, `None` for directories.
    pub file: Option<u32>,
    /// Exclusive end of this node's subtree in the arena.
    pub subtree_end: u32,
    /// Whether a directory is expanded (ignored while a filter is active).
    pub expanded: bool,
    /// Cached aggregate of everything below (or the file itself).
    pub total: Totals,
    /// Cached aggregate of the currently selected part.
    pub selected: Totals,
}

impl Node {
    #[inline]
    pub fn is_dir(&self) -> bool {
        self.file.is_none()
    }
}

/// Active fuzzy filter: per-node match flags (ancestors of matches included).
#[derive(Debug)]
struct Filter {
    matched: Vec<bool>,
    best: Option<usize>,
    match_count: usize,
}

enum Scope {
    Node(usize),
    All,
}

/// The flattened tree plus the visible-row projection.
#[derive(Debug)]
pub struct FlatTree {
    nodes: Vec<Node>,
    files: Vec<RepoFile>,
    visible: Vec<u32>,
    total: Totals,
    selected: Totals,
    filter: Option<Filter>,
}

#[derive(Default)]
struct Trie {
    children: BTreeMap<String, Trie>,
    file: Option<u32>,
}

#[inline]
fn idx32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Token weight of a file. Callers are expected to pre-compute `token_count`;
/// the `bytes / 4` estimate is only a safety net so the UI stays usable.
fn file_tokens(file: &RepoFile) -> u64 {
    file.token_count
        .map_or_else(|| file.size_bytes.div_ceil(4), |t| t as u64)
}

fn flatten(
    nodes: &mut Vec<Node>,
    trie: &Trie,
    parent: u32,
    depth: u16,
    parent_path: &str,
    files: &[RepoFile],
    expand_depth: u16,
) {
    // Directories first, then files; each group alphabetical (BTreeMap order).
    for want_dir in [true, false] {
        for (name, child) in &trie.children {
            if child.file.is_none() != want_dir {
                continue;
            }
            let idx = idx32(nodes.len());
            let path: Box<str> = if parent_path.is_empty() {
                name.as_str().into()
            } else {
                format!("{parent_path}/{name}").into()
            };
            let total = child
                .file
                .and_then(|fi| files.get(fi as usize))
                .map_or(Totals::ZERO, |f| Totals {
                    files: 1,
                    tokens: file_tokens(f),
                    bytes: f.size_bytes,
                });
            nodes.push(Node {
                name: name.as_str().into(),
                path: path.clone(),
                parent,
                depth,
                file: child.file,
                subtree_end: idx + 1,
                expanded: child.file.is_none() && depth < expand_depth,
                total,
                selected: Totals::ZERO,
            });
            if child.file.is_none() {
                flatten(
                    nodes,
                    child,
                    idx,
                    depth.saturating_add(1),
                    &path,
                    files,
                    expand_depth,
                );
            }
            let end = idx32(nodes.len());
            if let Some(node) = nodes.get_mut(idx as usize) {
                node.subtree_end = end;
            }
        }
    }
}

impl FlatTree {
    /// Builds the tree from scanned files. Everything starts **selected**.
    ///
    /// Directories with `depth < expand_depth` start expanded.
    pub fn new(files: Vec<RepoFile>, expand_depth: u16) -> Self {
        let mut root = Trie::default();
        for (i, file) in files.iter().enumerate() {
            let Ok(fi) = u32::try_from(i) else { break };
            let mut cur = &mut root;
            let mut components = 0usize;
            for component in file.relative_path.components() {
                if let Component::Normal(part) = component {
                    cur = cur
                        .children
                        .entry(part.to_string_lossy().into_owned())
                        .or_default();
                    components += 1;
                }
            }
            if components > 0 {
                cur.file = Some(fi);
            }
        }

        let mut nodes = Vec::with_capacity(files.len() + files.len() / 4 + 1);
        flatten(&mut nodes, &root, NO_PARENT, 0, "", &files, expand_depth);

        // Children always have larger indices than their parent (pre-order), so a
        // single reverse pass rolls the totals up.
        for i in (0..nodes.len()).rev() {
            let (parent, total) = (nodes[i].parent, nodes[i].total);
            if parent != NO_PARENT {
                if let Some(p) = nodes.get_mut(parent as usize) {
                    p.total = p.total.add(total);
                }
            }
        }
        for node in &mut nodes {
            node.selected = node.total;
        }

        let mut tree = Self {
            nodes,
            files,
            visible: Vec::new(),
            total: Totals::ZERO,
            selected: Totals::ZERO,
            filter: None,
        };
        tree.total = tree.sum_top_level(|n| n.total);
        tree.selected = tree.total;
        tree.rebuild_visible();
        tree
    }

    fn sum_top_level(&self, pick: impl Fn(&Node) -> Totals) -> Totals {
        let mut sum = Totals::ZERO;
        let mut i = 0;
        while let Some(node) = self.nodes.get(i) {
            sum = sum.add(pick(node));
            i = (node.subtree_end as usize).max(i + 1);
        }
        sum
    }

    // ----- read access -----------------------------------------------------

    #[inline]
    pub fn node(&self, idx: usize) -> Option<&Node> {
        self.nodes.get(idx)
    }

    /// The file behind a leaf node.
    pub fn file(&self, idx: usize) -> Option<&RepoFile> {
        let fi = self.nodes.get(idx)?.file?;
        self.files.get(fi as usize)
    }

    /// Node indices of the rows currently shown, in display order.
    #[inline]
    pub fn visible(&self) -> &[u32] {
        &self.visible
    }

    /// Everything found by the scan.
    #[inline]
    pub fn total(&self) -> Totals {
        self.total
    }

    /// Currently selected part. `O(1)`.
    #[inline]
    pub fn selected(&self) -> Totals {
        self.selected
    }

    pub fn selection(&self, idx: usize) -> Selection {
        match self.nodes.get(idx) {
            Some(n) if n.selected.files == 0 => Selection::Unselected,
            Some(n) if n.selected.files >= n.total.files => Selection::Selected,
            Some(_) => Selection::Partial,
            None => Selection::Unselected,
        }
    }

    pub fn parent(&self, idx: usize) -> Option<usize> {
        match self.nodes.get(idx)?.parent {
            NO_PARENT => None,
            p => Some(p as usize),
        }
    }

    // ----- visibility ------------------------------------------------------

    /// Rebuilds the visible-row list. Reuses the existing allocation and only
    /// walks rows that end up on screen.
    pub fn rebuild_visible(&mut self) {
        self.visible.clear();
        let n = self.nodes.len();
        let mut i = 0;
        match &self.filter {
            // Filtering force-expands every directory that contains a match.
            Some(filter) => {
                while i < n {
                    if filter.matched.get(i).copied().unwrap_or(false) {
                        self.visible.push(idx32(i));
                        i += 1;
                    } else {
                        i = (self.nodes[i].subtree_end as usize).max(i + 1);
                    }
                }
            }
            None => {
                while i < n {
                    self.visible.push(idx32(i));
                    let node = &self.nodes[i];
                    i = if node.is_dir() && !node.expanded {
                        (node.subtree_end as usize).max(i + 1)
                    } else {
                        i + 1
                    };
                }
            }
        }
    }

    /// Position of `node` within the visible rows, if it is shown.
    pub fn visible_position(&self, node: usize) -> Option<usize> {
        self.visible.binary_search(&idx32(node)).ok()
    }

    /// Visible position of `node`, or of its closest visible ancestor.
    pub fn nearest_visible(&self, node: usize) -> usize {
        let mut cur = node;
        loop {
            if let Some(pos) = self.visible_position(cur) {
                return pos;
            }
            match self.nodes.get(cur).map(|n| n.parent) {
                Some(p) if p != NO_PARENT => cur = p as usize,
                _ => return 0,
            }
        }
    }

    /// Expands or collapses a directory. Returns `true` if anything changed.
    pub fn set_expanded(&mut self, idx: usize, expanded: bool) -> bool {
        let Some(node) = self.nodes.get_mut(idx) else {
            return false;
        };
        if node.is_dir() && node.expanded != expanded {
            node.expanded = expanded;
            self.rebuild_visible();
            true
        } else {
            false
        }
    }

    // ----- filtering -------------------------------------------------------

    /// Applies a filter. `score` receives each file's relative path and returns
    /// `Some(rank)` for a match (higher is better).
    pub fn set_filter(&mut self, mut score: impl FnMut(&str) -> Option<u32>) {
        let n = self.nodes.len();
        let mut matched = vec![false; n];
        let mut best: Option<(u32, usize)> = None;
        let mut match_count = 0;
        for (i, node) in self.nodes.iter().enumerate() {
            if node.is_dir() {
                continue;
            }
            if let Some(s) = score(&node.path) {
                matched[i] = true;
                match_count += 1;
                if best.is_none_or(|(top, _)| s > top) {
                    best = Some((s, i));
                }
            }
        }
        // Reverse pass: propagate "has a matching descendant" to ancestors.
        for i in (0..n).rev() {
            if matched[i] {
                let p = self.nodes[i].parent;
                if p != NO_PARENT {
                    if let Some(flag) = matched.get_mut(p as usize) {
                        *flag = true;
                    }
                }
            }
        }
        self.filter = Some(Filter {
            matched,
            best: best.map(|(_, i)| i),
            match_count,
        });
        self.rebuild_visible();
    }

    pub fn clear_filter(&mut self) {
        if self.filter.take().is_some() {
            self.rebuild_visible();
        }
    }

    #[inline]
    pub fn filter_active(&self) -> bool {
        self.filter.is_some()
    }

    /// Number of files matching the active filter.
    pub fn filter_match_count(&self) -> usize {
        self.filter.as_ref().map_or(0, |f| f.match_count)
    }

    /// Node index of the best-ranked match, if any.
    pub fn filter_best(&self) -> Option<usize> {
        self.filter.as_ref().and_then(|f| f.best)
    }

    #[inline]
    fn passes_filter(&self, idx: usize) -> bool {
        self.filter
            .as_ref()
            .is_none_or(|f| f.matched.get(idx).copied().unwrap_or(false))
    }

    // ----- selection -------------------------------------------------------

    /// `true` when every file in `start..end` that passes the filter is selected.
    fn range_all_selected(&self, start: usize, end: usize) -> bool {
        (start..end.min(self.nodes.len())).all(|i| {
            let node = &self.nodes[i];
            node.is_dir() || !self.passes_filter(i) || node.selected.files > 0
        })
    }

    /// Toggles a file or (recursively) a folder.
    ///
    /// If everything (matching the filter) below is selected the subtree is
    /// cleared, otherwise it is fully selected.
    pub fn toggle_selection(&mut self, idx: usize) {
        let Some(end) = self.nodes.get(idx).map(|n| n.subtree_end as usize) else {
            return;
        };
        let select = !self.range_all_selected(idx, end);
        self.apply(Scope::Node(idx), |_| Some(select));
    }

    /// Select every file (matching the filter); if all are already selected,
    /// deselect them instead.
    pub fn toggle_all(&mut self) {
        let select = !self.range_all_selected(0, self.nodes.len());
        self.apply(Scope::All, |_| Some(select));
    }

    /// Flips the selection of every file (matching the filter).
    pub fn invert_selection(&mut self) {
        self.apply(Scope::All, |current| Some(!current));
    }

    /// Core mutation: remap leaf states in a range, then restore the cached
    /// aggregates (`O(range)` for the range, `O(depth)` for ancestors).
    fn apply(&mut self, scope: Scope, mut remap: impl FnMut(bool) -> Option<bool>) {
        let n = self.nodes.len();
        let (start, end) = match scope {
            Scope::Node(i) => match self.nodes.get(i) {
                Some(node) => (i, (node.subtree_end as usize).min(n)),
                None => return,
            },
            Scope::All => (0, n),
        };
        if start >= end {
            return;
        }
        let before = self.nodes[start].selected;

        // 1. Leaves.
        for i in start..end {
            if self.nodes[i].is_dir() || !self.passes_filter(i) {
                continue;
            }
            let node = &mut self.nodes[i];
            if let Some(new_state) = remap(node.selected.files > 0) {
                node.selected = if new_state { node.total } else { Totals::ZERO };
            }
        }

        // 2. Directories inside the range: zero, then roll up (children come
        //    after their parent, so iterating in reverse sees children first).
        for node in &mut self.nodes[start..end] {
            if node.is_dir() {
                node.selected = Totals::ZERO;
            }
        }
        for i in (start..end).rev() {
            let (parent, selected) = (self.nodes[i].parent, self.nodes[i].selected);
            if parent != NO_PARENT && (parent as usize) >= start {
                if let Some(p) = self.nodes.get_mut(parent as usize) {
                    p.selected = p.selected.add(selected);
                }
            }
        }

        // 3. Ancestors outside the range + global counters.
        match scope {
            Scope::Node(_) => {
                let after = self.nodes[start].selected;
                let mut p = self.nodes[start].parent;
                while p != NO_PARENT {
                    let Some(ancestor) = self.nodes.get_mut(p as usize) else {
                        break;
                    };
                    ancestor.selected = ancestor.selected.sub(before).add(after);
                    p = ancestor.parent;
                }
                self.selected = self.selected.sub(before).add(after);
            }
            Scope::All => {
                self.selected = self.sum_top_level(|n| n.selected);
            }
        }
    }

    // ----- results ---------------------------------------------------------

    /// Consumes the tree, returning the selected files in their **original**
    /// (scanner) order so the generated prompt stays deterministic.
    pub fn into_selected(self) -> Vec<RepoFile> {
        let mut keep = vec![false; self.files.len()];
        for node in &self.nodes {
            if let Some(fi) = node.file {
                if node.selected.files > 0 {
                    if let Some(flag) = keep.get_mut(fi as usize) {
                        *flag = true;
                    }
                }
            }
        }
        self.files
            .into_iter()
            .zip(keep)
            .filter_map(|(file, keep)| keep.then_some(file))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn rf(path: &str, tokens: usize, bytes: u64) -> RepoFile {
        let mut f = RepoFile::new(
            PathBuf::from(path),
            PathBuf::from("/abs").join(path),
            bytes,
            String::new(),
        );
        f.token_count = Some(tokens);
        f
    }

    fn sample() -> FlatTree {
        FlatTree::new(
            vec![
                rf("Cargo.toml", 10, 100),
                rf("crates/a/x.rs", 20, 200),
                rf("crates/a/y.rs", 30, 300),
                rf("crates/b/z.rs", 40, 400),
                rf("src/lib.rs", 50, 500),
                rf("src/main.rs", 60, 600),
            ],
            u16::MAX,
        )
    }

    fn rows(tree: &FlatTree) -> Vec<String> {
        tree.visible()
            .iter()
            .filter_map(|&i| tree.node(i as usize))
            .map(|n| format!("{}{}", n.depth, n.name))
            .collect()
    }

    fn idx_of(tree: &FlatTree, path: &str) -> usize {
        (0..tree.nodes.len())
            .find(|&i| &*tree.nodes[i].path == path)
            .unwrap_or_else(|| panic!("no node {path}"))
    }

    /// Recomputes every aggregate from the leaves and compares with the caches.
    fn assert_invariants(tree: &FlatTree) {
        let n = tree.nodes.len();
        let mut expect = vec![Totals::ZERO; n];
        for i in 0..n {
            if tree.nodes[i].is_dir() {
                continue;
            }
            if tree.nodes[i].selected.files > 0 {
                expect[i] = tree.nodes[i].total;
            }
        }
        for i in (0..n).rev() {
            let p = tree.nodes[i].parent;
            if p != NO_PARENT {
                let e = expect[i];
                expect[p as usize] = expect[p as usize].add(e);
            }
        }
        for i in 0..n {
            assert_eq!(
                tree.nodes[i].selected, expect[i],
                "selected aggregate mismatch at {}",
                tree.nodes[i].path
            );
        }
        assert_eq!(tree.selected, tree.sum_top_level(|nd| nd.selected));
        assert_eq!(tree.selected, expect_top(tree, &expect));
    }

    fn expect_top(tree: &FlatTree, expect: &[Totals]) -> Totals {
        let mut sum = Totals::ZERO;
        let mut i = 0;
        while i < tree.nodes.len() {
            sum = sum.add(expect[i]);
            i = tree.nodes[i].subtree_end as usize;
        }
        sum
    }

    #[test]
    fn builds_dirs_first_in_preorder() {
        let tree = sample();
        assert_eq!(
            rows(&tree),
            [
                "0crates",
                "1a",
                "2x.rs",
                "2y.rs",
                "1b",
                "2z.rs",
                "0src",
                "1lib.rs",
                "1main.rs",
                "0Cargo.toml"
            ]
        );
    }

    #[test]
    fn subtree_ranges_are_contiguous() {
        let tree = sample();
        let crates = idx_of(&tree, "crates");
        // crates, a, x, y, b, z -> 6 nodes
        assert_eq!(tree.nodes[crates].subtree_end as usize - crates, 6);
        let a = idx_of(&tree, "crates/a");
        assert_eq!(tree.nodes[a].subtree_end as usize - a, 3);
    }

    #[test]
    fn aggregates_roll_up_and_everything_starts_selected() {
        let tree = sample();
        let crates = idx_of(&tree, "crates");
        assert_eq!(tree.nodes[crates].total.tokens, 90);
        assert_eq!(tree.nodes[crates].total.bytes, 900);
        assert_eq!(tree.nodes[crates].total.files, 3);
        assert_eq!(tree.total().tokens, 210);
        assert_eq!(tree.selected(), tree.total());
        assert_eq!(tree.selection(crates), Selection::Selected);
        assert_invariants(&tree);
    }

    #[test]
    fn toggling_a_file_makes_ancestors_partial() {
        let mut tree = sample();
        let x = idx_of(&tree, "crates/a/x.rs");
        tree.toggle_selection(x);

        assert_eq!(tree.selection(x), Selection::Unselected);
        assert_eq!(tree.selection(idx_of(&tree, "crates/a")), Selection::Partial);
        assert_eq!(tree.selection(idx_of(&tree, "crates")), Selection::Partial);
        assert_eq!(tree.selection(idx_of(&tree, "src")), Selection::Selected);
        assert_eq!(tree.selected().tokens, 190);
        assert_eq!(tree.selected().files, 5);
        assert_invariants(&tree);

        tree.toggle_selection(x);
        assert_eq!(tree.selected(), tree.total());
        assert_eq!(tree.selection(idx_of(&tree, "crates")), Selection::Selected);
        assert_invariants(&tree);
    }

    #[test]
    fn toggling_a_folder_cascades_down_and_up() {
        let mut tree = sample();
        let crates = idx_of(&tree, "crates");
        tree.toggle_selection(crates);

        for p in ["crates", "crates/a", "crates/a/x.rs", "crates/b/z.rs"] {
            assert_eq!(tree.selection(idx_of(&tree, p)), Selection::Unselected, "{p}");
        }
        assert_eq!(tree.selected().tokens, 120);
        assert_eq!(tree.selected().files, 3);
        assert_invariants(&tree);

        // Partial folder -> select everything.
        let y = idx_of(&tree, "crates/a/y.rs");
        tree.toggle_selection(y);
        assert_eq!(tree.selection(crates), Selection::Partial);
        tree.toggle_selection(crates);
        assert_eq!(tree.selection(crates), Selection::Selected);
        assert_eq!(tree.selected(), tree.total());
        assert_invariants(&tree);
    }

    #[test]
    fn collapsed_folder_selection_still_reaches_hidden_children() {
        let mut tree = sample();
        let crates = idx_of(&tree, "crates");
        assert!(tree.set_expanded(crates, false));
        assert_eq!(rows(&tree).len(), 5);

        tree.toggle_selection(crates);
        assert_eq!(tree.selection(idx_of(&tree, "crates/b/z.rs")), Selection::Unselected);
        assert_invariants(&tree);
    }

    #[test]
    fn expand_collapse_changes_rows_not_selection() {
        let mut tree = FlatTree::new(
            vec![rf("a/b/c.rs", 1, 1), rf("a/d.rs", 2, 2), rf("e.rs", 3, 3)],
            0,
        );
        assert_eq!(rows(&tree), ["0a", "0e.rs"]);
        let a = idx_of(&tree, "a");
        assert!(tree.set_expanded(a, true));
        assert_eq!(rows(&tree), ["0a", "1b", "1d.rs", "0e.rs"]);
        assert!(!tree.set_expanded(a, true), "no-op reports false");
        assert!(!tree.set_expanded(idx_of(&tree, "e.rs"), true), "files cannot expand");
        assert_eq!(tree.selected(), tree.total());
    }

    #[test]
    fn nearest_visible_falls_back_to_collapsed_ancestor() {
        let mut tree = sample();
        let z = idx_of(&tree, "crates/b/z.rs");
        let crates = idx_of(&tree, "crates");
        assert!(tree.set_expanded(crates, false));
        let pos = tree.nearest_visible(z);
        assert_eq!(tree.visible()[pos] as usize, crates);
    }

    #[test]
    fn filter_shows_matches_with_ancestors_and_forces_expansion() {
        let mut tree = sample();
        let crates = idx_of(&tree, "crates");
        tree.set_expanded(crates, false);

        tree.set_filter(|p| p.contains("z.rs").then_some(1));
        assert!(tree.filter_active());
        assert_eq!(tree.filter_match_count(), 1);
        assert_eq!(rows(&tree), ["0crates", "1b", "2z.rs"]);
        assert_eq!(tree.filter_best(), Some(idx_of(&tree, "crates/b/z.rs")));

        tree.clear_filter();
        assert!(!tree.filter_active());
        // `crates` is collapsed again once the filter is gone.
        assert_eq!(rows(&tree).len(), 5);
    }

    #[test]
    fn filter_best_prefers_highest_score() {
        let mut tree = sample();
        tree.set_filter(|p| match p {
            "src/lib.rs" => Some(5),
            "src/main.rs" => Some(9),
            _ => None,
        });
        assert_eq!(tree.filter_best(), Some(idx_of(&tree, "src/main.rs")));
    }

    #[test]
    fn folder_toggle_with_filter_only_touches_matches() {
        let mut tree = sample();
        tree.set_filter(|p| p.ends_with("x.rs").then_some(1));
        let a = idx_of(&tree, "crates/a");

        // Only x.rs is in scope and it is selected -> toggling deselects just x.rs.
        tree.toggle_selection(a);
        assert_eq!(tree.selection(idx_of(&tree, "crates/a/x.rs")), Selection::Unselected);
        assert_eq!(tree.selection(idx_of(&tree, "crates/a/y.rs")), Selection::Selected);
        assert_eq!(tree.selection(a), Selection::Partial);
        assert_invariants(&tree);

        // Toggling again re-selects x.rs only.
        tree.toggle_selection(a);
        assert_eq!(tree.selected(), tree.total());
        assert_invariants(&tree);
    }

    #[test]
    fn toggle_all_and_invert() {
        let mut tree = sample();
        tree.toggle_all();
        assert_eq!(tree.selected(), Totals::ZERO);
        assert_invariants(&tree);

        tree.toggle_all();
        assert_eq!(tree.selected(), tree.total());
        assert_invariants(&tree);

        let x = idx_of(&tree, "crates/a/x.rs");
        tree.toggle_selection(x);
        tree.invert_selection();
        assert_eq!(tree.selected().files, 1);
        assert_eq!(tree.selected().tokens, 20);
        assert_eq!(tree.selection(x), Selection::Selected);
        assert_invariants(&tree);
    }

    #[test]
    fn toggle_all_respects_filter() {
        let mut tree = sample();
        tree.set_filter(|p| p.starts_with("src/").then_some(1));
        tree.toggle_all(); // deselect the two src files only
        assert_eq!(tree.selected().files, 4);
        assert_eq!(tree.selection(idx_of(&tree, "src")), Selection::Unselected);
        assert_eq!(tree.selection(idx_of(&tree, "Cargo.toml")), Selection::Selected);
        assert_invariants(&tree);
    }

    #[test]
    fn into_selected_preserves_input_order() {
        let mut tree = sample();
        tree.toggle_selection(idx_of(&tree, "crates/a"));
        let out: Vec<_> = tree
            .into_selected()
            .into_iter()
            .map(|f| f.relative_path.to_string_lossy().into_owned())
            .collect();
        assert_eq!(out, ["Cargo.toml", "crates/b/z.rs", "src/lib.rs", "src/main.rs"]);
    }

    #[test]
    fn missing_token_counts_fall_back_to_a_byte_estimate() {
        let file = RepoFile::new(PathBuf::from("a.txt"), PathBuf::from("/a.txt"), 41, String::new());
        let tree = FlatTree::new(vec![file], 1);
        assert_eq!(tree.total().tokens, 11); // ceil(41 / 4)
    }

    #[test]
    fn empty_tree_is_safe() {
        let mut tree = FlatTree::new(Vec::new(), 1);
        assert!(tree.visible().is_empty());
        tree.toggle_all();
        tree.invert_selection();
        tree.toggle_selection(0);
        tree.set_filter(|_| Some(1));
        assert_eq!(tree.filter_match_count(), 0);
        assert!(tree.into_selected().is_empty());
    }

    #[test]
    fn many_random_toggles_keep_aggregates_consistent() {
        let files: Vec<_> = (0..200)
            .map(|i| rf(&format!("d{}/s{}/f{}.rs", i % 5, i % 7, i), i + 1, (i as u64 + 1) * 10))
            .collect();
        let mut tree = FlatTree::new(files, 1);
        let n = tree.nodes.len();
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        for step in 0..500 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let idx = (seed % n as u64) as usize;
            match step % 11 {
                0 => tree.toggle_all(),
                1 => tree.invert_selection(),
                2 => tree.set_filter(|p| (p.len() % 3 == 0).then_some(1)),
                3 => tree.clear_filter(),
                _ => tree.toggle_selection(idx),
            }
            assert_invariants(&tree);
        }
    }
}
