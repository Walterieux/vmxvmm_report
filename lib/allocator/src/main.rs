pub mod allocator;

use crate::allocator::BuddyAllocator;
fn main() {
    let mut frame_alloc = Box::new(BuddyAllocator::new());
    println!("Allocator instanciated!");

    for i in 0..100*512*512 * 10 {
        let new_frame = frame_alloc.allocate_frame();
        frame_alloc.deallocate_frame(new_frame.unwrap());
    }

}
