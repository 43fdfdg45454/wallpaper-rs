use super::*;

#[test]
fn chroma_key_removes_the_key_color() {
    let green = [0, 255, 0];
    let Some(out) = Fx::new("chroma_key", json!({})).input(halves(green, RED)).shot() else { return };
    out.check(&[((10, 16), [0; 3]), ((54, 16), RED)]);
    let blue = Fx::new("chroma_key", json!({ "key": "#0000ff" })).input(halves(green, BLUE));
    let Some(out) = blue.shot() else { return };
    out.check(&[((10, 16), green), ((54, 16), [0; 3])]);
}
