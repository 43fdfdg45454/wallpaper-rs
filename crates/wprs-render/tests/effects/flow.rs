use super::*;

#[test]
fn flow_moves_along_its_direction_or_map() {
    let Some(right) = Fx::new("flow", json!({ "strength": 16 })).input(halves(RED, BLUE)).shot() else {
        return;
    };
    right.check(&[((10, 16), RED), ((36, 16), RED), ((44, 16), BLUE)]);
    let left = Fx::new("flow", json!({ "strength": 16, "map": "west" })).input(halves(RED, BLUE)).image(
        "west",
        solid([0, 128, 0]),
        true,
    );
    let Some(left) = left.shot() else { return };
    left.check(&[((26, 16), BLUE), ((20, 16), RED), ((44, 16), BLUE)]);
}
