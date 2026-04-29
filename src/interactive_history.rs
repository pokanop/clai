//! In-memory session history policy (FR-2–FR-5, NFR-2).

/// Default cap for retained lines when config does not override (FR-5).
pub const DEFAULT_HISTORY_MAX_ENTRIES: usize = 1000;

/// Minimum configurable cap; values below are clamped (FR-5).
pub const MIN_HISTORY_MAX_ENTRIES: usize = 100;

/// Upper bound on total retained character data (NFR-2).
pub const MAX_HISTORY_CHARS_BUDGET: usize = 4 * 1024 * 1024;

#[must_use]
pub fn sanitize_history_max_entries(n: usize) -> usize {
    n.max(MIN_HISTORY_MAX_ENTRIES)
}

#[derive(Debug, Default)]
pub struct InteractiveHistoryStore {
    entries: std::collections::VecDeque<String>,
    total_chars: usize,
    max_entries: usize,
    max_chars: usize,
}

impl InteractiveHistoryStore {
    pub fn new(max_entries: usize) -> Self {
        Self::with_limits(max_entries, MAX_HISTORY_CHARS_BUDGET)
    }

    pub(crate) fn with_limits(max_entries: usize, max_chars: usize) -> Self {
        Self {
            entries: std::collections::VecDeque::new(),
            total_chars: 0,
            max_entries,
            max_chars,
        }
    }

    /// Append a qualifying trimmed request line. Returns `true` if a new entry was stored (FR-4).
    pub fn push_qualifying(&mut self, line: &str) -> bool {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return false;
        }
        if self
            .entries
            .back()
            .is_some_and(|last| last.as_str() == trimmed)
        {
            return false;
        }

        self.entries.push_back(trimmed.to_string());
        self.total_chars += trimmed.chars().count();

        self.evict_to_bounds();
        true
    }

    fn evict_from_front(&mut self, n: usize) {
        for _ in 0..n {
            if let Some(old) = self.entries.pop_front() {
                self.total_chars = self.total_chars.saturating_sub(old.chars().count());
            }
        }
    }

    fn evict_to_bounds(&mut self) {
        while self.entries.len() > self.max_entries {
            self.evict_from_front(1);
        }
        while self.total_chars > self.max_chars && !self.entries.is_empty() {
            self.evict_from_front(1);
        }
        while self.entries.len() > self.max_entries && !self.entries.is_empty() {
            self.evict_from_front(1);
        }
    }

    pub fn len_entries(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub fn entries(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excludes_consecutive_duplicate() {
        let mut h = InteractiveHistoryStore::new(1000);
        assert!(h.push_qualifying("a"));
        assert!(!h.push_qualifying("a"));
        assert_eq!(h.len_entries(), 1);
    }

    #[test]
    fn drops_oldest_when_over_count_cap() {
        let mut h = InteractiveHistoryStore::new(3);
        assert!(h.push_qualifying("a"));
        assert!(h.push_qualifying("b"));
        assert!(h.push_qualifying("c"));
        assert!(h.push_qualifying("d"));
        let v: Vec<&str> = h.entries().collect();
        assert_eq!(v, vec!["b", "c", "d"]);
    }

    #[test]
    fn evicts_oldest_when_char_budget_exceeded() {
        let mut h = InteractiveHistoryStore::with_limits(100, 10);
        assert!(h.push_qualifying("0123456789")); // 10 chars
        assert!(h.push_qualifying("ABCDEFGHIJ")); // distinct; 20 total — evicts first
        let v: Vec<&str> = h.entries().collect();
        assert_eq!(v, vec!["ABCDEFGHIJ"]);
    }

    #[test]
    fn sanitize_raises_small_values() {
        assert_eq!(sanitize_history_max_entries(10), MIN_HISTORY_MAX_ENTRIES);
        assert_eq!(sanitize_history_max_entries(200), 200);
    }
}
