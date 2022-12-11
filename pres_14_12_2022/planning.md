# An Operating System Kernel Frame Allocator in Rust

## Why do we need a custom allocator?
Objective is to have a provable in formal verification ->only loops of constant variables.
Here we want to implement an allocator for 4Kb, 2Mb and 1Gb frames. 

## Presentation of different allocators models

### Bump Allocator
Just an ilustration image is sufficient

### Free Linked List Allocator
Advantage and disadvantage
O(1).

### Buddy Allocator
Size are power of 2.
O(log(n)) complexity allocation, deallocation is more complex.
Internal Frag but no extern fragmentation.

## Custom Allocator
Is a sort of Buddy Allocator with three trees instead of one.

For each tree, it has a 1 (free) if at least one children is free. Memory is shared between trees.

### How does it works?
Here's is the video.

Deallocation is safe, just pass a pointer this will detect the type that was allocated with extra need of memory

### Memory Overhead
~20Mb overhead for 512Gb allocated -> 0.004% overhead! 

### Constant time?
BSF operation benchmark here

## Performance analysis

### Stability metrics
Quantitative and spacial metrics.

### Time metrics



## Conclusion
No comparison with existing algorithm due to the fact that this is difficult to have a standalone  allocator. Ex. linux page allocator have many dependencies and is not exactly the same (parallelism).

A good point we can explore is to make the algorrithm parallisable.
