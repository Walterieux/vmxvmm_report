use csv::Writer;
use rand::distributions::{Bernoulli, Distribution};
use rand::prelude::IteratorRandom;
use rand::rngs::ThreadRng;
use statrs::distribution::DiscreteCDF;
use statrs::distribution::Poisson;
use indicatif::ProgressBar;

// use ulimit -s XXX to run the code

fn main() {
    //save_plot_distribution(70.0);
    //no_internal_fragmentation(70.0, 512);
    custom_allocator(70.0);
}

/**
 * Utilitary function to pop a random element from a given Vec
 * To maximize performance, order is not kept
 */

fn choose(raw: &mut Vec<usize>, rnd: &mut ThreadRng) -> Option<usize> {
    let i = (0..raw.len()).choose(rnd)?;
    Some(raw.swap_remove(i))
}

/**
 * Simulate the custom buddy allocator
 *
 * lambda: threshold memory objective  (0;100)
 */
#[allow(dead_code)]
fn custom_allocator(lambda: f64) {
    assert!(0.0 < lambda && lambda < 100.0);

    let num_gb = 512;

    let mut frame_alloc = Box::new(allocator::BuddyAllocator::new());

    let poisson = Poisson::new(lambda).unwrap();
    let mut wtr = Writer::from_path("custom_allocator.csv").unwrap();
    let _ = wtr.write_record(&[
        "time",
        "4kb_alloc",
        "2mb_alloc",
        "1gb_alloc",
        "1gb_free",
        "2mb_free",
        "4kb_free",
    ]);

    let tot_num_4kb_blocks: u64 = num_gb * 512 * 512;
    let mut free_num_4kb_blocks = tot_num_4kb_blocks;

    let mut allocated_4kb: u64 = 0;
    let mut allocated_2mb: u64 = 0;
    let mut allocated_1gb: u64 = 0;

    let mut allocated_4kb_ids: Vec<usize> = Vec::new();
    let mut allocated_2mb_ids: Vec<usize> = Vec::new();
    let mut allocated_1gb_ids: Vec<usize> = Vec::new();

    let prob_4kb: f64 = 262144.0 / 262657.0;
    let prob_2mb: f64 = prob_4kb / 512.0;
    let prob_1gb: f64 = prob_2mb / 512.0;

    let mut rng = rand::thread_rng();

    let prob_4kb_ber = Bernoulli::from_ratio(262144, 262657).unwrap();
    let prob_2mb_ber = Bernoulli::from_ratio(512, 513).unwrap();

    let bar = ProgressBar::new(1000);

    for t in 0..2_000_000_000 {
        // stats
        if t % 2_000_000 == 0 {
            bar.inc(1);
            let (free_1gb, free_2mb, free_4kb) = frame_alloc.stat_free_memory();
            let _ = wtr.write_record(&[
                t.to_string(),
                allocated_4kb.to_string(),
                (allocated_2mb * 512).to_string(),
                (allocated_1gb * 512 * 512).to_string(),
                (free_1gb * 512 * 512).to_string(),
                (free_2mb * 512).to_string(),
                free_4kb.to_string(),
            ]);
        }

        let mem_occup: u64 = 100 * (tot_num_4kb_blocks - free_num_4kb_blocks) / tot_num_4kb_blocks;

        let d = Bernoulli::new(poisson.sf(mem_occup)).unwrap();

        let is_allocation = d.sample(&mut rng);
        if is_allocation {
            if prob_4kb_ber.sample(&mut rng) {
                let frame = frame_alloc.allocate_frame();
                if frame.is_some() {
                    allocated_4kb_ids.push(frame.unwrap());
                    free_num_4kb_blocks -= 1;
                    allocated_4kb += 1;
                } else {
                    println!("Not enough memory to allocate a frame!");
                }
            } else if prob_2mb_ber.sample(&mut rng) {
                let frame = frame_alloc.allocate_big_page();
                if frame.is_some() {
                    allocated_2mb_ids.push(frame.unwrap());
                    free_num_4kb_blocks -= 512;
                    allocated_2mb += 1;
                    // println!("allocate 2mb at time {}", t);
                } else {
                    println!("Not enough memory to allocate a big page!");
                }
            } else {
                let frame = frame_alloc.allocate_huge_page();
                if frame.is_some() {
                    allocated_1gb_ids.push(frame.unwrap());
                    free_num_4kb_blocks -= 512 * 512;
                    allocated_1gb += 1;
                    // println!("allocate 1gb at time {}", t);
                } else {
                    println!("Not enough memory to allocate a huge page!");
                }
            }
        } else {
            if prob_4kb_ber.sample(&mut rng) {
                if allocated_4kb > 0 {
                    frame_alloc.deallocate_frame(choose(&mut allocated_4kb_ids, &mut rng).unwrap());
                    free_num_4kb_blocks += 1;
                    allocated_4kb -= 1;
                }
            } else if prob_2mb_ber.sample(&mut rng) {
                if allocated_2mb > 0 {
                    frame_alloc
                        .deallocate_big_page(choose(&mut allocated_2mb_ids, &mut rng).unwrap());
                    free_num_4kb_blocks += 512;
                    allocated_2mb -= 1;
                    // println!("dellocate 2mb at time {}", t);
                }
            } else {
                if allocated_1gb > 0 {
                    frame_alloc
                        .deallocate_huge_page(choose(&mut allocated_1gb_ids, &mut rng).unwrap());
                    free_num_4kb_blocks += 512 * 512;
                    allocated_1gb -= 1;
                    // println!("deallocate 1gb at time {}", t);
                }
            }
        }
    }

    bar.finish();

    let _ = wtr.flush();
}

