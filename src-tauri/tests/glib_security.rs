#![cfg(target_os = "linux")]

#[test]
fn optimized_variant_string_iteration_is_sound() {
    use glib_gtk3::prelude::ToVariant;
    let variant = vec!["first", "middle", "last"].to_variant();
    let mut iter = variant.array_iter_str().unwrap();
    assert_eq!(iter.next(), Some("first"));
    assert_eq!(iter.next_back(), Some("last"));
    assert_eq!(iter.last(), Some("middle"));
    assert_eq!(variant.array_iter_str().unwrap().nth(1), Some("middle"));
    assert_eq!(
        variant.array_iter_str().unwrap().nth_back(1),
        Some("middle")
    );
}
