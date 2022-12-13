# An Operating System Kernel Frame Allocator in Rust

## Why do we need a custom allocator?
Here we want to implement an allocator for 4Kb, 2Mb and 1Gb frames (due to intel x86-64 page table size).
We want to avoid path explosion to be able to perform symbolic execution.
Objective is to have a provable in formal verification ->only loops of constant variables.
 

## Presentation of different allocators models

### Bump Allocator
Next pointer is reset only when all blocks are freed.
Here we can allocate any size we want.
no internal fragmentation, allocated block have the right size.
External fragmenation caused be freed blocks.


### Free Linked List Allocator
Why not a good idea: we cannot split the block because of complexity of merging blocks.
O(1).

### Buddy Allocator
Size are power of 2 -> internal fragmentation
bad placement leads to external fragmentation
O(log(n)) complexity allocation, deallocation is more complex.

## Custom Allocator
Is a sort of Buddy Allocator with three trees instead of one. One tree for each block size.
512 children instead of 2. why 512? because of ration between each block size.

Initial state: all bits to 1 -> 1 is for free and 0 occupied.
Algorithm explanation:
	- Always starts on top for each tree
	- search for first bit set to one
	- got to children's bock and do the same
	- block path on others tree.
For each tree, it has a 1 (free) if at least one children is free. Memory is shared between trees.

Deallocation is safe, just pass a pointer this will detect the type that was allocated with extra need of memory

In reality trees are stored flatten.

### How does it works?
See how bits are update across different trees.

### Bit scan forward (BSF) operator
Why 64 bits? Because 64 bits system, we need 8 operations to go trough all 512 bits.
Why simple for i is not efficient enough for us? Max optimization uses loop unrolling, but we still have 16 iterations.

Here comes bit scan forward from x86 assembly instructions. Way more efficent than simple loop.

Rust has a trailing_zeros method which does the same.


### Memory Overhead
Linear overhead.
less than 20Mb overhead for 512Gb allocated -> 0.004% overhead! 

biggest tree is the one for 4kb.

### Benchmark setup
70% objective

### Stability metrics
Quantitative and spacial metrics.

External fragmentation cause of free_4Kb and free_2mb.


## Conclusion
No comparison with existing algorithm due to the fact that this is difficult to have a standalone  allocator. Ex. linux page allocator have many dependencies and is not exactly the same (parallelism).

A good point we can explore is to make the algorrithm parallisable.
