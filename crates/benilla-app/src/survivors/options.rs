//! Survivors preferences and runtime bindings; audio/video use the engine's persisted CVars.
use bevy::prelude::*;
mod ui;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub(super) enum Action {
    Forward,
    Back,
    TurnLeft,
    TurnRight,
    StrafeLeft,
    StrafeRight,
    Jump,
    Mobility,
    Talents,
    Grade,
    Card1,
    Card2,
    Card3,
    Pause,
    Character,
    DamageMeter,
    Inventory,
}
impl Action {
    pub const ALL: [Self; 17] = [
        Self::Forward,
        Self::Back,
        Self::TurnLeft,
        Self::TurnRight,
        Self::StrafeLeft,
        Self::StrafeRight,
        Self::Jump,
        Self::Mobility,
        Self::Talents,
        Self::Grade,
        Self::Card1,
        Self::Card2,
        Self::Card3,
        Self::Pause,
        Self::Character,
        Self::DamageMeter,
        Self::Inventory,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Forward => "Avancer",
            Self::Back => "Reculer",
            Self::TurnLeft => "Tourner à gauche",
            Self::TurnRight => "Tourner à droite",
            Self::StrafeLeft => "Pas à gauche",
            Self::StrafeRight => "Pas à droite",
            Self::Jump => "Sauter",
            Self::Mobility => "Compétence de déplacement",
            Self::Talents => "Talents",
            Self::Grade => "Filtre couleur et netteté",
            Self::Card1 => "Choisir la carte 1",
            Self::Card2 => "Choisir la carte 2",
            Self::Card3 => "Choisir la carte 3",
            Self::Pause => "Pause / reprendre",
            Self::Character => "Personnage",
            Self::DamageMeter => "Compteur de combat",
            Self::Inventory => "Sacs / inventaire",
        }
    }
}

const KEY_CODES: &[KeyCode] = &[
    KeyCode::KeyA,
    KeyCode::KeyB,
    KeyCode::KeyC,
    KeyCode::KeyD,
    KeyCode::KeyE,
    KeyCode::KeyF,
    KeyCode::KeyG,
    KeyCode::KeyH,
    KeyCode::KeyI,
    KeyCode::KeyJ,
    KeyCode::KeyK,
    KeyCode::KeyL,
    KeyCode::KeyM,
    KeyCode::KeyN,
    KeyCode::KeyO,
    KeyCode::KeyP,
    KeyCode::KeyQ,
    KeyCode::KeyR,
    KeyCode::KeyS,
    KeyCode::KeyT,
    KeyCode::KeyU,
    KeyCode::KeyV,
    KeyCode::KeyW,
    KeyCode::KeyX,
    KeyCode::KeyY,
    KeyCode::KeyZ,
    KeyCode::Digit0,
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
    KeyCode::Space,
    KeyCode::Tab,
    KeyCode::Enter,
    KeyCode::Escape,
    KeyCode::Backspace,
    KeyCode::Delete,
    KeyCode::Insert,
    KeyCode::Home,
    KeyCode::End,
    KeyCode::PageUp,
    KeyCode::PageDown,
    KeyCode::ArrowUp,
    KeyCode::ArrowDown,
    KeyCode::ArrowLeft,
    KeyCode::ArrowRight,
    KeyCode::ShiftLeft,
    KeyCode::ShiftRight,
    KeyCode::ControlLeft,
    KeyCode::ControlRight,
    KeyCode::AltLeft,
    KeyCode::AltRight,
    KeyCode::F1,
    KeyCode::F2,
    KeyCode::F3,
    KeyCode::F4,
    KeyCode::F5,
    KeyCode::F6,
    KeyCode::F7,
    KeyCode::F8,
    KeyCode::F9,
    KeyCode::F10,
    KeyCode::F11,
    KeyCode::F12,
];

