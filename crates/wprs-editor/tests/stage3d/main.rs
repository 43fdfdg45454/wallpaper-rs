use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use std::sync::OnceLock;
use tempfile::TempDir;
use wprs_editor::Project;
use wprs_editor::actions::Action;
use wprs_editor::starters::starter;
use wprs_scene::{Forge, ForgeMaterial, ForgeShape};

mod edits;
mod flight;
mod looks;
mod motion;
mod views;

fn state() {
    static STATE: OnceLock<TempDir> = OnceLock::new();
    STATE.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("XDG_STATE_HOME", dir.path()) };
        dir
    });
}

pub fn act(project: &mut Project, action: Value) -> anyhow::Result<()> {
    let action = serde_json::from_value::<Action>(action).unwrap();
    assert!(wprs_editor::ACTIONS.contains(&action.name().as_str()), "{}", action.name());
    project.act(&action).map(drop)
}

pub fn edit(project: &mut Project, args: Value) -> anyhow::Result<()> {
    act(project, json!({ "scene3d": args }))
}

pub fn layer<'a>(p: &'a Project, id: &str) -> &'a Value {
    wprs_editor::actions::layer(&p.doc, id).unwrap().1
}

pub fn ask(p: &Project, q: Value) -> Value {
    wprs_editor::rigs::query(&p.dir, &p.doc, "", &q)
}

pub fn glb() -> Vec<u8> {
    let mut png = std::io::Cursor::new(Vec::new());
    RgbaImage::from_pixel(8, 8, Rgba([200, 40, 40, 255]))
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let mut paint = ForgeMaterial::new("paint", [1.0, 1.0, 1.0, 1.0]);
    paint.maps[0] = Some(png.into_inner());
    Forge::default()
        .material(paint)
        .material(ForgeMaterial::new("trim", [0.2, 0.2, 0.9, 1.0]))
        .part("body", ForgeShape::Cube, [0.0; 3], 2.0, 0)
        .part("knob", ForgeShape::Sphere(8), [0.0, 1.5, 0.0], 0.5, 1)
        .glb()
}

pub fn project(root: &TempDir) -> Project {
    state();
    let dir = root.path().join("toy");
    let mut project =
        Project::create(&dir, starter("particles", "org.example.toy", "Toy", "en").unwrap()).unwrap();
    std::fs::write(dir.join("toy.glb"), glb()).unwrap();
    RgbaImage::from_pixel(8, 4, Rgba([20, 90, 200, 255])).save(dir.join("sky.png")).unwrap();
    for (id, path) in [("toy", "toy.glb"), ("sky", "sky.png")] {
        let pointer = format!("/scene/assets/{id}");
        act(&mut project, json!({ "set": { "pointer": pointer, "value": { "path": path } } })).unwrap();
    }
    project
}

pub fn undone(p: &mut Project, before: &Value) {
    let after = p.doc.clone();
    act(p, json!({ "undo": {} })).unwrap();
    assert_eq!(&p.doc, before, "undo goes back");
    act(p, json!({ "redo": {} })).unwrap();
    assert_eq!(p.doc, after, "redo comes again");
}
