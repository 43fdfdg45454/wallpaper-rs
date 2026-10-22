use super::*;

#[test]
fn pixelate_holds_one_color_per_cell() {
    let Some(square) = Fx::new("pixelate", json!({ "size": 8 })).shot() else { return };
    assert_eq!(square.at(1, 16), square.at(6, 16));
    assert_ne!(square.at(7, 16), square.at(8, 16));
    assert!((0..H).all(|y| square.at(10, y) == square.at(10, 0)));
    let Some(hex) = Fx::new("pixelate", json!({ "size": 8, "hex": true })).shot() else { return };
    assert!((0..H).any(|y| hex.at(10, y) != hex.at(10, 0)));
    assert_eq!(hex.at(1, 1), hex.at(2, 2));
}
