use crate::choices::{flipped, toggled};

const RATIOS: &str = "16x9,16x10,21x9,landscape";

#[test]
fn lists_keep_the_order_of_the_choices_and_the_others_last() {
    assert_eq!(toggled("", "21x9", RATIOS), "21x9");
    assert_eq!(toggled("21x9", "16x9", RATIOS), "16x9,21x9");
    assert_eq!(toggled("16x9,21x9", "16x9", RATIOS), "21x9");
    assert_eq!(toggled("16x9,43x18", "landscape", RATIOS), "16x9,landscape,43x18");
    assert_eq!(toggled("16x9, 43x18", "43x18", RATIOS), "16x9");
    assert_eq!(toggled("16x9", " ", RATIOS), "16x9");
}

#[test]
fn flags_flip_one_digit() {
    assert_eq!(flipped("100", 1), "110");
    assert_eq!(flipped("111", 0), "011");
    assert_eq!(flipped("100", 7), "100");
}
