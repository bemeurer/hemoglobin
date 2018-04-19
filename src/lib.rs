extern crate base64;
extern crate bit_vec;
extern crate num;
extern crate rand;
extern crate rustty;

use std::fmt;
use std::collections::HashSet;

use bit_vec::BitVec;
use rand::Rng;
use rustty::ui::Widget;
use rustty::CellAccessor;

type Cell = (usize, usize);
type CellSet = HashSet<Cell>;

#[derive(PartialEq, Eq, Debug)]
pub struct Grid {
    grid: CellSet,
    bounds: Option<(usize, usize)>,
}

impl Grid {
    fn new(b: Option<(usize, usize)>) -> Self {
        match b {
            None => Grid {
                grid: CellSet::new(),
                bounds: b,
            },
            Some((w, h)) => Grid {
                grid: CellSet::with_capacity(w * h),
                bounds: b,
            },
        }
    }
    fn insert(&mut self, cell: &Cell) {
        match self.bounds {
            None => {
                self.grid.insert(*cell);
            }
            Some((w, h)) => {
                if cell.0 <= w && cell.1 <= h {
                    self.grid.insert(*cell);
                }
            }
        }
    }
    fn contains(&self, cell: &Cell) -> bool {
        self.grid.contains(cell)
    }

    fn x_bound(&self) -> Option<usize> {
        match self.bounds {
            None => None,
            Some((w, _)) => Some(w),
        }
    }

    fn y_bound(&self) -> Option<usize> {
        match self.bounds {
            None => None,
            Some((_, h)) => Some(h),
        }
    }

    pub fn gen(&mut self) {
        match self.bounds {
            None => {}
            Some(_) => {
                self.grid.clear();
                for x in 0..self.x_bound().unwrap() {
                    for y in 0..self.y_bound().unwrap() {
                        if rand::thread_rng().gen_bool(1.0/10.0) {
                            self.insert(&(x, y));
                        }
                    }
                }
            }
        }
    }
}

impl<'a> From<Vec<&'a str>> for Grid {
    /// Returns a Grid interpreted from a string representation
    ///
    /// # Arguments
    ///
    /// * `s` - Representation of the grid. Each element of the vector
    /// represents a row in the grid. Hash marks # indicate live cells.
    /// For example ovec!["#  ", "   ", " # "] represents a grid with live
    /// cells at (0, 0) and (2, 1).
    ///
    /// # Example
    ///
    /// ```
    /// let grid = hemoglobin::Grid::from(vec!["#  ", "   ", " # "]);
    /// ```
    fn from(s: Vec<&str>) -> Self {
        let mut result = Grid::new(None);
        for (y, row) in s.iter().enumerate() {
            for (x, c) in row.chars().enumerate() {
                if c == '#' {
                    result.insert(&(x, y));
                }
            }
        }
        result
    }
}

#[derive(Clone)]
struct B64 {
    data: [u8; 8],
    conf: base64::Config,
}

impl Default for B64 {
    fn default() -> Self {
        B64 {
            data: [0u8;8],
            conf: base64::Config::new(
            base64::CharacterSet::Standard,
            true,
            true,
            base64::LineWrap::NoWrap,
        )
        }
    }
}

impl From<String> for B64 {
    fn from(s: String) -> Self {
        let mut enc = B64::default();
        base64::decode_config_slice(&s, enc.conf, &mut enc.data).unwrap();
        enc
    }
}

impl fmt::Display for B64 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", base64::encode_config(&self.data, self.conf))
    }
}

#[allow(dead_code)]
pub struct Rule {
    b64: B64,
    bin: BitVec,
}

impl From<B64> for Rule {
    fn from(x: B64) -> Self {
        Rule {
            b64: x.clone(),
            bin: {
                let reversed = BitVec::from_bytes(&x.data);
                let mut result = BitVec::from_elem(512, false);
                for i in 0..reversed.len() {
                    result.set(i, reversed[reversed.len() - i - 1]);
                }
                result
            },
        }
    }
}

impl From<String> for Rule {
    fn from(s: String) -> Self {
        Rule::from(B64::from(s))
    }
}

pub struct World {
    rule: Rule,
    grid: Grid,
    swap_grid: Grid,
}

impl World {
    pub fn new(width: usize, height: usize, rule: Rule) -> Self {
        World {
            rule: rule,
            grid: Grid::new(Some((width, height))),
            swap_grid: Grid::new(Some((width, height))),
        }
    }

    fn decide_next_state(&self, cell: &Cell) -> bool {
        let state = get_state(&self.grid, cell);
        self.rule.bin[state]
    }

    pub fn step(&mut self) {
        self.swap_grid.grid.clear();

        for x in 0..self.grid.x_bound().unwrap() {
            for y in 0..self.grid.y_bound().unwrap() {
                let cell = (x, y);
                if self.decide_next_state(&cell) {
                    self.swap_grid.insert(&cell);
                }
            }
        }
        std::mem::swap(&mut self.grid, &mut self.swap_grid);
    }

    pub fn gen(&mut self) {
        self.grid.gen()
    }

