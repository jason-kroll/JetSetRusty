//! Safe-Rust vendoring of glibc's `rand()`/`srand()` (the TYPE_3
//! additive-feedback generator from glibc's stdlib/random_r.c), so the crate
//! needs no libc FFI at all. Replay determinism depends on producing exactly
//! libc's sequence for a given seed; the test below pins reference vectors
//! captured from the real libc rand() (glibc 2.36, x86-64).

use std::cell::RefCell;

// glibc random_r.c: TYPE_3 has degree 31, separation 3.
const DEG: usize = 31;
const SEP: usize = 3;

struct GlibcRand {
    state: [i32; DEG],
    f: usize, // fptr
    r: usize, // rptr
}

impl GlibcRand {
    // glibc treats rand() before srand() as srand(1).
    fn new() -> Self {
        let mut r = GlibcRand {
            state: [0; DEG],
            f: SEP,
            r: 0,
        };
        r.srand(1);
        r
    }

    fn srand(&mut self, seed: u32) {
        // state[0] = seed == 0 ? 1 : seed, reinterpreted as int32 with C's
        // wrapping unsigned->signed semantics (from_ne_bytes is bit-exact).
        let mut word: i32 = if seed == 0 {
            1
        } else {
            i32::from_ne_bytes(seed.to_ne_bytes())
        };
        self.state[0] = word;
        for slot in self.state[1..].iter_mut() {
            // Schrage's trick for (16807 * word) mod 2147483647, computed in
            // i64 and truncated back to i32 exactly like glibc's int32_t word
            // (C's / and % truncate toward zero, as does Rust's).
            let w = i64::from(word);
            let hi = w / 127773;
            let lo = w % 127773;
            let mut next = 16807 * lo - 2836 * hi;
            if next < 0 {
                next += 2147483647;
            }
            // Truncate to the low 32 bits exactly like glibc's int32_t word.
            word = i32::from_ne_bytes(
                u32::try_from(next & 0xffff_ffff)
                    .expect("masked to 32 bits")
                    .to_ne_bytes(),
            );
            *slot = word;
        }
        self.f = SEP;
        self.r = 0;
        // glibc discards rand_deg * 10 = 310 outputs after seeding.
        for _ in 0..DEG * 10 {
            self.rand();
        }
    }

    fn rand(&mut self) -> i32 {
        self.state[self.f] = self.state[self.f].wrapping_add(self.state[self.r]);
        let result = (self.state[self.f] >> 1) & 0x7fffffff;
        self.f += 1;
        if self.f == DEG {
            self.f = 0;
        }
        self.r += 1;
        if self.r == DEG {
            self.r = 0;
        }
        result
    }
}

thread_local! {
    static RNG: RefCell<GlibcRand> = RefCell::new(GlibcRand::new());
}

/// libc srand(): seed the generator. Main thread only (all game code runs there).
pub(crate) fn srand(seed: u32) {
    RNG.with(|r| r.borrow_mut().srand(seed));
}

/// libc rand(): next pseudo-random value in [0, 2^31).
pub(crate) fn rand() -> i32 {
    RNG.with(|r| r.borrow_mut().rand())
}

#[cfg(test)]
mod tests {
    // First 12 outputs after srand(seed), captured from real glibc rand().
    // Includes seed 0 (glibc maps it to 1) and seeds >= 2^31 (negative int32
    // seed path).
    const VECTORS: &[(u32, [i32; 12])] = &[
        (
            0,
            [
                1804289383, 846930886, 1681692777, 1714636915, 1957747793, 424238335, 719885386,
                1649760492, 596516649, 1189641421, 1025202362, 1350490027,
            ],
        ),
        (
            1,
            [
                1804289383, 846930886, 1681692777, 1714636915, 1957747793, 424238335, 719885386,
                1649760492, 596516649, 1189641421, 1025202362, 1350490027,
            ],
        ),
        (
            42,
            [
                71876166, 708592740, 1483128881, 907283241, 442951012, 537146758, 1366999021,
                1854614940, 647800535, 53523743, 783815874, 1643643143,
            ],
        ),
        (
            12345,
            [
                383100999, 858300821, 357768173, 455528251, 133005921, 116285904, 591987137,
                102557902, 689413528, 585691128, 789708827, 477528897,
            ],
        ),
        (
            1700000000,
            [
                280491303, 626398474, 65378190, 821524244, 1465218044, 821971775, 895036163,
                1845157644, 705984302, 785305673, 2115933942, 1626383116,
            ],
        ),
        (
            2147483647,
            [
                1065668062, 2142264300, 1066566375, 1064012770, 2141034222, 1065509725, 2135810236,
                2139491828, 2138030783, 2133535487, 1063823214, 1062867731,
            ],
        ),
        (
            4000000000,
            [
                1111130805, 1380198982, 1306113867, 1557021840, 1956528986, 1880734592, 1640346462,
                817433120, 1333394793, 322755644, 1760664040, 104305595,
            ],
        ),
    ];

    #[test]
    fn matches_libc_rand() {
        for (seed, expected) in VECTORS {
            super::srand(*seed);
            for want in expected {
                assert_eq!(&super::rand(), want, "seed {seed}");
            }
        }
    }
}
