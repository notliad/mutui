use mutui_common::Track;

pub struct Queue {
    pub tracks: Vec<Track>,
    pub current: usize,
}

impl Queue {
    pub fn new() -> Self {
        Self {
            tracks: Vec::new(),
            current: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    pub fn current_track(&self) -> Option<&Track> {
        self.tracks.get(self.current)
    }

    pub fn add(&mut self, track: Track) {
        self.tracks.push(track);
    }

    /// Insert a track right after the current one
    pub fn insert_next(&mut self, track: Track) {
        let pos = if self.tracks.is_empty() {
            0
        } else {
            (self.current + 1).min(self.tracks.len())
        };
        self.tracks.insert(pos, track);
    }

    pub fn remove(&mut self, index: usize) -> Option<Track> {
        if index >= self.tracks.len() {
            return None;
        }
        let track = self.tracks.remove(index);
        // Adjust current index if needed
        if index < self.current {
            self.current = self.current.saturating_sub(1);
        } else if index == self.current && self.current >= self.tracks.len() && !self.tracks.is_empty() {
            self.current = self.tracks.len() - 1;
        }
        Some(track)
    }

    pub fn clear(&mut self) {
        self.tracks.clear();
        self.current = 0;
    }

    pub fn move_track(&mut self, from: usize, to: usize) {
        if from >= self.tracks.len() || to >= self.tracks.len() {
            return;
        }
        let track = self.tracks.remove(from);
        self.tracks.insert(to, track);

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
        if index < self.tracks.len() {
            self.current = index;
            true
        } else {
            false
        }
    }

    /// Advance to the next track. Returns true if there is one.
    pub fn next(&mut self) -> bool {
        if self.current + 1 < self.tracks.len() {
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
}