    pub fn render(&self, canvas: &mut Widget) {
        for x in 0..self.grid.x_bound().unwrap() {
            for y in 0..self.grid.y_bound().unwrap() {
                let mut cell = canvas.get_mut(x, y).unwrap();
                if self.grid.contains(&(x, y)) {
                    cell.set_ch('\u{2588}');
                } else {
                    cell.set_ch(' ');
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate byteorder;

    use super::*;
    use self::byteorder::{ByteOrder, LittleEndian};

    /*const EXPECTED_1082_BITS: [bool; 16] = [
        false, true, false, true, false, false, false, false, true, true, true, false, false,
        false, false, false,
    ];*/
    // 1802 = 10 + 7*(2^8), so writing in little endian byte order but writing
    // the bits within each byte with MSB on the left, we have
    // [00001010][00000111].
    // As a BitVec, we want 01010000 11100000.

    #[test]
    fn test_bitvec_order() {
        // Consider a two-byte number where the first byte's value is 10 and
        // the second byte's value is 7. Converting to a little endian byte
        // array should make the 0th byte 10 and the 1th byte 7.
        let num = 10 + 7 * (2_usize.pow(8)) as u64;
        let mut bytes = [0u8; 2];
        LittleEndian::write_uint(&mut bytes, num, 2);
        assert_eq!(bytes[0], 10);
        assert_eq!(bytes[1], 7);

        // Now check what happens when we convert this to a BitVec. The bytes
        // are in little endian order, but the bits within each byte are big
        // endian:
        // [00001010][00000111]
        let bits = BitVec::from_bytes(&bytes);
        let expected = vec![
            // 0th byte representing 10
            false,
            false,
            false,
            false,
            true,
            false,
            true,
            false,
            // 1st byte representing 7
            false,
            false,
            false,
            false,
            false,
            true,
            true,
            true,
        ];
        for i in 0..16 {
            assert_eq!(bits[i], expected[i]);
        }
    }

    #[test]
    fn test_gen_conway() {
        fn gen_conway() -> B64 {
            let mut conway = B64::default();
            for state in 0..512 {
                let mut bit_count = 0_usize;
                let current_state = (state >> 4) % 2;
                for bit_offset in [0, 1, 2, 3, 5, 6, 7, 8].iter() {
                    bit_count
                }
            }
        }
    }

    /*

    fn gen_conway_dec() -> BigUint {
        let mut kode = BigUint::from(0u32);
        for state in 0..512 {
            let mut bit_count = 0usize;
            let current_state = (state >> 4) % 2;
            for bit_offset in [0, 1, 2, 3, 5, 6, 7, 8].iter() {
                bit_count += (state >> bit_offset) & 1usize;
            }
            let result = BigUint::from(match bit_count {
                2 => current_state,
                3 => 1,
                _ => 0,
            });
            kode = kode + (result << state);
        }
        kode
    }

    #[test]
    fn test_gen_conway_dec() {
        let expected = "476348294852520375132009738840824718882889556\
                        423255282629108876378472743729817205343700177\
                        683429960362194923168607044012736510546282236\
                        08960"
            .parse::<BigUint>()
            .unwrap();
        assert_eq!(expected, gen_conway_dec());
    }

    #[test]
    fn test_grid_from_string() {
        let grid = Grid::from(vec!["   ", "   "]);
        let mut expected = Grid::new(None);
        assert_eq!(grid, expected);

        let grid = Grid::from(vec!["#  ", "   "]);
        expected.insert(&(0, 0));
        assert_eq!(grid, expected);

        let grid = Grid::from(vec!["#  ", " # "]);
        expected.insert(&(1, 1));
        assert_eq!(grid, expected);
    }

    #[test]
    fn test_rule_dec_str_to_rule() {
        let rule = Rule::from("1802".to_string());
        for i in 0..16 {
            assert_eq!(rule.bin[i], EXPECTED_1082_BITS[i]);
        }
    }

    #[test]
    fn test_rule_from_bigint() {
        let rule = Rule::from(BigUint::from(1802u32));
        for i in 0..16 {
            assert_eq!(rule.bin[i], EXPECTED_1082_BITS[i]);
        }
    }

    #[test]
    fn test_get_state() {
        let mut grid = Grid::new(None);
        //  0
        // 0#< look here
        //  ^
        grid.insert(&(0, 0));
        assert_eq!(get_state(&grid, &(0, 0)), 16); // 2^4
        //  01
        // 0#-< look here
        // 1-#
        //  ^
        grid.insert(&(1, 1));
        assert_eq!(get_state(&grid, &(0, 0)), 272); // 2^4 + 2^8
    }
 */
}

fn get_state(grid: &Grid, cell: &Cell) -> usize {
    let (x, y) = (cell.0, cell.1);
    let mut val = 0;
    // We now build up an integer representation of the state centered at cell.
    // We iterate over neighboring cells: dx and dy go over [0, 1, 2] where
    //   0 means "minus one", so "left" for x or "up" for y
    //   1 means same row (for x) or column (for y).
    //   2 means "plus one", so "right" for x or "down" for y.
    // Therefore, for a given dx and dy, the coordinates of the neighbor are
    // (x+dx-1, y+dy-1). However, if we're at an edge, these coordinates might
    // take us off the grid. This shows up as a failure to do the subtraction
    // because getting a negative number means we're off the grid. We check
    // for this failure with checked_sub and return false if the call returns
    // None.
    //
    // TODO: replace "integer representation" with canonical name once we pick
    // one.
    for dx in 0..3 {
        for dy in 0..3 {
            if match (x + dx).checked_sub(1) {
                None => false,
                Some(xx) => {
                    match (y + dy).checked_sub(1) {
                        None => false,
                        Some(yy) => grid.contains(&(xx, yy)),
                    }
                }
            }
            {
                val += 1 << (dx + (3 * dy));
            }
        }
    }
    val
}
