use feathermd::scrollbar::ScrollTrack;

#[test]
fn huge_files_reach_both_endpoints_without_truncation() {
    for maximum in [100_u64 * 1024 * 1024 * 1024, u64::MAX] {
        let track = ScrollTrack::new(100, 600, maximum, 4096, maximum / 2, 32);
        assert_eq!(track.thumb_length, 32);
        assert_eq!(track.value_at(100, 0), 0);
        assert_eq!(track.value_at(700 - 32, 0), maximum);
        assert_eq!(track.value_at(i32::MAX, 16), maximum);
        assert_eq!(track.value_at(i32::MIN, 16), 0);
        let middle = track.value_at(track.thumb_start + 16, 16);
        assert!(middle.abs_diff(maximum / 2) <= maximum / 568);
    }
}

#[test]
fn short_tracks_and_empty_content_have_valid_geometry() {
    let empty = ScrollTrack::new(10, 20, 0, 100, 0, 32);
    assert_eq!(empty.thumb_length, 20);
    assert_eq!(empty.value_at(1000, 0), 0);
    let short = ScrollTrack::new(10, 8, 800, 10, 300, 32);
    assert_eq!(short.thumb_start, 10);
    assert_eq!(short.thumb_length, 8);
}

#[test]
fn grabbing_inside_a_thumb_preserves_position_and_pixel_navigation() {
    let track = ScrollTrack::new(30, 600, 900, 300, 450, 24);
    assert_eq!(track.thumb_length, 150);
    assert_eq!(track.value_at(track.thumb_start + 47, 47), 450);
    assert_eq!(track.value_at(track.thumb_start + 97, 47), 550);
}
