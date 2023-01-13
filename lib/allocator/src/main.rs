pub mod allocator;

use crate::allocator::BuddyAllocator;
fn main() {
    let mut frame_alloc = Box::new(BuddyAllocator::new());
    println!("Allocator instanciated!");

    const NB_GB: usize = 512;
    const NB_PAGES: usize = 512 * 512 * NB_GB;

    let mut cnt = 0;

    for s in 0..1000 {
        println!("step: {}", s);
        // allocates all possible frames
        for _ in 0..NB_PAGES {
            let frame = frame_alloc.allocate_frame();
            cnt += frame.unwrap();
        }

        // deallocates all frames
        for i in 0..NB_PAGES {
            frame_alloc.deallocate_frame(i);
        }

        // allocates all possible big pages
        for _ in 0..512 {
            for _ in 0..NB_PAGES / 512 {
                let big_page = frame_alloc.allocate_big_page();
                cnt += big_page.unwrap();
            }

            // deallocates all big pages
            for i in 0..NB_PAGES / 512 {
                frame_alloc.deallocate_big_page(i * 512);
            }
        }

        // allocates all possible big pages
        for _ in 0..512 * 512 {
            for _ in 0..NB_GB {
                let huge_page = frame_alloc.allocate_huge_page();
                cnt += huge_page.unwrap();
            }

            // deallocates all big pages
            for i in 0..NB_GB {
                frame_alloc.deallocate_huge_page(i * 512 * 512);
            }
        }
    }
    println!("counter: {}", cnt)
}
