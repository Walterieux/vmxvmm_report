//! Revisited buddy allocator

use std::arch::asm;

const NB_GB: usize = 512;
const NB_PAGES: usize = 512 * 512 * NB_GB;
const TREE_1GB_SIZE: usize = 8;
const TREE_2MB_SIZE: usize = TREE_1GB_SIZE + NB_GB * 512 / 64;
const TREE_4KB_SIZE: usize = TREE_2MB_SIZE + NB_GB * 512 * 512 / 64;

#[derive(Copy, Clone, PartialEq)]
pub enum TreeType {
    Tree4kb,
    Tree2mb,
    Tree1gb,
}

#[derive(Copy, Clone, PartialEq)]
pub enum Level {
    Level1,
    Level2,
    Level3,
}

// TODO generic const <const N>
pub struct BuddyAllocator {
    tree_4kb: Box<[u64; TREE_4KB_SIZE]>,
    tree_2mb: Box<[u64; TREE_2MB_SIZE]>,
    tree_1gb: Box<[u64; TREE_1GB_SIZE]>,
}

impl BuddyAllocator {
    pub fn new() -> Self {
        assert!(NB_GB <= 512);
        Self {
            tree_4kb: Box::new([0xFFFFFFFFFFFFFFFF; TREE_4KB_SIZE]),
            tree_2mb: Box::new([0xFFFFFFFFFFFFFFFF; TREE_2MB_SIZE]),
            tree_1gb: Box::new([0xFFFFFFFFFFFFFFFF; TREE_1GB_SIZE]),
        }
    }

    /**
     * Bit Scan Forward
     */
    fn bsf(input: u64) -> usize {
        assert!(input > 0);
        let mut pos: usize;
        unsafe {
            asm! {
                "bsf {pos}, {input}",
                input = in(reg) input,
                pos = out(reg) pos,
                options(nomem, nostack),
            };
        };
        assert!(pos < 64);
        pos
    }

    /**
     * Bit Scan Reverse
     */
    #[allow(dead_code)]
    fn bsr(input: u64) -> usize {
        assert!(input > 0);
        let mut pos: usize;
        unsafe {
            asm! {
                "bsr {pos}, {input}",
                input = in(reg) input,
                pos = out(reg) pos,
                options(nomem, nostack),
            };
        };
        assert!(pos < 64);
        pos
    }

    /**
     * Allocate 4kb page
     * return None if allocation fails
     */
    pub fn allocate_frame(&mut self) -> Option<usize> {
        // First level search
        let l1_idx_found = self.search_first_bit_set(TreeType::Tree4kb, 0);
        if l1_idx_found.is_none() {
            return None;
        }
        let l1_idx = l1_idx_found.unwrap();
        if l1_idx >= NB_GB {
            return None;
        }

        // Second level search
        let first_block_l2 = Self::compute_first_block_index(l1_idx, 0, Level::Level2);
        let l2_idx_found = self.search_first_bit_set(TreeType::Tree4kb, first_block_l2);
        assert!(l2_idx_found.is_some());
        let l2_idx = l2_idx_found.unwrap();

        // Third level search
        let first_block_l3 = Self::compute_first_block_index(l1_idx, l2_idx, Level::Level3);
        let l3_idx_found = self.search_first_bit_set(TreeType::Tree4kb, first_block_l3);
        assert!(l3_idx_found.is_some());
        let l3_idx = l3_idx_found.unwrap();

        // 4Kb tree: set bits to 0
        self.tree_4kb[first_block_l3 + l3_idx / 64] &= !(1u64 << (l3_idx % 64));
        // if block is full set upper level to 0
        if self
            .search_first_bit_set(TreeType::Tree4kb, first_block_l3)
            .is_none()
        {
            self.tree_4kb[first_block_l2 + l2_idx / 64] &= !(1u64 << (l2_idx % 64));
        }
        if self
            .search_first_bit_set(TreeType::Tree4kb, first_block_l2)
            .is_none()
        {
            self.tree_4kb[l1_idx / 64] &= !(1u64 << (l1_idx % 64));
        }

        // 2Mb tree: set bits to 0
        self.tree_2mb[first_block_l2 + l2_idx / 64] &= !(1u64 << (l2_idx % 64));
        if self
            .search_first_bit_set(TreeType::Tree2mb, first_block_l2)
            .is_none()
        {
            self.tree_2mb[l1_idx / 64] &= !(1u64 << (l1_idx % 64))
        }

        // 1Gb tree: set bit to 0
        self.tree_1gb[l1_idx / 64] &= !(1u64 << (l1_idx % 64));

        // Memory index is built as follow: |l1 l1 l1 l1 l1 l1 l1 l1 l1|l2 l2 l2 l2 l2 l2 l2 l2 l2|l3 l3 l3 l3 l3 l3 l3 l3 l3|
        Some((l1_idx << 18) + (l2_idx << 9) + l3_idx)
    }

