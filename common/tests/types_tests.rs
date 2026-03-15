use common::types::{BoardSize, DurationMs, Score, TimestampMs};

#[test]
fn score_saturates_on_sub() {
    let mut score = Score::from(10);
    score -= Score::from(25);
    assert_eq!(score, Score::zero());
}

#[test]
fn score_saturates_on_add() {
    let mut score = Score::from(u64::MAX - 1);
    score += Score::from(10);
    assert_eq!(score, Score::from(u64::MAX));
}

#[test]
fn board_size_helpers_handle_odd_sizes() {
    let size = BoardSize::from(41);
    assert_eq!(size.half(), 20);
    assert_eq!(size.limit_pos(), 21);
}

#[test]
fn board_size_clamps_to_minimum() {
    let size = BoardSize::from(0);
    assert_eq!(size.as_i32(), 1);
}

#[test]
fn duration_and_timestamp_math_roundtrip() {
    let start = TimestampMs::from_millis(1_000);
    let end = TimestampMs::from_millis(2_500);
    let diff = end - start;
    assert_eq!(diff.as_i64(), 1_500);

    let shifted = start + DurationMs::from_millis(500);
    assert_eq!(shifted.as_i64(), 1_500);
}
