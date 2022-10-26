//! Revisited buddy allocator

use core::arch::asm;

const NB_GB: usize = 2;
//const NB_PAGES: usize = 512 * 512 * NB_GB;
const TREE_4KB_SIZE: usize = 8209;
const TREE_2MB_SIZE: usize = 17;
const TREE_1GB_SIZE: usize = 1; // TODO change that to upper log64(x)

pub struct BuddyAllocator {
    tree_4kb: [u64; TREE_4KB_SIZE],
    tree_2mb: [u64; TREE_2MB_SIZE],
    tree_1gb: [u64; TREE_1GB_SIZE],
}

impl BuddyAllocator {
    pub fn new() -> Self {
        Self {
            tree_4kb: [0xFFFFFFFFFFFFFFFF; TREE_4KB_SIZE],
            tree_2mb: [0xFFFFFFFFFFFFFFFF; TREE_2MB_SIZE],
            tree_1gb: [0xFFFFFFFFFFFFFFFF; TREE_1GB_SIZE],
        }
    }

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
        if l3_idx == 511 {
            self.tree_4kb[first_block_l2 + block_chosen_l2] &= !(1u64 << (l2_idx % 64));
            if l2_idx == 511 {
                self.tree_4kb[0] &= !(1u64 << l1_idx);
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
        Some(final_idx)
    }

    /**
     * Allocates 2Mb page
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
        if l2_idx == 511 {
            self.tree_2mb[0] &= !(1u64 << l1_idx);
        }

        // set bits from TREE_1GB and TREE_4KB to 0
        self.tree_4kb[first_block_l2 + block_chosen_l2] &= !(1u64 << (l2_idx % 64));
        let mut need_to_set_4kb_level1 = true;
        for i in 0..8 {
            if self.tree_4kb[first_block_l2 + i] != 0 {
                need_to_set_4kb_level1 = false;
                break;
            }
        }
        if need_to_set_4kb_level1 {
            self.tree_4kb[0] &= !(1u64 << l1_idx);
        }

        self.tree_1gb[0] &= !(1u64 << l1_idx);

        let final_idx = 512 * 512 * l1_idx + 512 * l2_idx;
        Some(final_idx)
    }

    /**
     * Allocates 1Gb page
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
        return Some(final_idx);
    }

    pub fn deallocate_frame(&mut self, frame_id: usize) {
        let mut id = frame_id;

        let l3_block_idx = id & 0x1FF;
        id >>= 9;
        let l2_block_idx = id & 0x1FF;
        id >>= 9;
        let l1_block_idx = id & 0x1FF;

        // TODO check that frame was previously allocated
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
            if self.tree_2mb[first_block_l2 + i] != 0u64 {
                need_to_free_1gb_level1 = false;
                break;
            }
        }
        if need_to_free_1gb_level1 {
            self.tree_1gb[0] |= 1u64 << (l1_block_idx % 64);
        }
    }

    pub fn deallocate_big_page(&mut self, frame_id: usize) {
        let mut id = frame_id;

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

    pub fn deallocate_huge_page(&mut self, frame_id: usize) {
        let mut id = frame_id;

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
}

#[cfg(test)]
mod tests {
    use super::*;
    const NB_PAGES: usize = 512 * 512 * NB_GB;

    #[test]
    fn test_alloc_works() {
        let mut frame_alloc = BuddyAllocator::new();
        let new_frame = frame_alloc.allocate_frame();
        assert!(new_frame.is_some());
    }

    #[test]
    fn test_alloc_when_full() {
        let mut frame_alloc = BuddyAllocator::new();
        for _ in 0..NB_PAGES {
            let new_frame = frame_alloc.allocate_frame();
            assert!(new_frame.is_some());
        }
        let new_frame = frame_alloc.allocate_frame();
        assert!(new_frame.is_none());
    }

    #[test]
    fn test_alloc_and_dealloc_several_times() {
        let mut frame_alloc = BuddyAllocator::new();
        for _ in 0..NB_PAGES * 10 {
            let new_frame = frame_alloc.allocate_frame();
            assert!(new_frame.is_some());
            frame_alloc.deallocate_frame(new_frame.unwrap());
        }
    }

    #[test]
    fn test_two_allocated_frame_are_diff() {
        let mut frame_alloc = BuddyAllocator::new();
        let frame1 = frame_alloc.allocate_frame();
        assert!(frame1.is_some());
        let frame2 = frame_alloc.allocate_frame();
        assert!(frame2.is_some());

        assert_ne!(frame1.as_ref().unwrap(), frame2.as_ref().unwrap());
        assert_ne!(frame1.as_ref().unwrap(), frame2.as_ref().unwrap());
    }

    #[test]
    fn test_alloc_and_dealloc_big_several_times() {
        let mut frame_alloc = BuddyAllocator::new();
        for _ in 0..NB_PAGES {
            let new_frame = frame_alloc.allocate_big_page();
            assert!(new_frame.is_some());
            frame_alloc.deallocate_big_page(new_frame.unwrap());
        }
    }

    #[test]
    fn test_alloc_and_dealloc_huge_several_times() {
        let mut frame_alloc = BuddyAllocator::new();
        for _ in 0..NB_PAGES {
            let new_frame = frame_alloc.allocate_huge_page();
            assert!(new_frame.is_some());
            frame_alloc.deallocate_huge_page(new_frame.unwrap());
        }
    }

    #[test]
    fn test_allocate_different_types() {
        let mut frame_alloc = BuddyAllocator::new();
        let frame = frame_alloc.allocate_frame();
        assert!(frame.is_some());
        let big_page = frame_alloc.allocate_big_page();
        assert!(big_page.is_some());
        let huge_page = frame_alloc.allocate_huge_page();
        assert!(huge_page.is_some());

        assert_ne!(frame.unwrap(), big_page.unwrap());
        assert_ne!(frame.unwrap(), huge_page.unwrap());
        assert_ne!(big_page.unwrap(), huge_page.unwrap());
    }

    #[test]
    fn test_fill_memory_with_frame_and_big() {
        let mut frame_alloc = BuddyAllocator::new();

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

        // assert memory is full
        let frame = frame_alloc.allocate_frame();
        assert!(frame.is_none());
        let big_page = frame_alloc.allocate_big_page();
        assert!(big_page.is_none());
        let huge_page = frame_alloc.allocate_huge_page();
        assert!(huge_page.is_none());
    }

    #[test]
    fn test_alloc_and_free_with_all_types() {
        let mut frame_alloc = BuddyAllocator::new();

        let frame = frame_alloc.allocate_frame();
        assert!(frame.is_some());
        let big_page = frame_alloc.allocate_big_page();
        assert!(big_page.is_some());
        let mut huge_page = frame_alloc.allocate_huge_page();
        assert!(huge_page.is_some());
        for _ in 0..NB_GB - 2 {
            huge_page = frame_alloc.allocate_huge_page();
            assert!(huge_page.is_some());
        }
        let huge_page1 = frame_alloc.allocate_huge_page();
        assert!(huge_page1.is_none());
        frame_alloc.deallocate_huge_page(huge_page.unwrap());
        let huge_page2 = frame_alloc.allocate_huge_page();
        assert!(huge_page2.is_some());
        let big_page1 = frame_alloc.allocate_big_page();
        assert!(big_page1.is_some());
        let frame1 = frame_alloc.allocate_frame();
        assert!(frame1.is_some());
    }

    #[test]
    fn test_dealloc_when_full() {
        let mut frame_alloc = BuddyAllocator::new();

        for _ in 0..2 {
            // allocates all possible frames
            for _ in 0..NB_PAGES {
                let frame = frame_alloc.allocate_frame();
                assert!(frame.is_some());
            }
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

            // allocates all possible big pages
            for _ in 0..NB_PAGES / 512 {
                let big_page = frame_alloc.allocate_big_page();
                assert!(big_page.is_some());
            }
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

            // allocates all possible big pages
            for _ in 0..NB_GB {
                let huge_page = frame_alloc.allocate_huge_page();
                assert!(huge_page.is_some());
            }
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

            // allocates all possible frames
            for _ in 0..NB_PAGES {
                let frame = frame_alloc.allocate_frame();
                assert!(frame.is_some());
            }
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

            // allocates all possible big pages
            for _ in 0..NB_GB {
                let huge_page = frame_alloc.allocate_huge_page();
                assert!(huge_page.is_some());
            }
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

            // allocates all possible big pages
            for _ in 0..NB_PAGES / 512 {
                let big_page = frame_alloc.allocate_big_page();
                assert!(big_page.is_some());
            }
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
        }
    }
}
