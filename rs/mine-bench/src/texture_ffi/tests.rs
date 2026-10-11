use super::*;
#[test]
fn texture_geometry_matches_fixed_shader_coordinates() {
    assert_eq!(geometry(8192, 262144).unwrap(), 1);
    assert_eq!(geometry(16384, 524288).unwrap(), 2);
    assert_eq!(geometry(1 << 27, 1 << 32).unwrap(), 16384);
    for (rows, bytes) in [
        (0, 0),
        (1, 32),
        (8191, 8191 * 32),
        (8193, 8193 * 32),
        (1 << 28, 1 << 33),
        (8192, 262143),
        (8192, 262145),
    ] {
        assert!(geometry(rows, bytes).is_err());
    }
}
