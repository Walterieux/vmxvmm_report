#![feature(test)]

// to run: cargo +nightly bench

extern crate test;

use core::arch::asm;
use csv::Writer;
use std::time::Instant;
use test::*;

#[inline(always)]
fn bsf(input: u64) -> u32 {
    let mut pos: u32;
    unsafe {
        asm! {
            "bsf {pos}, {input}",
            input = in(reg) input,
            pos = out(reg) pos,
            options(nomem, nostack),
        };
    };
    pos
}

#[inline(always)]
fn find_first_one(input: u64) -> u32 {
    let mut temp = input;
    for i in 0..64 {
        if temp & 1 == 1 {
            return i;
        }
        temp >>= 1;
    }
    return 64;
}

#[inline(always)]
fn trailing_zeros(input: u64) -> u32 {
    input.trailing_zeros()
}

fn main() {
    let mut x_arr: [u64; 8] = [1, 0, 0, 0, 0, 0, 0, 0];
    let mut array_idx_set = 0;

    let mut wtr = Writer::from_path("foo.csv").unwrap();
    let _ = wtr.write_record(&["iter", "bsf", "loop", "tralling"]);

    for i in 0..512 {
        print!("\nBit {} set:\n", i);
        let mut elapsed_time1 = 0;

        let n = test::black_box(1000);
        let mut ctr = 0;
        for i in 0..n {
            for j in 0..8 {
                let input = test::black_box(x_arr[j]);
                let start1 = Instant::now();
                if input == 0 {
                    ctr += 64;
                    continue;
                }
                let idx = bsf(input);
                elapsed_time1 += start1.elapsed().as_nanos();
                ctr += idx;
                break;
            }
        }
        println!("Running bsf() took {:?}. sum = {}", elapsed_time1, ctr);
        ctr = 0;

        let mut elapsed_time2 = 0;
        let n = test::black_box(1000);
        for i in 0..n {
            for j in 0..8 {
                let input = test::black_box(x_arr[j]);
                let start2 = Instant::now();
                let idx = find_first_one(input);
                elapsed_time2 += start2.elapsed().as_nanos();
                if idx != 64 {
                    ctr += idx;
                    break;
                }
                ctr += 64;
            }
        }

        println!(
            "Running find_first_one() took {:?}. sum = {}",
            elapsed_time2, ctr
        );
        ctr = 0;
        let mut elapsed_time3 = 0;

        let n = test::black_box(1000);
        for i in 0..n {
            for j in 0..8 {
                let input = test::black_box(x_arr[j]);
                let start3 = Instant::now();
                if input == 0 {
                    ctr += 64;
                    continue;
                }
                let idx = trailing_zeros(input);
                elapsed_time3 += start3.elapsed().as_nanos();

                ctr += idx;
                break;
            }
        }

        println!(
            "Running trailing_zeros() took {:?}. sum = {}",
            elapsed_time3, ctr
        );
        ctr = 0;
        x_arr[array_idx_set] <<= 1;
        if (i + 1) % 64 == 0 && i != 511 {
            for j in 0..8 {
                x_arr[j] = 0;
            }
            array_idx_set += 1;
            x_arr[array_idx_set] = 1;
        }
        println!("{:?}\n", x_arr);

        if i % 3 == 0 {
            let _ = wtr.write_record(&[
                i.to_string(),
                elapsed_time1.to_string(),
                elapsed_time2.to_string(),
                elapsed_time3.to_string(),
            ]);
        }
    }
    let _ = wtr.flush();
}

#[cfg(test)]
mod tests {
    use super::*;
    use test::Bencher;

    #[bench]
    fn bench_bsf(b: &mut Bencher) {
        b.iter(|| {
            let n = test::black_box(1000);
            let mut ctr = 0;
            for i in 0..n {
                let input = test::black_box(1 << (i & 0x3F));
                ctr += bsf(input);
            }
            ctr
        });
    }

    #[bench]
    fn bench_loop(b: &mut Bencher) {
        b.iter(|| {
            let n = test::black_box(1000);
            let mut ctr = 0;
            for i in 0..n {
                let input = test::black_box(1 << (i & 0x3F));
                ctr += find_first_one(input);
            }
            ctr
        });
    }

    #[bench]
    fn bench_trailling(b: &mut Bencher) {
        b.iter(|| {
            let n = test::black_box(1000);
            let mut ctr = 0;
            for i in 0..n {
                let input = test::black_box(1 << (i & 0x3F));
                ctr += trailing_zeros(input);
            }
            ctr
        });
    }
}
