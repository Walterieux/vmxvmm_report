# Operating System Kernel Frame Allocator in Rust

This project present a unique frame allocator based on the buddy allocator. This enables the allocation of three different sizes from Intel x86-64 page tables; 4Kb, 2Mb and 1Gb.
The algorithm effectively allocates and deallocates frames using 512-ary threes and two Intel-specific instructions (BSF & BSR). 

Memory overhead is less than 20Mb for 512Gb.

## Code Structure

### Allocator

The allocator code is located in the file `allocator/allocator.rs`. The main goal of the allocator is to return an address when requested for one of the following size: 4Kb, 2Mb and 1Gb. Once an memory zone is allocated, it cannot be reused until it is deallocated (no memory sharing).

### BSF Benchmark

Folder `bsf_benchmark` contains the benchmark of bit scan forward (BSF) compared to a simple loop. Bit scan forward operator returns the index of the first 1, it is used to effeciently find the first free page.

### External Fragmentation Measurement

Folder `distribution` contains external fragmentation measurement given a predifined scenario which simulate a memory usage of 70%.