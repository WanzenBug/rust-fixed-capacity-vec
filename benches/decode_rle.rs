#![feature(test)]
extern crate fixed_capacity_vec;
extern crate test;

use fixed_capacity_vec::AsFixedCapacityVec;

const INIT_VEC_SIZE: usize = 600;
const RLE_FRAGMENT_SIZE: usize = 256;
const RLE_FILL_SIZE: usize = 3333;

fn decode_rle_naive(buffer: &mut Vec<u8>, repeating_fragment_len: usize, num_bytes_to_fill: usize) {
    buffer.reserve(num_bytes_to_fill); // allocate required memory immediately, it's faster this way
    for _ in 0..num_bytes_to_fill {
        // byte_to_copy variable is needed because buffer.push(buffer[i]) doesn't compile
        let byte_to_copy = buffer[buffer.len() - repeating_fragment_len];
        buffer.push(byte_to_copy);
    }
}

fn decode_rle_vuln(buffer: &mut Vec<u8>, repeating_fragment_len: usize, num_bytes_to_fill: usize) {
    buffer.reserve(num_bytes_to_fill); // allocate required memory immediately, it's faster this way
    unsafe {
        // set length of the buffer up front so we can set elements in a slice instead of pushing
        let len = buffer.len();
        buffer.set_len(len + num_bytes_to_fill);
    }
    for i in (buffer.len() - num_bytes_to_fill)..buffer.len() {
        buffer[i] = buffer[i - repeating_fragment_len];
    }
}

fn decode_rle_lib_naive(
    buffer: &mut Vec<u8>,
    repeating_fragment_len: usize,
    num_bytes_to_fill: usize,
) {
    let (old, mut append) = buffer.with_fixed_capacity(num_bytes_to_fill);
    let slice_to_repeat = &old[(old.len() - repeating_fragment_len)..]; // figure out what to repeat
    append.extend(
        slice_to_repeat
            .iter()
            .cycle()
            .map(|u| *u)
            .take(num_bytes_to_fill),
    );
}

fn decode_rle_lib_optim(
    buffer: &mut Vec<u8>,
    repeating_fragment_len: usize,
    num_bytes_to_fill: usize,
) {
    let (old, mut append) = buffer.with_fixed_capacity(num_bytes_to_fill);
    let slice_to_repeat = &old[(old.len() - repeating_fragment_len)..]; // figure out what to repeat
    let mut filled = 0;
    while filled + slice_to_repeat.len() < num_bytes_to_fill {
        append.extend_from_slice(slice_to_repeat); // Hopefully memcpy here
        filled += slice_to_repeat.len();
    }
    append.extend(
        slice_to_repeat[..(num_bytes_to_fill - filled)]
            .iter()
            .map(|u| *u),
    );
}

fn get_initial_vec() -> Vec<u8> {
    let mut start = 0u8;
    std::iter::repeat_with(|| {
        let val = start;
        start = start.wrapping_add(1);
        val
    }).take(INIT_VEC_SIZE)
        .collect()
}

fn check_result(res: &[u8]) {
    for i in 0..(res.len() - 256) {
        assert_eq!(res[i], res[i + 256])
    }
}

#[bench]
fn bench_decode_rle_naive(b: &mut test::Bencher) {
    let mut test_vec = get_initial_vec();
    b.iter(|| {
        decode_rle_naive(&mut test_vec, RLE_FRAGMENT_SIZE, RLE_FILL_SIZE);
    });
    test::black_box(&test_vec);
    check_result(&test_vec);
}

#[bench]
fn bench_decode_rle_vuln(b: &mut test::Bencher) {
    let mut test_vec = get_initial_vec();
    b.iter(|| {
        decode_rle_vuln(&mut test_vec, RLE_FRAGMENT_SIZE, RLE_FILL_SIZE);
    });
    test::black_box(&test_vec);
    check_result(&test_vec);
}

#[bench]
fn bench_decode_rle_lib_naive(b: &mut test::Bencher) {
    let mut test_vec = get_initial_vec();
    b.iter(|| {
        decode_rle_lib_naive(&mut test_vec, RLE_FRAGMENT_SIZE, RLE_FILL_SIZE);
    });
    test::black_box(&test_vec);
    check_result(&test_vec);
}

#[bench]
fn bench_decode_rle_lib_opt(b: &mut test::Bencher) {
    let mut test_vec = get_initial_vec();
    b.iter(|| {
        decode_rle_lib_optim(&mut test_vec, RLE_FRAGMENT_SIZE, RLE_FILL_SIZE);
    });
    test::black_box(&test_vec);
    check_result(&test_vec);
}
