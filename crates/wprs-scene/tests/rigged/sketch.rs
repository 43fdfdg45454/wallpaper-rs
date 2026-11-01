use wprs_scene::{Cut, Rigged, Sketch, SketchBone};

fn disc(side: u32) -> Vec<u8> {
    let r = side as f32 / 2.0;
    (0..side * side)
        .map(|i| {
            let (x, y) = ((i % side) as f32 + 0.5 - r, (i / side) as f32 + 0.5 - r);
            if x * x + y * y < (r * 0.8).powi(2) { 255 } else { 0 }
        })
        .collect()
}

#[test]
fn a_grid_covers_the_picture_and_skips_empty_cells() {
    let full = Sketch::cut(&vec![255; 64 * 32], [64, 32], Cut::Grid, 4);
    assert_eq!((full.points.len(), full.triangles.len()), (15, 16));
    assert!(full.points.iter().all(|p| (0.0..=64.0).contains(&p[0]) && (0.0..=32.0).contains(&p[1])));
    let round = Sketch::cut(&disc(64), [64, 64], Cut::Grid, 8);
    assert!(round.triangles.len() < 2 * 64, "the corners are left out");
}

#[test]
fn an_outline_follows_the_shape_of_the_picture() {
    let sketch = Sketch::cut(&disc(64), [64, 64], Cut::Outline, 8);
    assert!(sketch.triangles.len() > 8);
    for t in &sketch.triangles {
        let c = (0..2).map(|k| t.iter().map(|i| sketch.points[*i as usize][k]).sum::<f32>() / 3.0 - 32.0);
        let c: Vec<f32> = c.collect();
        assert!((c[0] * c[0] + c[1] * c[1]).sqrt() < 26.5, "{c:?}");
    }
    let empty = Sketch::cut(&[0; 16], [4, 4], Cut::Outline, 2);
    assert_eq!(empty.triangles.len(), 8, "an empty picture falls back to a full grid");
}

#[test]
fn a_sketch_with_bones_writes_a_mesh_that_reads_back() {
    let mut sketch = Sketch::cut(&vec![255; 64 * 64], [64, 64], Cut::Grid, 4);
    sketch.bones = vec![
        SketchBone { name: "root".into(), parent: None, at: [8.0, 32.0] },
        SketchBone { name: "arm".into(), parent: Some(0), at: [32.0, 32.0] },
    ];
    sketch.bind();
    assert!(sketch.weights.iter().all(|w| (w.iter().map(|x| x.1).sum::<f32>() - 1.0).abs() < 1e-4));
    let rig = Rigged::decode(&sketch.glb()).unwrap();
    assert_eq!((rig.vertex_count(), rig.palette.len(), rig.parts.clone()), (25, 2, vec!["body".to_string()]));
    let back = Sketch::from_rig(&rig, [64.0, 64.0]).unwrap();
    assert_eq!(back.bones, sketch.bones);
    assert_eq!(back.triangles, sketch.triangles);
    assert!(
        back.points
            .iter()
            .zip(&sketch.points)
            .all(|(a, b)| (a[0] - b[0]).abs() < 1e-4 && (a[1] - b[1]).abs() < 1e-4)
    );
    let plain = Sketch::cut(&[255; 16], [4, 4], Cut::Grid, 2);
    let rig = Rigged::decode(&plain.glb()).unwrap();
    assert_eq!((rig.palette.len(), Sketch::from_rig(&rig, [4.0, 4.0]).unwrap().bones.len()), (1, 0));
}
