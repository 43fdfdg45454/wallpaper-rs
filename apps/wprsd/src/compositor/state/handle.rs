use super::super::Command;
use super::super::scene::SceneState;
use super::State;

impl State {
    pub(super) fn handle(&mut self, command: Command) -> bool {
        match command {
            Command::Configure(output, size, spec) => self.configure(output, size, spec),
            Command::Remove(output) => {
                super::super::feed::declare(&output, None);
                drop((self.outputs.remove(&output), self.windows.remove(&output)));
                self.pictures.prune(&self.outputs);
                self.follow(&output);
            }
            Command::Attach(output, window) => self.attach(output, window),
            Command::Frame(output) => self.frame_done(&output),
            Command::Repaint(output) => self.repaint(&output),
            Command::Region(output) => self.region(&output),
            Command::Backdrop(output) => self.backdrop_again(&output),
            Command::SetActive(output, active, covered) => {
                self.outputs.get_mut(&output).into_iter().for_each(|o| o.covered = covered);
                if let Some(o) = self.outputs.get_mut(&output).filter(|o| o.active != active) {
                    o.active = active;
                    o.shown.scene.iter_mut().filter(|_| !active).for_each(SceneState::pause);
                    let cancelled = !active && o.fade.take().is_some();
                    o.dirty |= cancelled || (active && o.animated());
                }
            }
            Command::Settings(settings) => self.apply_settings(settings),
            Command::Calm(calm) => self.calm(calm),
            Command::Power(on_battery) => {
                self.power.on_battery = on_battery;
                self.outputs.values_mut().filter(|o| o.shown.scene.is_some()).for_each(|o| o.dirty = true);
            }
            Command::Saver(saver) => {
                self.power.saver = saver;
                self.outputs.values_mut().filter(|o| o.shown.scene.is_some()).for_each(|o| o.dirty = true);
            }
            Command::Locked(locked) => {
                self.locked = locked;
                self.hush();
            }
            Command::Press(output, id) => {
                if let Some(o) = self.outputs.get_mut(&output)
                    && let Some(scene) = o.shown.scene.as_mut()
                {
                    scene.press(&id);
                    o.dirty = true;
                }
            }
            Command::Retry => {
                super::super::stops::retry();
                for o in self.outputs.values_mut() {
                    o.shown.scene.iter_mut().for_each(SceneState::retry);
                    o.dirty |= o.shown.scene.is_some();
                }
            }
            Command::Wake => {}
            Command::Quit => return false,
        }
        true
    }
}
