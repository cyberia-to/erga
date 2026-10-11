#![allow(dead_code)]
use mine_bench::MineVariant;
pub const MESSAGE: [u8; 32] = [
    0xa5, 0xa5, 0xa5, 0xa5, 0xde, 0xad, 0xbe, 0xef, 0xca, 0xfe, 0xba, 0xbe, 0x12, 0x34, 0x56, 0x78,
    0x9a, 0xbc, 0xde, 0xf0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc,
];
pub fn hex(text: &str) -> [u8; 32] {
    assert_eq!(text.len(), 64);
    std::array::from_fn(|i| u8::from_str_radix(&text[2 * i..2 * i + 2], 16).unwrap())
}
pub struct Case {
    pub rows: u64,
    pub kind: &'static str,
    pub base: u64,
    pub count: u32,
    pub output: [u8; 32],
}
pub fn cases() -> Vec<Case> {
    include_str!("../fixtures/batches.txt")
        .lines()
        .map(|line| {
            let v: Vec<_> = line.split_whitespace().collect();
            assert_eq!(v.len(), 5);
            Case {
                rows: v[0].parse().unwrap(),
                kind: v[1],
                base: v[2].parse().unwrap(),
                count: v[3].parse().unwrap(),
                output: hex(v[4]),
            }
        })
        .collect()
}
pub fn expected_kind(variant: MineVariant) -> &'static str {
    match variant {
        MineVariant::V6NoBlake => "v6",
        MineVariant::V7NoLoads => "v7",
        MineVariant::V8DualTable => "v8-distinct",
        _ => "reference",
    }
}
