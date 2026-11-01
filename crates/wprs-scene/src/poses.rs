use crate::figure::{Attach, MAX_FIGURES, Space};
use crate::lens::MAX_PATHS;
use crate::validate::{Check, Issue, nested};
use crate::{Kind, Layer, MAX_KEYS};
use std::collections::HashMap;

impl Check<'_> {
    pub(crate) fn figures(&mut self) {
        let scene = self.scene;
        let figures = scene.layers().filter(|l| l.figure().is_some()).count();
        if figures > MAX_FIGURES {
            self.issues.push(Issue::Figures(figures));
        }
        let mut paths = Vec::new();
        nested("/layers", &scene.layers, &mut paths);
        let mut deep: HashMap<&str, bool> = HashMap::new();
        for l in scene.layers() {
            let inside = deep.get(l.id.as_str()).copied().unwrap_or(false);
            let space = matches!(l.kind, Kind::Group { space: Some(Space::Depth), .. });
            self.uses(space, "model3d");
            deep.extend(l.children().iter().map(|c| (c.id.as_str(), inside || space)));
        }
        for (path, id) in &paths {
            let Some(l) = scene.layer(id) else { continue };
            if l.depth_test && !deep.get(id).copied().unwrap_or(false) {
                self.issues.push(Issue::Flat(format!("{path}/depth_test")));
            }
            if let Some(a) = &l.attach {
                self.attach(path, l, a);
            }
        }
        if let Some(wind) = &scene.canvas.wind {
            self.uses(true, "bone_physics");
            self.value("/canvas/wind", wind);
        }
        if let Some(g) = scene.canvas.gusts {
            self.uses(true, "bone_physics");
            self.range("/canvas/gusts", g, 0.0, 1.0);
        }
        let Some(cam) = &scene.camera3d else { return };
        self.uses(true, "camera3d");
        self.number("/camera3d/fov", &cam.fov, 1.0, 179.0);
        self.range("/camera3d/near", cam.near, 0.001, 1000.0);
        self.range("/camera3d/far", cam.far, cam.near.next_up(), 1e6);
        self.value("/camera3d/eye", &cam.eye);
        self.value("/camera3d/center", &cam.center);
        if cam.up.0 == [0.0; 3] {
            self.issues.push(Issue::Type("/camera3d/up".into()));
        }
        cam.fade.iter().for_each(|f| self.range("/camera3d/fade/duration", f.duration, 0.0, 10.0));
        self.count("/camera3d/paths", cam.paths.len(), MAX_PATHS);
        for (i, path) in cam.paths.iter().enumerate() {
            let q = format!("/camera3d/paths/{i}");
            self.id(&format!("{q}/id"), &path.id);
            if cam.paths[..i].iter().any(|o| o.id == path.id) {
                self.issues.push(Issue::DuplicateId(format!("{q}/id")));
            }
            self.range(&format!("{q}/keys"), path.keys.len() as f32, 2.0, MAX_KEYS as f32);
            self.keys_sorted(&format!("{q}/keys"), path.keys.iter().map(|k| k.t));
            for (k, key) in path.keys.iter().enumerate() {
                key.fov.iter().for_each(|f| self.range(&format!("{q}/keys/{k}/fov"), *f, 1.0, 179.0));
                for v in [key.eye, key.center].into_iter().chain(key.up).flat_map(|v| v.0) {
                    self.range(&format!("{q}/keys/{k}"), v, -1e6, 1e6);
                }
            }
        }
    }

    fn attach(&mut self, path: &str, l: &Layer, a: &Attach) {
        self.uses(true, "attach");
        let scene = self.scene;
        let target = scene.layer(&a.layer).filter(|t| t.id != l.id && t.figure().is_some());
        self.known(&format!("{path}/attach/layer"), "mesh or model layer", &a.layer, target.is_some());
        if l.parent.is_some() || a.bone.is_some() == a.point.is_some() {
            self.issues.push(Issue::Type(format!("{path}/attach")));
        }
        let model = target.and_then(|t| scene.models.get(t.figure()?));
        let names =
            [("bone", &a.bone, model.map(|m| &m.bones)), ("point", &a.point, model.map(|m| &m.nodes))];
        for (field, name, known) in names {
            let Some(name) = name else { continue };
            if known.is_some_and(|k| !k.contains(name)) {
                self.issues.push(Issue::Missing(format!("{path}/attach/{field}"), name.clone()));
            }
        }
        a.offset
            .iter()
            .for_each(|o| o.iter().for_each(|v| self.range(&format!("{path}/attach/offset"), *v, -1e5, 1e5)));
        let inherit = a.inherit.as_deref().unwrap_or_default();
        if inherit.iter().enumerate().any(|(i, f)| inherit[..i].contains(f)) {
            self.issues.push(Issue::Type(format!("{path}/attach/inherit")));
        }
        if let Some(t) = target
            && std::iter::successors(Some(t), |at| at.attach.as_ref().and_then(|x| scene.layer(&x.layer)))
                .skip(1)
                .take(crate::MAX_DEPTH + 1)
                .any(|at| at.id == l.id)
        {
            self.issues.push(Issue::Circle(format!("{path}/attach")));
        }
    }
}