/**
 * Simulate a perfect case were there is no internal fragmentation
 * this will be usefull to compare with the custom allocator and linux allocator
 *
 * lambda: threshold memory objective  (0;100)
 * nb_gb: available memory (0; 512]
 */
#[allow(dead_code)]
fn no_internal_fragmentation(lambda: f64, num_gb: u64) {
    assert!(0.0 < lambda && lambda < 100.0);
    assert!(0 < num_gb && num_gb <= 512);

    let poisson = Poisson::new(lambda).unwrap();
    let mut wtr = Writer::from_path("no_internal_frag_usage.csv").unwrap();
    let _ = wtr.write_record(&[
        "time",
        "4kb_alloc",
        "2mb_alloc",
        "1gb_alloc",
        "1gb_free",
        "2mb_free",
        "4kb_free",
    ]);

    let tot_num_4kb_blocks: u64 = num_gb * 512 * 512;
    let mut free_num_4kb_blocks = tot_num_4kb_blocks;

    let mut allocated_4kb: u64 = 0;
    let mut allocated_2mb: u64 = 0;
    let mut allocated_1gb: u64 = 0;

    let prob_4kb: f64 = 262144.0 / 262657.0;
    let prob_2mb: f64 = prob_4kb / 512.0;
    let prob_1gb: f64 = prob_2mb / 512.0;

    let prob_4kb_ber = Bernoulli::from_ratio(262144, 262657).unwrap();
    let prob_2mb_ber = Bernoulli::from_ratio(512, 513).unwrap();

    let mut rng = rand::thread_rng();
    for t in 0..100_000_000 {
        // stats
        if t % 100_000 == 0 {
            let (free_1gb, free_2mb, free_4kb) = stat_free_memory(free_num_4kb_blocks);
            let _ = wtr.write_record(&[
                t.to_string(),
                allocated_4kb.to_string(),
                (allocated_2mb * 512).to_string(),
                (allocated_1gb * 512 * 512).to_string(),
                (free_1gb * 512 * 512).to_string(),
                (free_2mb * 512).to_string(),
                free_4kb.to_string(),
            ]);
        }

        let mem_occup: u64 = 100 * (tot_num_4kb_blocks - free_num_4kb_blocks) / tot_num_4kb_blocks;

        let d = Bernoulli::new(poisson.sf(mem_occup)).unwrap();

        let is_allocation = d.sample(&mut rng);
        if is_allocation {
            if prob_4kb_ber.sample(&mut rng) {
                if free_num_4kb_blocks > 0 {
                    free_num_4kb_blocks -= 1;
                    allocated_4kb += 1;
                } else {
                    println!("Not enough memory to allocate a frame!");
                }
            } else if prob_2mb_ber.sample(&mut rng) {
                if free_num_4kb_blocks >= 512 {
                    free_num_4kb_blocks -= 512;
                    allocated_2mb += 1;
                    // println!("allocate 2mb at time {}", t);
                } else {
                    println!("Not enough memory to allocate a big page!");
                }
            } else {
                if free_num_4kb_blocks >= 512 * 512 {
                    free_num_4kb_blocks -= 512 * 512;
                    allocated_1gb += 1;
                    // println!("allocate 1gb at time {}", t);
                } else {
                    println!("Not enough memory to allocate a huge page!");
                }
            }
        } else {
            if prob_4kb_ber.sample(&mut rng) {
                if allocated_4kb > 0 {
                    free_num_4kb_blocks += 1;
                    allocated_4kb -= 1;
                }
            } else if prob_2mb_ber.sample(&mut rng) {
                if allocated_2mb > 0 {
                    free_num_4kb_blocks += 512;
                    allocated_2mb -= 1;
                    // println!("dellocate 2mb at time {}", t);
                }
            } else {
                if allocated_1gb > 0 {
                    free_num_4kb_blocks += 512 * 512;
                    allocated_1gb -= 1;
                    // println!("deallocate 1gb at time {}", t);
                }
            }
        }
    }

    let _ = wtr.flush();
}

/**
 * given the number of 4kb free blocks, it returns the numbers of 1gb_block, 2mb_blocks and 4kb_blocks that can be allocated
 * without considering internal fragmentation
 */
#[allow(dead_code)]
fn stat_free_memory(num_free_4kb_blocks: u64) -> (u64, u64, u64) {
    let mut available_4kb_blocks = num_free_4kb_blocks;
    let nb_gb: u64 = available_4kb_blocks / (512 * 512);
    available_4kb_blocks -= nb_gb * 512 * 512;
    let nb_2mb: u64 = available_4kb_blocks / 512;
    available_4kb_blocks -= nb_2mb * 512;
    let nb_4kb = available_4kb_blocks;

    (nb_gb, nb_2mb, nb_4kb)
}

/**
 * Save poisson inverse cumulative distribution function given lambda
 */
#[allow(dead_code)]
fn save_plot_distribution(lambda: f64) {
    let n = Poisson::new(lambda).unwrap();

    let mut wtr = Writer::from_path("distribution.csv").unwrap();
    let _ = wtr.write_record(&["percentage used", "prob"]);

    for i in 0..101 {
        let _ = wtr.write_record(&[i.to_string(), n.sf(i).to_string()]);
    }

    let _ = wtr.flush();
}