#[derive(Resource, Clone, Debug, PartialEq)]
pub(super) struct Settings {
    pub bindings: [[Option<KeyCode>; 2]; 17],
    pub damage: bool,
    pub healing: bool,
    pub incoming: bool,
    pub critical_only: bool,
    pub grade: bool,
    pub invert_x: bool,
    pub invert_y: bool,
    pub zoom_speed: f32,
}
impl Settings {
    pub fn defaults() -> Self {
        let primary = [
            KeyCode::KeyW,
            KeyCode::KeyS,
            KeyCode::KeyA,
            KeyCode::KeyD,
            KeyCode::KeyQ,
            KeyCode::KeyE,
            KeyCode::Space,
            KeyCode::KeyF,
            KeyCode::KeyT,
            KeyCode::KeyG,
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Escape,
            KeyCode::KeyC,
            KeyCode::F6,
            KeyCode::KeyB,
        ];
        let mut bindings = primary.map(|key| [Some(key), None]);
        for (action, key) in [
            (Action::Forward, KeyCode::ArrowUp),
            (Action::Back, KeyCode::ArrowDown),
            (Action::TurnLeft, KeyCode::ArrowLeft),
            (Action::TurnRight, KeyCode::ArrowRight),
        ] {
            bindings[action as usize][1] = Some(key);
        }
        Self {
            bindings,
            damage: true,
            healing: true,
            incoming: true,
            critical_only: false,
            grade: true,
            invert_x: false,
            invert_y: false,
            zoom_speed: 1.0,
        }
    }
    pub fn pressed(&self, action: Action, keys: &ButtonInput<KeyCode>) -> bool {
        self.bindings[action as usize]
            .iter()
            .flatten()
            .any(|&key| keys.pressed(key))
    }
    pub fn just_pressed(&self, action: Action, keys: &ButtonInput<KeyCode>) -> bool {
        self.bindings[action as usize]
            .iter()
            .flatten()
            .any(|&key| keys.just_pressed(key))
    }
    pub fn rebind(&mut self, action: Action, slot: usize, key: Option<KeyCode>) {
        let old = self.bindings[action as usize][slot];
        if let Some(key) = key {
            for row in &mut self.bindings {
                for value in row {
                    if *value == Some(key) {
                        *value = old;
                    }
                }
            }
        }
        self.bindings[action as usize][slot] = key;
    }
    pub fn numbers(&self, healing: bool, incoming: bool, critical: bool) -> bool {
        if healing {
            self.healing
        } else if incoming {
            self.incoming
        } else {
            self.damage && (!self.critical_only || critical)
        }
    }
    fn encode(&self) -> String {
        let mut out = format!(
            "v1\ndamage {}\nhealing {}\nincoming {}\ncritical_only {}\ngrade {}\n",
            self.damage, self.healing, self.incoming, self.critical_only, self.grade
        );
        for (i, row) in self.bindings.iter().enumerate() {
            for (slot, key) in row.iter().enumerate() {
                out.push_str(&format!(
                    "bind.{i}.{slot} {}\n",
                    key.map_or("None".to_owned(), |k| format!("{k:?}"))
                ));
            }
        }
        out.push_str(&format!(
            "invert_x {}\ninvert_y {}\nzoom_speed {}\n",
            self.invert_x, self.invert_y, self.zoom_speed
        ));
        out
    }
    fn decode(text: &str) -> Self {
        let mut out = Self::defaults();
        if text.lines().next() != Some("v1") {
            return out;
        }
        for line in text.lines().skip(1) {
            let Some((name, value)) = line.split_once(' ') else {
                continue;
            };
            let flag = value.parse::<bool>().ok();
            match name {
                "invert_x" => {
                    if let Some(v) = flag {
                        out.invert_x = v;
                    }
                }
                "invert_y" => {
                    if let Some(v) = flag {
                        out.invert_y = v;
                    }
                }
                "zoom_speed" => {
                    if let Ok(v) = value.parse::<f32>() {
                        if v.is_finite() {
                            out.zoom_speed = v.clamp(0.25, 2.0);
                        }
                    }
                }
                "damage" => {
                    if let Some(v) = flag {
                        out.damage = v;
                    }
                }
                "healing" => {
                    if let Some(v) = flag {
                        out.healing = v;
                    }
                }
                "incoming" => {
                    if let Some(v) = flag {
                        out.incoming = v;
                    }
                }
                "critical_only" => {
                    if let Some(v) = flag {
                        out.critical_only = v;
                    }
                }
                "grade" => {
                    if let Some(v) = flag {
                        out.grade = v;
                    }
                }
                _ => {
                    let parts: Vec<_> = name.split('.').collect();
                    if parts.len() != 3 || parts[0] != "bind" {
                        continue;
                    }
                    let (Ok(action), Ok(slot)) =
                        (parts[1].parse::<usize>(), parts[2].parse::<usize>())
                    else {
                        continue;
                    };
                    if action >= Action::ALL.len() || slot >= 2 {
                        continue;
                    }
                    if value == "None" {
                        out.bindings[action][slot] = None;
                    } else if let Some(&key) =
                        KEY_CODES.iter().find(|key| format!("{key:?}") == value)
                    {
                        out.bindings[action][slot] = Some(key);
                    }
                }
            }
        }
        // A malformed file cannot assign the same key to several actions.
        let mut seen = Vec::new();
        for row in &mut out.bindings {
            for key in row {
                if let Some(value) = *key {
                    if seen.contains(&value) {
                        *key = None;
                    } else {
                        seen.push(value);
                    }
                }
            }
        }
        out
    }
    pub fn save(&self) {
        if let Some(home) = crate::local_state::home() {
            if let Err(error) = crate::local_state::write_atomic(
                &home.join("survivors-options.txt"),
                &self.encode(),
            ) {
                warn!("Could not save Survivors options: {error}");
            }
        }
    }
}
impl Default for Settings {
    fn default() -> Self {
        let text = crate::local_state::home().and_then(|h| {
            crate::local_state::read_to_string(&h.join("survivors-options.txt")).ok()
        });
        let mut settings = text
            .as_deref()
            .map(Self::decode)
            .unwrap_or_else(Self::defaults);
        if crate::webenv::var("WOW_GRADE").as_deref() == Some("0") {
            settings.grade = false;
        }
        settings
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum Tab {
    #[default]
    Controls,
    Sound,
    Mouse,
    Display,
    Video,
    Help,
}
#[derive(Resource, Default)]
pub(super) struct Panel {
    pub open: bool,
    tab: Tab,
    capture: Option<(Action, usize)>,
    scroll: f32,
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Settings>()
        .init_resource::<Panel>()
        .add_message::<bevy::input::mouse::MouseWheel>()
        .add_systems(
            Update,
            (
                ui::input.before(super::hud::keys),
                ui::render.after(ui::input),
                ui::scroll.after(ui::input).before(ui::render),
            ),
        );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bindings_swap_conflicts_and_are_used_for_input() {
        let mut s = Settings::defaults();
        s.rebind(Action::Forward, 0, Some(KeyCode::KeyG));
        assert_eq!(s.bindings[Action::Grade as usize][0], Some(KeyCode::KeyW));
        let mut keys = ButtonInput::default();
        keys.press(KeyCode::KeyG);
        assert!(s.pressed(Action::Forward, &keys));
        assert!(!s.pressed(Action::Grade, &keys));
        s.rebind(Action::Forward, 0, None);
        assert!(!s.pressed(Action::Forward, &keys));
    }
    #[test]
    fn preferences_and_unbound_secondary_keys_roundtrip() {
        let mut s = Settings::defaults();
        s.rebind(Action::Forward, 0, Some(KeyCode::KeyZ));
        s.rebind(Action::Back, 1, None);
        s.damage = false;
        s.critical_only = true;
        s.invert_y = true;
        s.zoom_speed = 1.5;
        assert_eq!(Settings::decode(&s.encode()), s);
        assert_eq!(Settings::decode("broken"), Settings::defaults());
        assert_eq!(Settings::decode("v1\nzoom_speed NaN\n").zoom_speed, 1.0);
        assert_eq!(Settings::decode("v1\nzoom_speed 100\n").zoom_speed, 2.0);
    }
    #[test]
    fn combat_number_filters_are_independent() {
        let mut s = Settings::defaults();
        s.critical_only = true;
        assert!(!s.numbers(false, false, false));
        assert!(s.numbers(false, false, true));
        assert!(s.numbers(true, false, false));
        s.healing = false;
        s.incoming = false;
        assert!(!s.numbers(true, false, true));
        assert!(!s.numbers(false, true, true));
    }
}
