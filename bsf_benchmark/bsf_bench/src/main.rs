#![feature(test)]

extern crate test;

use core::arch::asm;
use std::time::Instant;
use std::error::Error;
use csv::Writer;


#[inline(always)]
fn bsf(input: u64) -> u32 {
    let mut pos: u32;
    // "bsf %1, %0" : "=r" (pos) : "rm" (input),
    unsafe {
        asm! {
            "bsf {pos}, {input}",
            input = in(reg) input,
            pos = out(reg) pos,
            options(nomem, nostack),
        };
    };
    return pos;
}

#[inline(always)]
fn find_first_one(input: u64) -> u32 {
    let mut temp = input;
    for i in 0..64 {
        if temp & 1 == 1{
            return i;
        }
        temp >>= 1;
    }
    return 0;
}

#[inline(always)]
fn trailing_zeros(input: u64) -> u32 {
    input.trailing_zeros()
}

fn main() {
    let mut x = 0x0000000000000001u64;

    let mut wtr = Writer::from_path("foo.csv").unwrap();
    wtr.write_record(&["iter", "bsf", "loop", "tralling"]);

    for i in 0..64 {

        print!("\nBit {} set:\n", i);
        let start1 = Instant::now();

        for _ in 0..100000 {
            let a = bsf(x);
        }
        let elapsed_time1 = start1.elapsed();
        println!("Running bsf() took {:?}.", elapsed_time1);


        let start2 = Instant::now();
        for _ in 0..100000 {
            let b = find_first_one(x);
        }
        let elapsed_time2 = start2.elapsed();
        println!("Running find_first_one() took {:?}.", elapsed_time2);

        let start3 = Instant::now();
        for _ in 0..100000 {
            let c = trailing_zeros(x);
        }
            let elapsed_time3 = start3.elapsed();
        println!("Running trailing_zeros() took {:?}.", elapsed_time3);
        x <<= 1;

        wtr.write_record(&[i.to_string(), elapsed_time1.as_nanos().to_string(), elapsed_time2.as_nanos().to_string(), elapsed_time3.as_nanos().to_string()]);

    }
    wtr.flush();
    
}

#[cfg(test)]
mod tests {
    use super::*;
    use test::Bencher;

    #[bench]
    fn bench_bsf(b : &mut Bencher) {
        b.iter(|| {
            let n = test::black_box(100000);
            let mut ctr = 0;
            for i in 0..n {
                
                ctr += bsf(1<<(i%64));
            }
            ctr
            
        });


    }

    #[bench]
    fn bench_loop(b : &mut Bencher) {
        b.iter(|| {
            let n = test::black_box(100000);
            let mut ctr = 0;
            for i in 0..n {
                
                ctr += find_first_one(1<<(i%64));
            }
            ctr
        });
        
    }

    #[bench]
    fn bench_trailling(b : &mut Bencher) {
        b.iter(|| {
            let n = test::black_box(100000);
            let mut ctr = 0;
            for i in 0..n {
                
                ctr += trailing_zeros(1<<(i%64));
            }
            ctr
        });
        
    }
}
