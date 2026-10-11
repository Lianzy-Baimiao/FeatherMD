use feathermd::viewport::Viewport;

#[test]
fn eof_fills_available_space_and_small_documents_stay_at_top() {
    assert_eq!(Viewport::new(&[20; 10], 65).bottom_top(), 7);
    assert_eq!(Viewport::new(&[20; 10], 60).bottom_top(), 7);
    assert_eq!(Viewport::new(&[20; 2], 65).bottom_top(), 0);
    assert_eq!(Viewport::new(&[100], 20).bottom_top(), 0);
    assert_eq!(Viewport::new(&[] as &[i32], 20).bottom_top(), 0);
}
#[test]
fn tall_headings_never_get_skipped_by_page_down() {
    let rows = [60, 60, 20, 70, 30, 40];
    let v = Viewport::new(&rows, 100);
    assert_eq!(v.visible_end(0), 2);
    assert_eq!(v.page_top(0, true), 1);
    assert_eq!(v.page_top(1, true), 3);
    assert_eq!(v.visible_end(1), 4);
    assert_eq!(v.page_top(3, false), 2);
    assert!(v.visible_end(v.page_top(3, false)) > 3);
}
#[test]
fn row_taller_than_viewport_still_allows_navigation() {
    let v = Viewport::new(&[200, 300, 400], 50);
    assert_eq!(v.page_top(0, true), 1);
    assert_eq!(v.page_top(2, false), 1);
    assert_eq!(v.visible_end(0), 1);
}
#[test]
fn all_zoom_sizes_and_mixed_rows_preserve_page_overlap() {
    for font in 12..=36 {
        for dpi in [96, 120, 144, 192] {
            let rows: Vec<i32> = (0..300)
                .map(|i| (font + if i % 3 == 0 { 30 } else { 10 }) * dpi / 96)
                .collect();
            for pixels in [250, 500, 1100] {
                let v = Viewport::new(&rows, pixels);
                for top in 1..250 {
                    let down = v.page_top(top, true);
                    assert!(down > top && down < v.visible_end(top));
                    let up = v.page_top(top, false);
                    assert!(up < top && v.visible_end(up) > top);
                }
                let bottom = v.bottom_top();
                assert!(rows[bottom..].iter().sum::<i32>() <= pixels);
                if bottom > 0 {
                    assert!(rows[bottom - 1..].iter().sum::<i32>() > pixels);
                }
            }
        }
    }
}
