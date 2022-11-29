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
     * Allocates 4kb page
     * return None if allocation fails
     */
    pub fn allocate_frame(&mut self) -> Option<usize> {
        // First level search
        let mut block_chosen_l1 = 8;
        for i in 0..8 {
            if self.tree_4kb[i] != 0 {
                block_chosen_l1 = i;
                break;
            }
        }
        if block_chosen_l1 == 8 {
            return None;
        }
        let l1_idx = Self::bsf(self.tree_4kb[block_chosen_l1]) + block_chosen_l1 * 64;
        if l1_idx >= NB_GB {
            return None;
        }

        // Second level search
        let first_block_l2 = TREE_1GB_SIZE + 8 * l1_idx;
        let mut block_chosen_l2 = 8;
        for i in 0..8 {
            if self.tree_4kb[first_block_l2 + i] != 0 {
                block_chosen_l2 = i;
                break;
            }
        }
        assert!(block_chosen_l2 < 8);
        assert!(self.tree_4kb[first_block_l2 + block_chosen_l2] != 0u64);
        let l2_idx =
            Self::bsf(self.tree_4kb[first_block_l2 + block_chosen_l2]) + 64 * block_chosen_l2;

        // Third level search
        let first_block_l3 = TREE_1GB_SIZE + 8 * NB_GB + 512 * 8 * l1_idx + 8 * l2_idx;
        let mut block_chosen_l3 = 8;
        for j in 0..8 {
            if self.tree_4kb[first_block_l3 + j] != 0u64 {
                block_chosen_l3 = j;
                break;
            }
        }
        assert!(block_chosen_l3 < 8);
        assert!(self.tree_4kb[first_block_l3 + block_chosen_l3] != 0);
        let l3_idx =
            Self::bsf(self.tree_4kb[first_block_l3 + block_chosen_l3]) + 64 * block_chosen_l3;

        // Set bits to 0
        self.tree_4kb[first_block_l3 + block_chosen_l3] &= !(1u64 << (l3_idx % 64));
        // if block is full set upper level to 0
        self.tree_4kb[first_block_l2 + block_chosen_l2] &= !(1u64 << (l2_idx % 64));
        for i in 0..8 {
            if self.tree_4kb[first_block_l3 + i] != 0u64 {
                self.tree_4kb[first_block_l2 + block_chosen_l2] |= 1u64 << (l2_idx % 64);
                break;
            }
        }

        self.tree_4kb[block_chosen_l1] &= !(1u64 << (l1_idx % 64));
        for i in 0..8 {
            if self.tree_4kb[first_block_l2 + i] != 0u64 {
                self.tree_4kb[block_chosen_l1] |= 1u64 << (l1_idx % 64);
                break;
            }
        }

        // set bits from TREE_1GB and TREE_2MB to 0
        self.tree_2mb[first_block_l2 + block_chosen_l2] &= !(1u64 << (l2_idx % 64));
        let mut need_to_set_mb_level1 = true;
        for i in 0..8 {
            if self.tree_2mb[first_block_l2 + i] != 0 {
                need_to_set_mb_level1 = false;
                break;
            }
        }
        if need_to_set_mb_level1 {
            self.tree_2mb[block_chosen_l1] &= !(1u64 << (l1_idx % 64));
        }
        self.tree_1gb[block_chosen_l1] &= !(1u64 << (l1_idx % 64));

        let final_idx = 512 * 512 * l1_idx + 512 * l2_idx + l3_idx;
        Some(final_idx)
    }

    /**
     * Allocates 2Mb page
     * return None if allocation fails
     */
    pub fn allocate_big_page(&mut self) -> Option<usize> {
        // First level search
        let mut block_chosen_l1 = 8;
        for i in 0..8 {
            if self.tree_2mb[i] != 0 {
                block_chosen_l1 = i;
                break;
            }
        }
        if block_chosen_l1 == 8 {
            return None;
        }
        let l1_idx = Self::bsf(self.tree_2mb[block_chosen_l1]) + 64 * block_chosen_l1;
        if l1_idx >= NB_GB {
            return None;
        }

        // Second level search
        let first_block_l2 = TREE_1GB_SIZE + 8 * l1_idx;
        let mut block_chosen_l2 = 8;
        for i in 0..8 {
            if self.tree_2mb[first_block_l2 + i] != 0 {
                block_chosen_l2 = i;
                break;
            }
        }
        assert!(block_chosen_l2 < 8);
        assert!(self.tree_2mb[first_block_l2 + block_chosen_l2] != 0u64);
        let l2_idx =
            Self::bsf(self.tree_2mb[first_block_l2 + block_chosen_l2]) + 64 * block_chosen_l2;

        // Set bits to 0
        self.tree_2mb[first_block_l2 + block_chosen_l2] &= !(1u64 << (l2_idx % 64));
        // if block is full set upper level to 0
        self.tree_2mb[block_chosen_l1] &= !(1u64 << (l1_idx % 64));
        for i in 0..8 {
            if self.tree_2mb[first_block_l2 + i] != 0u64 {
                self.tree_2mb[block_chosen_l1] |= 1u64 << (l1_idx % 64);
                break;
            }
        }

        // set bits from TREE_1GB and TREE_4KB to 0
        self.tree_4kb[first_block_l2 + block_chosen_l2] &= !(1u64 << (l2_idx % 64));
        self.tree_4kb[block_chosen_l1] &= !(1u64 << (l1_idx % 64));
        for i in 0..8 {
            if self.tree_4kb[first_block_l2 + i] != 0u64 {
                self.tree_4kb[block_chosen_l1] |= 1u64 << (l1_idx % 64);
                break;
            }
        }

        self.tree_1gb[block_chosen_l1] &= !(1u64 << (l1_idx % 64));

        let final_idx = 512 * 512 * l1_idx + 512 * l2_idx;
        Some(final_idx)
    }

    /**
     * Allocates 1Gb page
     * return None if allocation fails
     */
    pub fn allocate_huge_page(&mut self) -> Option<usize> {
        // First level search
        let mut block_chosen_l1 = 8;
        for i in 0..8 {
            if self.tree_1gb[i] != 0 {
                block_chosen_l1 = i;
                break;
            }
        }
        if block_chosen_l1 == 8 {
            return None;
        }
        let l1_idx = Self::bsf(self.tree_1gb[block_chosen_l1]) + block_chosen_l1 * 64;
        if l1_idx >= NB_GB {
            return None;
        }

        self.tree_1gb[block_chosen_l1] &= !(1u64 << (l1_idx % 64));
        self.tree_2mb[block_chosen_l1] &= !(1u64 << (l1_idx % 64));
        self.tree_4kb[block_chosen_l1] &= !(1u64 << (l1_idx % 64));

        let final_idx = 512 * 512 * l1_idx;
        return Some(final_idx);
    }

    /**
     * Deallocates frame
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

        let l1_tree_idx = l1_block_idx / 64;
        self.tree_4kb[l1_tree_idx] |= 1u64 << (l1_block_idx % 64);
        let l2_tree_idx = TREE_1GB_SIZE + 8 * l1_block_idx + l2_block_idx / 64;
        self.tree_4kb[l2_tree_idx] |= 1u64 << (l2_block_idx % 64);
        let l3_tree_idx = TREE_1GB_SIZE
            + 8 * NB_GB
            + 512 * 8 * l1_block_idx
            + 8 * l2_block_idx
            + l3_block_idx / 64;
        self.tree_4kb[l3_tree_idx] |= 1u64 << (l3_block_idx % 64);

        let mut need_to_free_2mb_level2 = true;
        let first_block_l3 = l3_tree_idx - l3_block_idx / 64;
        for i in 0..8 {
            if self.tree_4kb[first_block_l3 + i] != !0u64 {
                need_to_free_2mb_level2 = false;
                break;
            }
        }
        if need_to_free_2mb_level2 {
            self.tree_2mb[l2_tree_idx] |= 1u64 << (l2_block_idx % 64);
            self.tree_2mb[l1_tree_idx] |= 1u64 << (l1_block_idx % 64);
        }

        let mut need_to_free_1gb_level1 = true;
        let first_block_l2 = l2_tree_idx - l2_block_idx / 64;
        for i in 0..8 {
            if self.tree_2mb[first_block_l2 + i] != !0u64 {
                need_to_free_1gb_level1 = false;
                break;
            }
        }
        if need_to_free_1gb_level1 {
            self.tree_1gb[l1_tree_idx] |= 1u64 << (l1_block_idx % 64);
        }
    }

    /**
     * Deallocates big page
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
        self.tree_2mb[l1_tree_idx] |= 1u64 << (l1_block_idx % 64);
        let l2_tree_idx = TREE_1GB_SIZE + 8 * l1_block_idx + l2_block_idx / 64;
        self.tree_2mb[l2_tree_idx] |= 1u64 << (l2_block_idx % 64);

        self.tree_4kb[l2_tree_idx] |= 1u64 << (l2_block_idx % 64);
        self.tree_4kb[l1_tree_idx] |= 1u64 << (l1_block_idx % 64);

        let mut need_to_free_1gb_level1 = true;
        let first_block_l2 = l2_tree_idx - l2_block_idx / 64;
        for i in 0..8 {
            if self.tree_2mb[first_block_l2 + i] != 0u64 {
                need_to_free_1gb_level1 = false;
                break;
            }
        }
        if need_to_free_1gb_level1 {
            self.tree_1gb[l1_tree_idx] |= 1u64 << (l1_block_idx % 64);
        }

        let mut need_to_free_1gb_level1 = true;
        let first_block_l2 = l2_tree_idx - l2_block_idx / 64;
        for i in 0..8 {
            if self.tree_2mb[first_block_l2 + i] != !0u64 {
                need_to_free_1gb_level1 = false;
                break;
            }
        }
        if need_to_free_1gb_level1 {
            self.tree_1gb[l1_tree_idx] |= 1u64 << (l1_block_idx % 64);
        }
    }

    /**
     * Deallocates huge page
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
     * Checks integrity of allocated pages
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

        let l1_tree_idx = l1_block_idx / 64;
        let l2_tree_idx = TREE_1GB_SIZE + 8 * l1_block_idx + l2_block_idx / 64;
        let l3_tree_idx = TREE_1GB_SIZE
            + 8 * NB_GB
            + 512 * 8 * l1_block_idx
            + 8 * l2_block_idx
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
            if i % 1000 == 0 {
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
            if i % 10000 == 0 {
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
            if i % 1000 == 0 {
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
            if i % 1000 == 0 {
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
            if i % 100 == 0 {
                frame_alloc.check_integrity();
            }
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