    /**
     * Allocate 2Mb page
     * return None if allocation fails
     */
    pub fn allocate_big_page(&mut self) -> Option<usize> {
        // First level search
        let l1_idx_found = self.search_first_bit_set(TreeType::Tree2mb, 0);
        if l1_idx_found.is_none() {
            return None;
        }
        let l1_idx = l1_idx_found.unwrap();
        if l1_idx >= NB_GB {
            return None;
        }

        // Second level search
        let first_block_l2 = Self::compute_first_block_index(l1_idx, 0, Level::Level2);
        let l2_idx_found = self.search_first_bit_set(TreeType::Tree2mb, first_block_l2);
        assert!(l2_idx_found.is_some());
        let l2_idx = l2_idx_found.unwrap();

        // Set bits to 0
        self.tree_2mb[first_block_l2 + l2_idx / 64] &= !(1u64 << (l2_idx % 64));
        // if block is full set upper level to 0
        if self
            .search_first_bit_set(TreeType::Tree2mb, first_block_l2)
            .is_none()
        {
            self.tree_2mb[l1_idx / 64] &= !(1u64 << (l1_idx % 64));
        }

        // set bits from TREE_1GB and TREE_4KB to 0
        self.tree_4kb[first_block_l2 + l2_idx / 64] &= !(1u64 << (l2_idx % 64));
        if self
            .search_first_bit_set(TreeType::Tree4kb, first_block_l2)
            .is_none()
        {
            self.tree_4kb[l1_idx / 64] &= !(1u64 << (l1_idx % 64));
        }

        self.tree_1gb[l1_idx / 64] &= !(1u64 << (l1_idx % 64));

        // Memory index is built as follow: |l1 l1 l1 l1 l1 l1 l1 l1 l1|l2 l2 l2 l2 l2 l2 l2 l2 l2|0 0 0 0 0 0 0 0 0|
        Some((l1_idx << 18) + (l2_idx << 9))
    }

    /**
     * Allocate 1Gb page
     * return None if allocation fails
     */
    pub fn allocate_huge_page(&mut self) -> Option<usize> {
        // First level search
        let l1_idx_found = self.search_first_bit_set(TreeType::Tree1gb, 0);
        if l1_idx_found.is_none() {
            return None;
        }
        let l1_idx = l1_idx_found.unwrap();
        if l1_idx >= NB_GB {
            return None;
        }

        // set bits from TREE_1GB, TREE_2MB and TREE_4KB to 0
        self.tree_1gb[l1_idx / 64] &= !(1u64 << (l1_idx % 64));
        self.tree_2mb[l1_idx / 64] &= !(1u64 << (l1_idx % 64));
        self.tree_4kb[l1_idx / 64] &= !(1u64 << (l1_idx % 64));

        // Memory index is built as follow: |l1 l1 l1 l1 l1 l1 l1 l1 l1|0 0 0 0 0 0 0 0 0|0 0 0 0 0 0 0 0 0|
        Some(l1_idx << 18)
    }

