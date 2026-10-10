use blake_bench::{fill_inputs, reference::blake2b256, verify_zero_copy, GpuBlake2b, Variant};

// BoringSSL 09f7078f953362d5c5afdd224118845327b60fb4,
// src/crypto/blake2/blake2b256_tests.txt, input bytes00..1f, lines138–139.
// https://boringssl.googlesource.com/boringssl/+/09f7078f953362d5c5afdd224118845327b60fb4/src/crypto/blake2/blake2b256_tests.txt
// Published generator: Go golang.org/x/crypto/blake2b.New256(nil).
//
// Copyright (c) 2021, Google Inc.
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY
// SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
// OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
// CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
const PUBLISHED: [u8; 32] = [
    0xcb, 0x2f, 0x51, 0x60, 0xfc, 0x1f, 0x7e, 0x05, 0xa5, 0x5e, 0xf4, 0x9d, 0x34, 0x0b, 0x48, 0xda,
    0x2e, 0x5a, 0x78, 0x09, 0x9d, 0x53, 0x39, 0x33, 0x51, 0xcd, 0x57, 0x9d, 0xd4, 0x25, 0x03, 0xd6,
];

#[test]
fn every_variant_matches_published_and_cpu_bytes_at_odd_and_partial_groups() {
    for variant in [
        Variant::V1Baseline,
        Variant::V2Unrolled,
        Variant::V3DualHash,
        Variant::V4DualHashFastRot,
    ] {
        let gpu = GpuBlake2b::open(variant).expect("real Metal pipeline required");
        for count in [1, 2, 3, 4, 63, 64, 65] {
            let mut batch = gpu
                .alloc_batch(count)
                .expect("real native backings required");
            assert_eq!(batch.count(), count);
            assert!(batch.inputs().unwrap().iter().all(|row| *row == [0; 32]));
            assert!(batch.outputs().unwrap().iter().all(|row| *row == [0; 32]));
            for seed in [0, 0xffff_ffff_1234_5678] {
                fill_inputs(&mut batch, seed).unwrap();
                batch.inputs_mut().unwrap()[0] = std::array::from_fn(|i| i as u8);
                let inputs = batch.inputs().unwrap().to_vec();
                let expected: Vec<_> = inputs.iter().map(|row| blake2b256(row)).collect();
                assert_eq!(expected[0], PUBLISHED);
                gpu.dispatch(&mut batch, 64).unwrap();
                assert_eq!(batch.inputs().unwrap(), inputs);
                assert_eq!(batch.outputs().unwrap()[0], PUBLISHED);
                assert_eq!(batch.outputs().unwrap(), expected);
                assert_eq!(verify_zero_copy(&batch).unwrap(), 0);
            }
        }
    }
}
