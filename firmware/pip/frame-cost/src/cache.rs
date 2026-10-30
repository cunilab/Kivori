//! Flash cache model: the ESP32-C3's external-memory cache, shared by instruction fetches (IROM
//! 0x4200_0000) and data reads (DROM 0x3C00_0000). Set-associative, true LRU, tags only.

/// Cache geometry. The C3 has 16 KiB for instructions and read-only data; ESP-IDF's C3 cache
/// configuration offers 16 or 32 byte lines and 8 ways. Treat the geometry as an assumption.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CacheGeometry {
    /// Total size in bytes.
    pub size: u32,
    /// Ways per set.
    pub ways: u32,
    /// Line size in bytes (power of two).
    pub line: u32,
}

impl CacheGeometry {
    /// The assumed ESP32-C3 flash cache.
    pub const C3: CacheGeometry = CacheGeometry {
        size: 16 * 1024,
        ways: 8,
        line: 32,
    };
}

/// Tags and recency stamps of a cache.
pub struct Cache {
    geometry: CacheGeometry,
    sets: u32,
    /// `tag + 1` per way, 0 = empty.
    tags: Vec<u32>,
    stamps: Vec<u64>,
    clock: u64,
}

impl Cache {
    /// An empty cache.
    pub fn new(geometry: CacheGeometry) -> Self {
        let sets = geometry.size / (geometry.ways * geometry.line);
        assert!(sets.is_power_of_two() && geometry.line.is_power_of_two());
        let slots = (sets * geometry.ways) as usize;
        Cache {
            geometry,
            sets,
            tags: vec![0; slots],
            stamps: vec![0; slots],
            clock: 0,
        }
    }

    /// Line number of `addr`.
    pub fn line_of(&self, addr: u32) -> u32 {
        addr / self.geometry.line
    }

    /// Touches the line holding `addr`; returns whether it was already cached.
    pub fn access(&mut self, addr: u32) -> bool {
        self.clock += 1;
        let line = self.line_of(addr);
        let set = (line % self.sets) as usize;
        let tag = line / self.sets + 1;
        let ways = self.geometry.ways as usize;
        let base = set * ways;
        let mut victim = base;
        for way in base..base + ways {
            if self.tags[way] == tag {
                self.stamps[way] = self.clock;
                return true;
            }
            if self.tags[way] == 0 {
                victim = way;
                self.stamps[victim] = 0;
            } else if self.tags[victim] != 0 && self.stamps[way] < self.stamps[victim] {
                victim = way;
            }
        }
        self.tags[victim] = tag;
        self.stamps[victim] = self.clock;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hits_after_a_miss_and_lru_evicts_the_oldest() {
        let g = CacheGeometry {
            size: 256,
            ways: 2,
            line: 32,
        }; // 4 sets
        let mut c = Cache::new(g);
        let stride = 4 * 32; // same set
        assert!(!c.access(0));
        assert!(c.access(4)); // same line
        assert!(!c.access(stride));
        assert!(c.access(0));
        assert!(!c.access(2 * stride)); // evicts `stride` (oldest), keeps 0
        assert!(c.access(0));
        assert!(!c.access(stride));
    }

    #[test]
    fn c3_geometry_has_64_sets() {
        let c = Cache::new(CacheGeometry::C3);
        assert_eq!(c.sets, 64);
    }
}