    /**
     * Deallocate frame
     * nothing is done if frame was not previously allocated
     */
    pub fn deallocate_frame(&mut self, frame_id: usize) {
        let mut id = frame_id;
        // return if frame was not allocated
        if self.get_bit_level_index(TreeType::Tree4kb, Level::Level3, id) {
            return;
        }

        let l3_block_idx = id & 0x1FF;
        id >>= 9;
        let l2_block_idx = id & 0x1FF;
        id >>= 9;
        let l1_block_idx = id & 0x1FF;

        // Set the 3 levels to free
        let l1_tree_idx = l1_block_idx / 64;
        let l2_tree_idx =
            Self::compute_first_block_index(l1_block_idx, 0, Level::Level2) + l2_block_idx / 64;
        let l3_tree_idx =
            Self::compute_first_block_index(l1_block_idx, l2_block_idx, Level::Level3)
                + l3_block_idx / 64;

        self.tree_4kb[l1_tree_idx] |= 1u64 << (l1_block_idx % 64);
        self.tree_4kb[l2_tree_idx] |= 1u64 << (l2_block_idx % 64);
        self.tree_4kb[l3_tree_idx] |= 1u64 << (l3_block_idx % 64);

        // if all 4Kb are free, free upper level for 2Mb
        let first_block_l3 = l3_tree_idx - l3_block_idx / 64;
        if self.all_free(TreeType::Tree4kb, first_block_l3) {
            self.tree_2mb[l2_tree_idx] |= 1u64 << (l2_block_idx % 64);
            self.tree_2mb[l1_tree_idx] |= 1u64 << (l1_block_idx % 64);
        }

        // if all 2Mb are free, free 1Gb block
        let first_block_l2 = l2_tree_idx - l2_block_idx / 64;
        if self.all_free(TreeType::Tree2mb, first_block_l2) {
            self.tree_1gb[l1_tree_idx] |= 1u64 << (l1_block_idx % 64);
        }
    }

    /**
     * Deallocate big page
     * nothing is done if page was not previously allocated
     */
    pub fn deallocate_big_page(&mut self, frame_id: usize) {
        let mut id = frame_id;
        // return if big page was not allocated
        if id % 512 != 0
            || self.get_bit_level_index(TreeType::Tree2mb, Level::Level2, id)
            || self.get_bit_level_index(TreeType::Tree4kb, Level::Level2, id)
        {
            return;
        }

        let l3_block_idx = id & 0x1FF;
        id >>= 9;
        let l2_block_idx = id & 0x1FF;
        id >>= 9;
        let l1_block_idx = id & 0x1FF;
        assert!(l3_block_idx == 0); // l3_block_idx must be 0 for 2mb pages

        let l1_tree_idx = l1_block_idx / 64;
        let l2_tree_idx =
            Self::compute_first_block_index(l1_block_idx, 0, Level::Level2) + l2_block_idx / 64;

        self.tree_2mb[l1_tree_idx] |= 1u64 << (l1_block_idx % 64);
        self.tree_2mb[l2_tree_idx] |= 1u64 << (l2_block_idx % 64);

        self.tree_4kb[l2_tree_idx] |= 1u64 << (l2_block_idx % 64);
        self.tree_4kb[l1_tree_idx] |= 1u64 << (l1_block_idx % 64);

        let first_block_l2 = l2_tree_idx - l2_block_idx / 64;
        if self.all_free(TreeType::Tree2mb, first_block_l2) {
            self.tree_1gb[l1_tree_idx] |= 1u64 << (l1_block_idx % 64);
        }
    }

