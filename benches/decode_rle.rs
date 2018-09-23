#![feature(test)]
extern crate fixed_capacity_vec;
extern crate test;

use fixed_capacity_vec::VecExt;

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
    let full_repeats = num_bytes_to_fill / slice_to_repeat.len();
    append.extend_with_repeat(slice_to_repeat, full_repeats);
    let filled = append.len();
    append.extend_from_slice(&slice_to_repeat[..(num_bytes_to_fill - filled)]);
}

fn decode_rle_lib_fill_unsafe(
    buffer: &mut Vec<u8>,
    repeating_fragment_len: usize,
    num_bytes_to_fill: usize,
) {
    buffer.fill_with(num_bytes_to_fill, |slice| {
        unsafe { *slice.get_unchecked(slice.len() - repeating_fragment_len) }
    })
}

fn decode_rle_lib_fill_safe(
    buffer: &mut Vec<u8>,
    repeating_fragment_len: usize,
    num_bytes_to_fill: usize,
) {
    buffer.fill_with(num_bytes_to_fill, |slice| {
        slice[slice.len() - repeating_fragment_len]
    })
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
    for i in INIT_VEC_SIZE..(res.len() - RLE_FRAGMENT_SIZE) {
        assert_eq!(res[i], res[i + RLE_FRAGMENT_SIZE])
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

#[bench]
fn bench_decode_rle_lib_fill_safe(b: &mut test::Bencher) {
    let mut test_vec = get_initial_vec();
    b.iter(|| {
        decode_rle_lib_fill_safe(&mut test_vec, RLE_FRAGMENT_SIZE, RLE_FILL_SIZE);
    });
    test::black_box(&test_vec);
    check_result(&test_vec);
}

#[bench]
fn bench_decode_rle_lib_fill_unsafe(b: &mut test::Bencher) {
    let mut test_vec = get_initial_vec();
    b.iter(|| {
        decode_rle_lib_fill_unsafe(&mut test_vec, RLE_FRAGMENT_SIZE, RLE_FILL_SIZE);
    });
    test::black_box(&test_vec);
    check_result(&test_vec);
}
