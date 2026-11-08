//! Versioned wire values. Builds come from trusted clients; Bevy entity IDs stay local.
use serde::{Deserialize, Serialize};

pub const VERSION: u32 = 18;
pub const MAX_PLAYERS: usize = 8;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub name: String,
    pub class: usize,
    pub display: u32,
    pub race: u8,
    pub sex: u8,
    pub save: Option<ClientSave>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClientSave {
    pub talent_xp: u64,
    pub talents: [[u8; 19]; 3],
    pub inventory: String,
    pub look: Option<SavedLook>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedLook {
    pub display: u32,
    pub fields: Vec<(u16, u32)>,
    pub equipment: Vec<(u32, Appearance)>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub sequence: u64,
    pub axes: [f32; 2],
    pub turn: f32,
    pub facing: Option<[f32; 3]>,
    pub aim: Option<[f32; 3]>,
    pub jump: bool,
    pub mobility: bool,
    pub active: bool,
}

impl Input {
    pub fn valid(&self) -> bool {
        self.axes.iter().all(|v| v.is_finite() && v.abs() <= 1.0)
            && self.turn.is_finite()
            && self.turn.abs() <= 1.0
            && self
                .facing
                .iter()
                .chain(self.aim.iter())
                .flatten()
                .all(|v| v.is_finite() && v.abs() < 100_000.0)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ToServer {
    ResetMeter,
    Chest(super::super::shared_chest::Request),
    Join { version: u32, profile: Profile },
    Input(Input),
    Pick { offer: u64, index: usize },
    Save(ClientSave),
    LootAck(u64),
    Ping,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Unit {
    #[serde(default)]
    pub rank: Option<super::super::enemies::Rank>,
    #[serde(default)]
    pub party_player: bool,
    #[serde(default)]
    pub static_pickup: bool,
    pub status: Option<super::super::unit_frames::Status>,
    pub guid: u64,
    pub owner: Option<u64>,
    pub kind: u8,
    pub display: Option<u32>,
    pub scale: f32,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub speed: f32,
    pub vertical_speed: f32,
    pub flags: u32,
    pub fields: Vec<(u16, u32)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Effect {
    Number {
        #[serde(default)]
        owner: Option<u64>,
        target: u64,
        amount: f32,
        crit: bool,
        color: u32,
    },
    Cast {
        caster: u64,
        spell: u32,
        targets: Vec<u64>,
        dest: Option<[f32; 3]>,
    },
    Impact {
        target: u64,
        spell: u32,
    },
    Swing {
        attacker: u64,
        victim: Option<u64>,
        damage: u32,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Stats {
    #[serde(default)]
    pub auras: Vec<super::super::player_auras::Aura>,
    #[serde(default)]
    pub healing_meter: super::super::combat_ui::Meter,
    #[serde(default)]
    pub taken_meter: super::super::combat_ui::Meter,
    pub movement_speed: f32,
    pub meter: super::super::combat_ui::Meter,
    pub earned: u64,
    pub hp: f32,
    pub max_hp: f32,
    pub elapsed: f32,
    pub level: u32,
    #[serde(default)]
    pub character_level: u32,
    pub xp: f32,
    pub kills: u32,
    pub dead: bool,
    #[serde(default)]
    pub revival: [f32; 2],
    #[serde(default)]
    pub coop_revive: bool,
    pub won: bool,
    pub abilities: Vec<(String, u8, f32)>,
    pub passives: Vec<(String, u64)>,
    #[serde(default)]
    pub draft_uses: [u8; 3],
    pub offer_id: u64,
    pub choices: Vec<Choice>,
    pub mobility: [f32; 4],
    pub shield: [f32; 2],
    pub immune: f32,
    #[serde(default)]
    pub support_bonuses: [f32; 6],
    pub damage_done: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Choice {
    Numeric(String, u8),
    Spell(String),
    Upgrade(usize),
    Passive(String),
    PassiveUpgrade(usize),
    Heal,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    #[serde(default)]
    pub boss_warnings: Vec<super::super::boss_spells::Warning>,
    /// Cumulative copper earned on this connection; duplicate snapshots cannot pay twice.
    pub money_earned: u64,
    #[serde(default)]
    pub chest: super::super::shared_chest::Replica,
    #[serde(default)]
    pub party: Vec<super::super::combat_ui::PartyRow>,
    pub run_id: u64,
    pub loot: Vec<(u64, String)>,
    pub tick: u64,
    pub acknowledged: u64,
    pub phase: u8,
    pub map: usize,
    pub in_hub: bool,
    pub stats: Stats,
    pub units: Vec<Unit>,
    pub effects: Vec<Effect>,
    pub equipment: Vec<(u32, Appearance)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Appearance {
    pub display_id: u32,
    pub inventory_type: u32,
    pub item_class: u32,
    pub subclass: u32,
    pub material: u32,
    pub sheath: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ToClient {
    Pong,
    Welcome { version: u32, id: u64 },
    Snapshot(Snapshot),
    Error(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_nonfinite_and_excessive_movement() {
        let mut input = Input::default();
        assert!(input.valid());
        input.axes[0] = 2.0;
        assert!(!input.valid());
        input.axes[0] = f32::NAN;
        assert!(!input.valid());
        input.axes[0] = 0.0;
        input.aim = Some([f32::INFINITY, 0.0, 0.0]);
        assert!(!input.valid());
    }

    #[test]
    fn input_round_trips_without_an_identity_or_position_to_spoof() {
        let message = ToServer::Input(Input {
            sequence: 8,
            jump: true,
            ..Default::default()
        });
        let encoded = serde_json::to_vec(&message).unwrap();
        let ToServer::Input(decoded) = serde_json::from_slice(&encoded).unwrap() else {
            panic!()
        };
        assert_eq!(decoded.sequence, 8);
        assert!(decoded.jump);
        assert!(decoded.valid());
    }

    #[test]
    fn target_status_roundtrips_with_real_health_and_debuff_durations() {
        let unit = Unit {
            rank: Some(super::super::super::enemies::Rank::Healer),
            party_player: false,
            static_pickup: true,
            status: Some(super::super::super::unit_frames::Status {
                name: "Training Boss".into(),
                health: 1234.0,
                maximum: 20000.0,
                level: 63,
                friendly: false,
                debuffs: vec![(589, 4.5), (339, 2.0)],
                debuff_stacks: vec![(1943, 3), (2818, 2), (133, 1)],
            }),
            guid: 42,
            owner: None,
            kind: 1,
            display: Some(16174),
            scale: 2.0,
            position: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            speed: 0.0,
            vertical_speed: 0.0,
            flags: 0,
            fields: vec![],
        };
        let decoded: Unit = serde_json::from_slice(&serde_json::to_vec(&unit).unwrap()).unwrap();
        assert!(decoded.static_pickup);
        assert_eq!(decoded.rank, unit.rank);
        let status = decoded.status.unwrap();
        assert_eq!(status.name, "Training Boss");
        assert_eq!(status.health, 1234.0);
        assert_eq!(status.maximum, 20000.0);
        assert_eq!(status.debuffs, vec![(589, 4.5), (339, 2.0)]);
        assert_eq!(status.debuff_stacks, vec![(1943, 3), (2818, 2), (133, 1)]);
    }
}