    /**
     * Deallocate huge page
     * nothing is done if frame was not previously allocated
     */
    pub fn deallocate_huge_page(&mut self, frame_id: usize) {
        let mut id = frame_id;
        // return if huge page was not allocated
        if id % (512 * 512) != 0
            || self.get_bit_level_index(TreeType::Tree1gb, Level::Level1, id)
            || self.get_bit_level_index(TreeType::Tree2mb, Level::Level1, id)
            || self.get_bit_level_index(TreeType::Tree4kb, Level::Level1, id)
        {
            return;
        }

        let l3_block_idx = id & 0x1FF;
        id >>= 9;
        let l2_block_idx = id & 0x1FF;
        id >>= 9;
        let l1_block_idx = id & 0x1FF;
        assert!(l3_block_idx == 0); // l3_block_idx must be 0 for 1gb pages
        assert!(l2_block_idx == 0); // l2_block_idx must be 0 for 1gb pages

        let l1_tree_idx = l1_block_idx / 64;
        self.tree_1gb[l1_tree_idx] |= 1u64 << (l1_block_idx % 64);
        self.tree_2mb[l1_tree_idx] |= 1u64 << (l1_block_idx % 64);
        self.tree_4kb[l1_tree_idx] |= 1u64 << (l1_block_idx % 64);
    }

    /**
     * Check integrity of allocated pages
     * crash if integrity is not ensured
     */
    pub fn check_integrity(&self) {
        for i in 0..NB_PAGES {
            // 4Kb tree level 3 not free
            assert!(
                !(!self.get_bit_level_index(TreeType::Tree4kb, Level::Level3, i)
                    && (self.get_bit_level_index(TreeType::Tree2mb, Level::Level2, i)
                        || self.get_bit_level_index(TreeType::Tree1gb, Level::Level1, i)))
            );

            // 4Kb tree level 3 free and 2Mb level 2 not free
            assert!(
                !(self.get_bit_level_index(TreeType::Tree4kb, Level::Level3, i)
                    && !self.get_bit_level_index(TreeType::Tree2mb, Level::Level2, i)
                    && self.get_bit_level_index(TreeType::Tree1gb, Level::Level1, i))
            );
        }
    }

    /**
     * Return the number of free block in the following order (1gb, 2mb, 4kb)
     */
    pub fn stat_free_memory(&self) -> (u64, u64, u64) {
        let mut nb_4kb = 0u64;
        let mut nb_2mb = 0u64;
        let mut nb_1gb = 0u64;

        let mut num_free = 0;
        let mut i = 0;
        while i < NB_PAGES {
            // 4Kb tree level 3 not free
            if !self.get_bit_level_index(TreeType::Tree4kb, Level::Level3, i) {
                i += 1;
                nb_1gb += num_free / (512 * 512);
                nb_2mb += (num_free % (512 * 512)) / 512;
                nb_4kb += num_free % 512;
                num_free = 0;
                continue;
            }

            // Not aligned 2Mb
            if i % 512 != 0 {
                if (i as u64) % 512 != num_free % 512 {
                    assert!(num_free == 0);
                    nb_4kb += 1;
                } else {
                    num_free += 1;
                }
                i += 1;
                continue;
            }

            // 2Mb and 4Kb trees level 2 not free
            if !self.get_bit_level_index(TreeType::Tree2mb, Level::Level2, i)
                && !self.get_bit_level_index(TreeType::Tree4kb, Level::Level2, i)
            {
                i += 512;
                nb_1gb += num_free / (512 * 512);
                nb_2mb += (num_free % (512 * 512)) / 512;
                nb_4kb += num_free % 512;
                num_free = 0;
                continue;
            }

            // Not aligned 1Gb
            if i % (512 * 512) != 0 {
                if (i as u64) % 512 != num_free % 512 {
                    assert!(num_free == 0);
                    nb_4kb += 1;
                } else {
                    num_free += 1;
                }
                i += 1;
                continue;
            }

            // 1Gb, 2Mb and 4Kb trees level 1 not free
            if !self.get_bit_level_index(TreeType::Tree1gb, Level::Level1, i)
                && !self.get_bit_level_index(TreeType::Tree2mb, Level::Level1, i)
                && !self.get_bit_level_index(TreeType::Tree4kb, Level::Level1, i)
            {
                i += 512 * 512;
                nb_1gb += num_free / (512 * 512);
                nb_2mb += (num_free % (512 * 512)) / 512;
                nb_4kb += num_free % 512;
                num_free = 0;
                continue;
            }

            if (i as u64) % 512 != num_free % 512 {
                assert!(num_free == 0);
                nb_4kb += 1;
            } else {
                num_free += 1;
            }
            i += 1;
        }

        nb_1gb += num_free / (512 * 512);
        nb_2mb += (num_free % (512 * 512)) / 512;
        nb_4kb += num_free % 512;

        (nb_1gb, nb_2mb, nb_4kb)
    }

