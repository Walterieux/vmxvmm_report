//! Revisited buddy allocator

use std::arch::asm;
use std::env;

const NB_GB: usize = 8;
const NB_PAGES: usize = 512 * 512 * NB_GB;
const TREE_1GB_SIZE: usize = NB_GB / 64 + if NB_GB % 64 != 0 { 1 } else { 0 }; // just a ceil
const TREE_2MB_SIZE: usize = TREE_1GB_SIZE + NB_GB * 512 / 64;
const TREE_4KB_SIZE: usize = TREE_2MB_SIZE + NB_GB * 512 * 512 / 64;

#[derive(Copy, Clone, PartialEq)]
enum PageType {
    Free,
    Frame,
    Big,
    Huge,
}

// TODO generic const <const N>
pub struct BuddyAllocator {
    tree_4kb: [u64; TREE_4KB_SIZE],
    tree_2mb: [u64; TREE_2MB_SIZE],
    tree_1gb: [u64; TREE_1GB_SIZE],
    allocator_state: [PageType; NB_PAGES],
}

impl BuddyAllocator {
    pub fn new() -> Self {
        Self {
            tree_4kb: [0xFFFFFFFFFFFFFFFF; TREE_4KB_SIZE],
            tree_2mb: [0xFFFFFFFFFFFFFFFF; TREE_2MB_SIZE],
            tree_1gb: [0xFFFFFFFFFFFFFFFF; TREE_1GB_SIZE],
            allocator_state: [PageType::Free; NB_PAGES],
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
     * Allocates 4kb page
     * return None if allocation fails
     */
    pub fn allocate_frame(&mut self) -> Option<usize> {
        // First level search
        if self.tree_4kb[0] == 0 {
            return None;
        }
        let l1_idx = Self::bsf(self.tree_4kb[0]);
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
        if block_chosen_l2 >= 8 {
            println!("error! l1_idx: {}, l2_idx: {}", l1_idx, first_block_l2);
            self.check_integrity();
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

        self.tree_4kb[0] &= !(1u64 << l1_idx);
        for i in 0..8 {
            if self.tree_4kb[first_block_l2 + i] != 0u64 {
                self.tree_4kb[0] |= 1u64 << l1_idx;
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
            self.tree_2mb[0] &= !(1u64 << l1_idx);
        }
        self.tree_1gb[0] &= !(1u64 << l1_idx);

        let final_idx = 512 * 512 * l1_idx + 512 * l2_idx + l3_idx;
        assert!(self.allocator_state[final_idx] == PageType::Free);
        self.allocator_state[final_idx] = PageType::Frame;
        Some(final_idx)
    }

    /**
     * Allocates 2Mb page
     * return None if allocation fails
     */
    pub fn allocate_big_page(&mut self) -> Option<usize> {
        // First level search
        if self.tree_2mb[0] == 0 {
            return None;
        }
        let l1_idx = Self::bsf(self.tree_2mb[0]);
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
        self.tree_2mb[0] &= !(1u64 << l1_idx);
        for i in 0..8 {
            if self.tree_2mb[first_block_l2 + i] != 0u64 {
                self.tree_2mb[0] |= 1u64 << l1_idx;
                break;
            }
        }

        // set bits from TREE_1GB and TREE_4KB to 0
        self.tree_4kb[first_block_l2 + block_chosen_l2] &= !(1u64 << (l2_idx % 64));
        self.tree_4kb[0] &= !(1u64 << l1_idx);
        for i in 0..8 {
            if self.tree_4kb[first_block_l2 + i] != 0u64 {
                self.tree_4kb[0] |= 1u64 << l1_idx;
                break;
            }
        }

        self.tree_1gb[0] &= !(1u64 << l1_idx);

        let final_idx = 512 * 512 * l1_idx + 512 * l2_idx;
        assert!(self.allocator_state[final_idx] == PageType::Free);
        self.allocator_state[final_idx] = PageType::Big;
        Some(final_idx)
    }

    /**
     * Allocates 1Gb page
     * return None if allocation fails
     */
    pub fn allocate_huge_page(&mut self) -> Option<usize> {
        // First level search
        if self.tree_1gb[0] == 0 {
            return None;
        }
        let l1_idx = Self::bsf(self.tree_1gb[0]);
        if l1_idx >= NB_GB {
            return None;
        }

        self.tree_1gb[0] &= !(1u64 << l1_idx);
        self.tree_2mb[0] &= !(1u64 << l1_idx);
        self.tree_4kb[0] &= !(1u64 << l1_idx);

        let final_idx = 512 * 512 * l1_idx;
        if self.allocator_state[final_idx] != PageType::Free {
            println!("error: Not free!");
        }
        assert!(self.allocator_state[final_idx] == PageType::Free);
        self.allocator_state[final_idx] = PageType::Huge;
        return Some(final_idx);
    }

    /**
     * Deallocates frame
     * nothing is done if frame was not previously allocated
     */
    pub fn deallocate_frame(&mut self, frame_id: usize) {
        let mut id = frame_id;
        // return if frame was not allocated
        if self.allocator_state[id] != PageType::Frame {
            return;
        }
        self.allocator_state[id] = PageType::Free;

        let l3_block_idx = id & 0x1FF;
        id >>= 9;
        let l2_block_idx = id & 0x1FF;
        id >>= 9;
        let l1_block_idx = id & 0x1FF;

        let l1_tree_idx = 0; // assume l1_block_idx is between 0 and 63
        self.tree_4kb[l1_tree_idx] |= 1u64 << l1_block_idx;
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
            self.tree_2mb[0] |= 1u64 << l1_block_idx;
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
            self.tree_1gb[0] |= 1u64 << (l1_block_idx % 64);
        }
    }

    /**
     * Deallocates big page
     * nothing is done if page was not previously allocated
     */
    pub fn deallocate_big_page(&mut self, frame_id: usize) {
        let mut id = frame_id;
        // return if big page was not allocated
        if self.allocator_state[id] != PageType::Big {
            return;
        }
        self.allocator_state[id] = PageType::Free;

        let l3_block_idx = id & 0x1FF;
        id >>= 9;
        let l2_block_idx = id & 0x1FF;
        id >>= 9;
        let l1_block_idx = id & 0x1FF;
        assert!(l3_block_idx == 0); // l3_block_idx must be 0 for 2mb pages

        let l1_tree_idx = 0; // assume l1_block_idx is between 0 and 63
        self.tree_2mb[l1_tree_idx] |= 1u64 << l1_block_idx;
        let l2_tree_idx = TREE_1GB_SIZE + 8 * l1_block_idx + l2_block_idx / 64;
        self.tree_2mb[l2_tree_idx] |= 1u64 << (l2_block_idx % 64);

        self.tree_4kb[l2_tree_idx] |= 1u64 << (l2_block_idx % 64);
        self.tree_4kb[0] |= 1u64 << l1_block_idx;

        let mut need_to_free_1gb_level1 = true;
        let first_block_l2 = l2_tree_idx - l2_block_idx / 64;
        for i in 0..8 {
            if self.tree_2mb[first_block_l2 + i] != 0u64 {
                need_to_free_1gb_level1 = false;
                break;
            }
        }
        if need_to_free_1gb_level1 {
            self.tree_1gb[0] |= 1u64 << l1_block_idx;
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
            self.tree_1gb[0] |= 1u64 << (l1_block_idx % 64);
        }
    }

    /**
     * Deallocates huge page
     * nothing is done if frame was not previously allocated
     */
    pub fn deallocate_huge_page(&mut self, frame_id: usize) {
        let mut id = frame_id;
        // return if huge page was not allocated
        if self.allocator_state[id] != PageType::Huge {
            return;
        }
        self.allocator_state[id] = PageType::Free;

        let l3_block_idx = id & 0x1FF;
        id >>= 9;
        let l2_block_idx = id & 0x1FF;
        id >>= 9;
        let l1_block_idx = id & 0x1FF;
        assert!(l3_block_idx == 0); // l3_block_idx must be 0 for 1gb pages
        assert!(l2_block_idx == 0); // l2_block_idx must be 0 for 1gb pages

        let l1_tree_idx = 0; // assume l1_block_idx is between 0 and 63
        self.tree_1gb[l1_tree_idx] |= 1u64 << l1_block_idx;
        self.tree_2mb[l1_tree_idx] |= 1u64 << l1_block_idx;
        self.tree_4kb[l1_tree_idx] |= 1u64 << l1_block_idx;
    }

    /**
     * Checks integrity of allocated pages
     * crash if integrity is not ensured
     */
    // #[cfg(test)]
    pub fn check_integrity(&self) {
        let mut num_next_free = 0;
        for i in 0..NB_PAGES {
            if num_next_free > 0 {
                assert!(self.allocator_state[i] == PageType::Free);
            } else {
                match self.allocator_state[i] {
                    PageType::Big => {
                        assert!(i % 512 == 0);
                        num_next_free = 511
                    }
                    PageType::Huge => {
                        assert!(i % (512 * 512) == 0);
                        num_next_free = 512 * 512 - 1
                    }
                    _ => (),
                }
            }
            num_next_free -= 1;
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
            match self.allocator_state[i] {
                PageType::Frame => {
                    i += 1;
                    nb_1gb += num_free / (512 * 512);
                    nb_2mb += (num_free % (512 * 512)) / 512;
                    nb_4kb += num_free % 512;
                    num_free = 0;
                }
                PageType::Big => {
                    i += 512;
                    assert!(num_free % 512 == 0);
                    nb_1gb += num_free / (512 * 512);
                    nb_2mb += (num_free % (512 * 512)) / 512;
                    nb_4kb += num_free % 512;
                    num_free = 0;
                }
                PageType::Huge => {
                    i += 512 * 512;
                    assert!(num_free % 512 == 0);
                    nb_1gb += num_free / (512 * 512);
                    nb_2mb += (num_free % (512 * 512)) / 512;
                    num_free = 0;
                }
                PageType::Free => {
                    if (i as u64) % 512 != num_free % 512 {
                        assert!(num_free == 0);
                        nb_4kb += 1;
                    } else {
                        num_free += 1;
                    }
                    i += 1;
                }
            }
        }
        nb_1gb += num_free / (512 * 512);
        nb_2mb += (num_free % (512 * 512)) / 512;
        nb_4kb += num_free % 512;

        (nb_1gb, nb_2mb, nb_4kb)
    }

    // perf
    // rust-gdb
}

#[cfg(test)]
mod tests {
    use super::*;
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
