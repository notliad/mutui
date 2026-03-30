use mutui_common::Track;

#[derive(Clone)]
pub struct QueueItem {
    pub track: Track,
    pub is_autoplay: bool,
}

pub struct Queue {
    items: Vec<QueueItem>,
    pub current: usize,
}

impl Queue {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            current: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn current_track(&self) -> Option<&Track> {
        self.items.get(self.current).map(|item| &item.track)
    }

    pub fn add(&mut self, track: Track) {
        self.add_with_flag(track, false);
    }

    pub fn add_autoplay(&mut self, track: Track) {
        self.add_with_flag(track, true);
    }

    fn add_with_flag(&mut self, track: Track, is_autoplay: bool) {
        self.items.push(QueueItem { track, is_autoplay });
    }

    /// Insert a track right after the current one
    pub fn insert_next(&mut self, track: Track) {
        let pos = if self.items.is_empty() {
            0
        } else {
            (self.current + 1).min(self.items.len())
        };
        self.items.insert(
            pos,
            QueueItem {
                track,
                is_autoplay: false,
            },
        );
    }

    pub fn remove(&mut self, index: usize) -> Option<Track> {
        if index >= self.items.len() {
            return None;
        }
        let item = self.items.remove(index);
        // Adjust current index if needed
        if index < self.current {
            self.current = self.current.saturating_sub(1);
        } else if index == self.current
            && self.current >= self.items.len()
            && !self.items.is_empty()
        {
            self.current = self.items.len() - 1;
        }
        Some(item.track)
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.current = 0;
    }

    pub fn move_track(&mut self, from: usize, to: usize) {
        if from >= self.items.len() || to >= self.items.len() {
            return;
        }
        let item = self.items.remove(from);
        self.items.insert(to, item);

        // Adjust current index
        if self.current == from {
            self.current = to;
        } else if from < self.current && to >= self.current {
            self.current = self.current.saturating_sub(1);
        } else if from > self.current && to <= self.current {
            self.current += 1;
        }
    }

    pub fn set_index(&mut self, index: usize) -> bool {
        if index < self.items.len() {
            self.current = index;
            true
        } else {
            false
        }
    }

    /// Advance to the next track. Returns true if there is one.
    pub fn next(&mut self) -> bool {
        if self.current + 1 < self.items.len() {
            self.current += 1;
            true
        } else {
            false
        }
    }

    /// Go to the previous track. Returns true if there is one.
    pub fn previous(&mut self) -> bool {
        if self.current > 0 {
            self.current -= 1;
            true
        } else {
            false
        }
    }

    pub fn tracks(&self) -> Vec<Track> {
        self.items.iter().map(|item| item.track.clone()).collect()
    }

    pub fn autoplay_indices(&self) -> Vec<usize> {
        self.items
            .iter()
            .enumerate()
            .filter_map(|(idx, item)| item.is_autoplay.then_some(idx))
            .collect()
    }
}