    /**
     * Check if a bit is set of a given tree at a given level
     * return true if bit equals 1, raise an error if given level does not exist
     */
    fn get_bit_level_index(&self, tree_type: TreeType, level: Level, index: usize) -> bool {
        assert!(index < NB_PAGES);

        let mut id = index;

        let l3_block_idx = id & 0x1FF;
        id >>= 9;
        let l2_block_idx = id & 0x1FF;
        id >>= 9;
        let l1_block_idx = id & 0x1FF;

        self.get_bit_level_block_levels_index(
            tree_type,
            level,
            l1_block_idx,
            l2_block_idx,
            l3_block_idx,
        )
    }

    /**
     * Check if a bit is set of a given tree at a given level (l1_block_idx, l2_block_idx and l3_block_idx are given)
     * return true if bit equals 1, raise an error if given level does not exist
     */
    fn get_bit_level_block_levels_index(
        &self,
        tree_type: TreeType,
        level: Level,
        l1_block_idx: usize,
        l2_block_idx: usize,
        l3_block_idx: usize,
    ) -> bool {
        let l1_tree_idx = Self::compute_first_block_index(l1_block_idx, 0, Level::Level1);
        let l2_tree_idx =
            Self::compute_first_block_index(l1_block_idx, 0, Level::Level2) + l2_block_idx / 64;
        let l3_tree_idx =
            Self::compute_first_block_index(l1_block_idx, l2_block_idx, Level::Level3)
                + l3_block_idx / 64;

        match tree_type {
            TreeType::Tree4kb => match level {
                Level::Level1 => {
                    return (self.tree_4kb[l1_tree_idx] & 1 << (l1_block_idx % 64)) != 0
                }
                Level::Level2 => {
                    return (self.tree_4kb[l2_tree_idx] & 1 << (l2_block_idx % 64)) != 0
                }
                Level::Level3 => {
                    return (self.tree_4kb[l3_tree_idx] & 1 << (l3_block_idx % 64)) != 0
                }
            },
            TreeType::Tree2mb => {
                assert!(level != Level::Level3);
                match level {
                    Level::Level1 => {
                        return (self.tree_2mb[l1_tree_idx] & 1 << (l1_block_idx % 64)) != 0
                    }

                    Level::Level2 => {
                        return (self.tree_2mb[l2_tree_idx] & 1 << (l2_block_idx % 64)) != 0
                    }
                    Level::Level3 => false,
                }
            }
            TreeType::Tree1gb => {
                assert!(level == Level::Level1);
                match level {
                    Level::Level1 => {
                        return (self.tree_1gb[l1_tree_idx] & 1 << (l1_block_idx % 64)) != 0
                    }
                    Level::Level2 => false,
                    Level::Level3 => false,
                }
            }
        }
    }

