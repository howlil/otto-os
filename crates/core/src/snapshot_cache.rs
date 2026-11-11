#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotBudget {
    pub max_entries: usize,
    pub max_bytes: usize,
}

impl SnapshotBudget {
    pub const GENIE: Self = Self {
        max_entries: 4,
        max_bytes: 64 * 1024 * 1024,
    };
}

#[derive(Debug, Clone)]
pub struct SnapshotCache<K, V> {
    budget: SnapshotBudget,
    entries: Vec<(K, V, usize)>,
}

impl<K: PartialEq + Copy, V> SnapshotCache<K, V> {
    #[must_use]
    pub const fn new(budget: SnapshotBudget) -> Self {
        Self {
            budget,
            entries: Vec::new(),
        }
    }

    pub fn insert(&mut self, key: K, value: V, bytes: usize) -> bool {
        self.remove(&key);
        if bytes > self.budget.max_bytes || self.budget.max_entries == 0 {
            return false;
        }
        while self.entries.len() >= self.budget.max_entries
            || self.total_bytes() + bytes > self.budget.max_bytes
        {
            self.entries.remove(0);
        }
        self.entries.push((key, value, bytes));
        true
    }

    pub fn get(&mut self, key: &K) -> Option<&V> {
        let index = self.entries.iter().position(|entry| entry.0 == *key)?;
        let entry = self.entries.remove(index);
        self.entries.push(entry);
        self.entries.last().map(|entry| &entry.1)
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        let index = self.entries.iter().position(|entry| entry.0 == *key)?;
        Some(self.entries.remove(index).1)
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&K) -> bool) {
        self.entries.retain(|entry| keep(&entry.0));
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    #[must_use]
    pub fn total_bytes(&self) -> usize {
        self.entries.iter().map(|entry| entry.2).sum()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[must_use]
    pub fn keys(&self) -> Vec<K> {
        self.entries.iter().map(|entry| entry.0).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{SnapshotBudget, SnapshotCache};

    const SMALL: SnapshotBudget = SnapshotBudget {
        max_entries: 3,
        max_bytes: 100,
    };

    #[test]
    fn evicts_least_recently_used_entry_when_entry_count_is_full() {
        let mut cache = SnapshotCache::new(SMALL);
        assert!(cache.insert(1, "a", 10));
        assert!(cache.insert(2, "b", 10));
        assert!(cache.insert(3, "c", 10));
        assert_eq!(cache.get(&1), Some(&"a"));
        assert!(cache.insert(4, "d", 10));
        assert_eq!(cache.keys(), vec![3, 1, 4]);
    }

    #[test]
    fn evicts_until_the_byte_budget_fits() {
        let mut cache = SnapshotCache::new(SMALL);
        assert!(cache.insert(1, (), 40));
        assert!(cache.insert(2, (), 40));
        assert!(cache.insert(3, (), 50));
        assert_eq!(cache.keys(), vec![2, 3]);
        assert_eq!(cache.total_bytes(), 90);
    }

    #[test]
    fn rejects_a_single_snapshot_larger_than_the_budget() {
        let mut cache = SnapshotCache::new(SMALL);
        assert!(cache.insert(1, (), 10));
        assert!(!cache.insert(2, (), 101));
        assert_eq!(cache.keys(), vec![1]);
    }

    #[test]
    fn replacing_a_key_does_not_double_count_its_bytes() {
        let mut cache = SnapshotCache::new(SMALL);
        assert!(cache.insert(1, "old", 60));
        assert!(cache.insert(1, "new", 60));
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.total_bytes(), 60);
        assert_eq!(cache.get(&1), Some(&"new"));
    }

    #[test]
    fn retain_and_clear_drop_entries() {
        let mut cache = SnapshotCache::new(SMALL);
        assert!(cache.insert(1, (), 10));
        assert!(cache.insert(2, (), 10));
        cache.retain(|key| *key == 2);
        assert_eq!(cache.keys(), vec![2]);
        assert_eq!(cache.remove(&2), Some(()));
        assert!(cache.is_empty());
        assert!(cache.insert(3, (), 10));
        cache.clear();
        assert_eq!(cache.total_bytes(), 0);
    }
}