    /**
     * Search for the first bit set in the next 512 bits at a given start index
     * search from LSB to MSB except for 1Gb tree
     * return Some(idx) if a bit is set otherwise None
     */
    fn search_first_bit_set(&self, tree_type: TreeType, start_idx: usize) -> Option<usize> {
        let mut found_index = None;
        for i in 0..8 {
            // code duplication because of `match` arms have incompatible types
            match tree_type {
                TreeType::Tree4kb => {
                    if self.tree_4kb[start_idx + i] != 0 {
                        found_index = Some(Self::bsf(self.tree_4kb[start_idx + i]) + 64 * i);
                        break;
                    }
                }
                TreeType::Tree2mb => {
                    if self.tree_2mb[start_idx + i] != 0 {
                        found_index = Some(Self::bsf(self.tree_2mb[start_idx + i]) + 64 * i);
                        break;
                    }
                }
                TreeType::Tree1gb => {
                    let rev_i = 7 - i;
                    if self.tree_1gb[start_idx + rev_i] != 0 {
                        found_index =
                            Some(Self::bsr(self.tree_1gb[start_idx + rev_i]) + 64 * rev_i);
                        break;
                    }
                }
            }
        }

        found_index
    }

    /**
     * Return false if at least one block of the 512 one is not free
     */
    fn all_free(&self, tree_type: TreeType, start_idx: usize) -> bool {
        let mut all_free = true;
        for i in 0..8 {
            if match tree_type {
                TreeType::Tree4kb => self.tree_4kb[start_idx + i],
                TreeType::Tree2mb => self.tree_2mb[start_idx + i],
                TreeType::Tree1gb => self.tree_1gb[start_idx + i],
            } != !0u64
            {
                all_free = false;
                break;
            }
        }
        all_free
    }

    /**
     * Compute index of the first block at a given level given his parents indexes
     * for level 1 and level 2, l2_idx is ignored
     */
    fn compute_first_block_index(l1_idx: usize, l2_idx: usize, level: Level) -> usize {
        match level {
            Level::Level1 => l1_idx / 64,
            Level::Level2 => TREE_1GB_SIZE + 8 * l1_idx,
            Level::Level3 => TREE_1GB_SIZE + 8 * NB_GB + 512 * 8 * l1_idx + 8 * l2_idx,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    const NB_PAGES: usize = 512 * 512 * NB_GB;

    #[test]
    fn test_alloc_works() {
        let mut frame_alloc = Box::new(BuddyAllocator::new());
        frame_alloc.check_integrity();
        let new_frame = frame_alloc.allocate_frame();
        assert!(new_frame.is_some());
        frame_alloc.check_integrity();
    }

    #[test]
    fn test_alloc_when_full() {
        let mut frame_alloc = Box::new(BuddyAllocator::new());
        for i in 0..NB_PAGES {
            if i % NB_PAGES / 100 == 0 {
                frame_alloc.check_integrity();
            }
            let new_frame = frame_alloc.allocate_frame();
            assert!(new_frame.is_some());
        }
        frame_alloc.check_integrity();
        let new_frame = frame_alloc.allocate_frame();
        assert!(new_frame.is_none());
        frame_alloc.check_integrity();
    }

    #[test]
    fn test_alloc_and_dealloc_several_times() {
        let mut frame_alloc = Box::new(BuddyAllocator::new());
        for i in 0..NB_PAGES * 10 {
            if i % NB_PAGES == 0 {
                frame_alloc.check_integrity();
            }
            let new_frame = frame_alloc.allocate_frame();
            assert!(new_frame.is_some());
            frame_alloc.deallocate_frame(new_frame.unwrap());
        }
        frame_alloc.check_integrity();
    }

    #[test]
    fn test_two_allocated_frame_are_diff() {
        let mut frame_alloc = Box::new(BuddyAllocator::new());
        let frame1 = frame_alloc.allocate_frame();
        assert!(frame1.is_some());
        frame_alloc.check_integrity();
        let frame2 = frame_alloc.allocate_frame();
        assert!(frame2.is_some());
        frame_alloc.check_integrity();

        assert_ne!(frame1.as_ref().unwrap(), frame2.as_ref().unwrap());
        assert_ne!(frame1.as_ref().unwrap(), frame2.as_ref().unwrap());
    }

    #[test]
    fn test_alloc_and_dealloc_big_several_times() {
        let mut frame_alloc = Box::new(BuddyAllocator::new());
        for i in 0..NB_PAGES {
            if i % NB_PAGES / 100 == 0 {
                frame_alloc.check_integrity();
            }
            let new_frame = frame_alloc.allocate_big_page();
            assert!(new_frame.is_some());
            frame_alloc.deallocate_big_page(new_frame.unwrap());
        }
    }

    #[test]
    fn test_alloc_and_dealloc_huge_several_times() {
        let mut frame_alloc = Box::new(BuddyAllocator::new());
        for i in 0..NB_PAGES {
            if i % NB_PAGES / 100 == 0 {
                frame_alloc.check_integrity();
            }
            let new_frame = frame_alloc.allocate_huge_page();
            assert!(new_frame.is_some());
            frame_alloc.deallocate_huge_page(new_frame.unwrap());
        }
    }

    #[test]
    fn test_allocate_different_types() {
        let mut frame_alloc = Box::new(BuddyAllocator::new());
        let frame = frame_alloc.allocate_frame();
        assert!(frame.is_some());
        frame_alloc.check_integrity();
        let big_page = frame_alloc.allocate_big_page();
        assert!(big_page.is_some());
        frame_alloc.check_integrity();
        let huge_page = frame_alloc.allocate_huge_page();
        assert!(huge_page.is_some());
        frame_alloc.check_integrity();

        assert_ne!(frame.unwrap(), big_page.unwrap());
        assert_ne!(frame.unwrap(), huge_page.unwrap());
        assert_ne!(big_page.unwrap(), huge_page.unwrap());
    }

    #[test]
    fn test_fill_memory_with_frame_and_big() {
        let mut frame_alloc = Box::new(BuddyAllocator::new());

        for i in 0..NB_PAGES / 512 {
            if i % 2 == 0 {
                let big_page = frame_alloc.allocate_big_page();
                assert!(big_page.is_some());
            } else {
                for _ in 0..512 {
                    let frame = frame_alloc.allocate_frame();
                    assert!(frame.is_some());
                }
            }
        }
        frame_alloc.check_integrity();

        // assert memory is full
        let frame = frame_alloc.allocate_frame();
        assert!(frame.is_none());
        let big_page = frame_alloc.allocate_big_page();
        assert!(big_page.is_none());
        let huge_page = frame_alloc.allocate_huge_page();
        assert!(huge_page.is_none());
        frame_alloc.check_integrity();
    }

    #[test]
    fn test_alloc_and_free_with_all_types() {
        let mut frame_alloc = Box::new(BuddyAllocator::new());

        let frame = frame_alloc.allocate_frame();
        assert!(frame.is_some());
        frame_alloc.check_integrity();
        let big_page = frame_alloc.allocate_big_page();
        assert!(big_page.is_some());
        frame_alloc.check_integrity();
        let mut huge_page = frame_alloc.allocate_huge_page();
        assert!(huge_page.is_some());
        frame_alloc.check_integrity();
        for _ in 0..NB_GB - 2 {
            huge_page = frame_alloc.allocate_huge_page();
            assert!(huge_page.is_some());
        }
        frame_alloc.check_integrity();
        let huge_page1 = frame_alloc.allocate_huge_page();
        assert!(huge_page1.is_none());
        frame_alloc.deallocate_huge_page(huge_page.unwrap());
        let huge_page2 = frame_alloc.allocate_huge_page();
        assert!(huge_page2.is_some());
        let big_page1 = frame_alloc.allocate_big_page();
        assert!(big_page1.is_some());
        let frame1 = frame_alloc.allocate_frame();
        assert!(frame1.is_some());
        frame_alloc.check_integrity();
    }

    #[test]
    fn test_dealloc_when_full() {
        let mut frame_alloc = Box::new(BuddyAllocator::new());
        env::set_var("RUST_BACKTRACE", "1");

        for _ in 0..2 {
            // allocates all possible frames
            for _ in 0..NB_PAGES {
                let frame = frame_alloc.allocate_frame();
                assert!(frame.is_some());
            }
            frame_alloc.check_integrity();
            let frame = frame_alloc.allocate_frame();
            assert!(frame.is_none());
            let big_page = frame_alloc.allocate_big_page();
            assert!(big_page.is_none());
            let huge_page = frame_alloc.allocate_huge_page();
            assert!(huge_page.is_none());
            // deallocates all frames
            for i in 0..NB_PAGES {
                frame_alloc.deallocate_frame(i);
            }
            frame_alloc.check_integrity();

            // allocates all possible big pages
            for _ in 0..NB_PAGES / 512 {
                let big_page = frame_alloc.allocate_big_page();
                assert!(big_page.is_some());
            }
            frame_alloc.check_integrity();
            let frame = frame_alloc.allocate_frame();
            assert!(frame.is_none());
            let big_page = frame_alloc.allocate_big_page();
            assert!(big_page.is_none());
            let huge_page = frame_alloc.allocate_huge_page();
            assert!(huge_page.is_none());
            // deallocates all big pages
            for i in 0..NB_PAGES / 512 {
                frame_alloc.deallocate_big_page(i * 512);
            }
            frame_alloc.check_integrity();

            // allocates all possible big pages
            for _ in 0..NB_GB {
                let huge_page = frame_alloc.allocate_huge_page();
                assert!(huge_page.is_some());
            }
            frame_alloc.check_integrity();
            let frame = frame_alloc.allocate_frame();
            assert!(frame.is_none());
            let big_page = frame_alloc.allocate_big_page();
            assert!(big_page.is_none());
            let huge_page = frame_alloc.allocate_huge_page();
            assert!(huge_page.is_none());
            // deallocates all big pages
            for i in 0..NB_GB {
                frame_alloc.deallocate_huge_page(i * 512 * 512);
            }
            frame_alloc.check_integrity();

            // allocates all possible frames
            for _ in 0..NB_PAGES {
                let frame = frame_alloc.allocate_frame();
                assert!(frame.is_some());
            }
            frame_alloc.check_integrity();
            let frame = frame_alloc.allocate_frame();
            assert!(frame.is_none());
            let big_page = frame_alloc.allocate_big_page();
            assert!(big_page.is_none());
            let huge_page = frame_alloc.allocate_huge_page();
            assert!(huge_page.is_none());
            // deallocates all frames
            for i in 0..NB_PAGES {
                frame_alloc.deallocate_frame(i);
            }
            frame_alloc.check_integrity();

            // allocates all possible big pages
            for _ in 0..NB_GB {
                let huge_page = frame_alloc.allocate_huge_page();
                assert!(huge_page.is_some());
            }
            frame_alloc.check_integrity();
            let frame = frame_alloc.allocate_frame();
            assert!(frame.is_none());
            let big_page = frame_alloc.allocate_big_page();
            assert!(big_page.is_none());
            let huge_page = frame_alloc.allocate_huge_page();
            assert!(huge_page.is_none());
            // deallocates all big pages
            for i in 0..NB_GB {
                frame_alloc.deallocate_huge_page(i * 512 * 512);
            }
            frame_alloc.check_integrity();

            // allocates all possible big pages
            for _ in 0..NB_PAGES / 512 {
                let big_page = frame_alloc.allocate_big_page();
                assert!(big_page.is_some());
            }
            frame_alloc.check_integrity();
            let frame = frame_alloc.allocate_frame();
            assert!(frame.is_none());
            let big_page = frame_alloc.allocate_big_page();
            assert!(big_page.is_none());
            let huge_page = frame_alloc.allocate_huge_page();
            assert!(huge_page.is_none());
            // deallocates all big pages
            for i in 0..NB_PAGES / 512 {
                frame_alloc.deallocate_big_page(i * 512);
            }
            frame_alloc.check_integrity();
        }
    }
}
