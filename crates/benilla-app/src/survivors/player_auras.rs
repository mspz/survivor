//! Player buff/debuff rows beside the minimap; replicated descriptions follow the host's effects.
use super::{data, hero::Hero, hud, multiplayer::Role, Phase, Run};
use bevy::{prelude::*, window::PrimaryWindow};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Aura {
    pub spell: u32,
    pub name: String,
    pub description: String,
    pub left: Option<f32>,
    pub debuff: bool,
}
impl Aura {
    pub(super) fn buff(
        spell: u32,
        name: impl Into<String>,
        description: impl Into<String>,
        left: Option<f32>,
    ) -> Self {
        Self {
            spell,
            name: name.into(),
            description: description.into(),
            left,
            debuff: false,
        }
    }
}
#[derive(Resource, Default)]
pub(super) struct Remote(pub Vec<Aura>);
pub(super) fn collect(run: &Run) -> Vec<Aura> {
    if run.dead_t.is_some() || run.hp <= 0.0 {
        return vec![];
    }
    let mut out = super::items::combat::active_buffs(run);
    out.extend(super::monster_specials::player::buffs(run));
    for b in run.support.buffs.iter().filter(|b| b.until > run.elapsed) {
        if let Some(def) = data::ABILITIES.iter().find(|d| d.key == b.key) {
            let mut desc = super::support_spells::description(b.key)
                .unwrap_or("Active buff.")
                .to_owned();
            if b.power > 0.0 {
                desc.push_str(&format!(
                    "\nPower: {}",
                    if super::support_spells::magnitude(b.key, 1) < 1.0 {
                        format!("{:.1}%", b.power * 100.0)
                    } else {
                        format!("{:.0}", b.power)
                    }
                ));
            }
            if b.reserve > 0.0 {
                desc.push_str(&format!("\nRemaining absorption: {:.0}", b.reserve));
            }
            if b.jumps > 0 {
                desc.push_str(&format!("\nJumps remaining: {}", b.jumps));
            }
            out.push(Aura::buff(
                def.spell,
                super::support_spells::name(b.key).unwrap_or(def.key),
                desc,
                Some(b.until - run.elapsed),
            ));
        }
    }
    if run.shield > 0.0 && run.shield_t > 0.0 {
        out.push(Aura::buff(
            17,
            "Absorption Shield",
            format!("Absorbs {:.0} more damage.", run.shield),
            Some(run.shield_t),
        ));
    }
    if run.immune_t > 0.0 {
        let def = run
            .abilities
            .iter()
            .find(|o| o.def.kind == data::Kind::Immune)
            .map(|o| o.def);
        out.push(Aura::buff(
            def.map_or(642, |d| d.spell),
            "Immunity",
            "Prevents incoming damage.",
            Some(run.immune_t),
        ));
    }
    if run.thorns_t > 0.0 {
        out.push(Aura::buff(
            if run.thorns_spell == 0 {
                467
            } else {
                run.thorns_spell
            },
            "Thorns",
            format!(
                "Reflects damage to attackers. Power: {:.0}.",
                run.thorns
            ),
            Some(run.thorns_t),
        ));
    }
    if run.heal_t > 0.0 && run.heal_rate > 0.0 {
        let def = run
            .heal_credit
            .and_then(|(_, key)| data::ABILITIES.iter().find(|d| d.key == key));
        out.push(Aura::buff(
            def.map_or(139, |d| d.spell),
            "Healing over time",
            format!("Restores {:.1} health per second.", run.heal_rate),
            Some(run.heal_t),
        ));
    }
    if let Some(m) = super::mobility::for_class(run.class.id) {
        if run.mobile.haste_t > 0.0 {
            out.push(Aura::buff(
                m.spell,
                m.name,
                format!(
                    "Movement speed: +{:.0}%.",
                    (run.mobile.haste - 1.0) * 100.0
                ),
                Some(run.mobile.haste_t),
            ));
        }
        if run.mobile.vanish_t > 0.0 {
            out.push(Aura::buff(
                m.spell,
                m.name,
                "Normal enemies ignore you while this effect is active.",
                Some(run.mobile.vanish_t),
            ));
        }
    }
    out.extend(run.healer_buffs.visible.iter().cloned());
    out.extend(super::items::legendary::visible_buffs(run));
    if run.legendary.effects.shield_lock > run.elapsed {
        out.push(Aura { spell: 17, name: "Shattered Shield".into(), description: "Your shield cannot be renewed. Collect its fragments to end this effect sooner.".into(), left: Some(run.legendary.effects.shield_lock-run.elapsed), debuff: true });
    }
    out.retain(|a| a.left.is_none_or(|t| t > 0.0 && t.is_finite()));
    out.sort_by(|a, b| (a.spell, &a.name).cmp(&(b.spell, &b.name)));
    out.dedup_by(|a, b| a.spell == b.spell && a.name == b.name);
    out
}
#[derive(Component)]
struct Root;
#[derive(Component)]
struct Tooltip;
#[derive(Component)]
struct Icon(usize);
#[derive(Component)]
struct Duration(usize);
#[derive(Resource, Default)]
struct Shown(Vec<Aura>);
fn text(font: &Handle<Font>, s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(s),
        TextFont {
            font: font.clone(),
            font_size: size,
            ..default()
        },
        TextColor(color),
    )
}
fn remaining(left: Option<f32>) -> String {
    match left {
        None => String::new(),
        Some(t) if t >= 60.0 => format!("{}m", (t / 60.0).ceil() as u32),
        Some(t) => format!("{}", t.ceil().max(0.0) as u32),
    }
}
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Remote>()
        .init_resource::<Shown>()
        .add_systems(
            Update,
            render
                .after(hud::refresh_hud)
                .after(super::unit_frames::sync_status),
        )
        .add_systems(Update, tooltip.after(render));
}
#[allow(clippy::too_many_arguments)]
fn render(
    mut commands: Commands,
    run: Res<Run>,
    phase: Res<State<Phase>>,
    role: Option<Res<Role>>,
    remote: Res<Remote>,
    status: Query<&super::unit_frames::Status, With<Hero>>,
    spells: Option<Res<crate::ui_action::Spells>>,
    font: Option<Res<hud::HudFont>>,
    mut shown: ResMut<Shown>,
    roots: Query<Entity, With<Root>>,
    mut durations: Query<(&Duration, &mut Text)>,
    mut cache: Local<String>,
) {
    let Some(font) = font else { return };
    let active = !matches!(*phase.get(), Phase::Menu | Phase::GameOver)
        && run.hp > 0.0
        && run.dead_t.is_none();
    let mut auras = if !active {
        vec![]
    } else if role.as_deref() == Some(&Role::Guest) {
        remote.0.clone()
    } else {
        collect(&run)
    };
    if active {
        if let Ok(status) = status.single() {
            for &(spell, left) in &status.debuffs {
                if left > 0.0 {
                    let display = spells.as_deref().and_then(|s| s.catalog.get(spell));
                    let def = data::ABILITIES.iter().find(|d| d.spell == spell);
                    let name = display.map(|d| d.name.clone()).unwrap_or_else(|| {
                        hud::spell_name(
                            spells.as_deref(),
                            spell,
                            def.map_or("Debuff", |d| d.key),
                        )
                    });
                    let description = def
                        .map(|d| {
                            hud::describe_card(hud::Choice::NewAbility(d), &run, spells.as_deref())
                                .desc
                        })
                        .unwrap_or_else(|| "Active debuff on your character.".into());
                    auras.push(Aura {
                        spell,
                        name,
                        description,
                        left: Some(left),
                        debuff: true,
                    });
                }
            }
        }
    }
    auras.retain(|a| a.left.is_none_or(|t| t > 0.0 && t.is_finite()));
    auras.sort_by(|a, b| (a.debuff, a.spell, &a.name).cmp(&(b.debuff, b.spell, &b.name)));
    auras.dedup_by(|a, b| a.debuff == b.debuff && a.spell == b.spell && a.name == b.name);
    let key = format!(
        "{:?}",
        auras
            .iter()
            .map(|a| (a.spell, a.debuff, a.name.as_str()))
            .collect::<Vec<_>>()
    );
    shown.0 = auras;
    if *cache == key {
        for (id, mut label) in &mut durations {
            if let Some(a) = shown.0.get(id.0) {
                let s = remaining(a.left);
                if label.0 != s {
                    label.0 = s;
                }
            }
        }
        return;
    }
    *cache = key;
    for e in &roots {
        commands.entity(e).despawn();
    }
    if shown.0.is_empty() {
        return;
    }
    commands
        .spawn((
            Root,
            GlobalZIndex(425),
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(204.0),
                top: Val::Px(18.0),
                width: Val::Px(352.0),
                max_width: Val::Percent(55.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                ..default()
            },
        ))
        .with_children(|root| {
            for debuff in [false, true] {
                if !shown.0.iter().any(|a| a.debuff == debuff) {
                    continue;
                }
                root.spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::RowReverse,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(6.0),
                    row_gap: Val::Px(5.0),
                    ..default()
                })
                .with_children(|row| {
                    for (index, a) in shown
                        .0
                        .iter()
                        .enumerate()
                        .filter(|(_, a)| a.debuff == debuff)
                    {
                        row.spawn(Node {
                            width: Val::Px(36.0),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            row_gap: Val::Px(2.0),
                            ..default()
                        })
                        .with_children(|cell| {
                            cell.spawn((
                                Button,
                                Icon(index),
                                BorderColor::all(if debuff {
                                    Color::srgb(0.9, 0.1, 0.1)
                                } else {
                                    hud::GOLD_TONE
                                }),
                                Node {
                                    width: Val::Px(36.0),
                                    height: Val::Px(36.0),
                                    border: UiRect::all(Val::Px(2.0)),
                                    ..default()
                                },
                            ))
                            .with_children(|icon| {
                                icon.spawn((
                                    ImageNode::default(),
                                    hud::SpellIcon(a.spell),
                                    Node {
                                        width: Val::Percent(100.0),
                                        height: Val::Percent(100.0),
                                        ..default()
                                    },
                                ));
                            });
                            cell.spawn((
                                Duration(index),
                                text(&font.0, remaining(a.left), 11.0, Color::WHITE),
                            ));
                        });
                    }
                });
            }
        });
}
fn tooltip(
    mut commands: Commands,
    shown: Res<Shown>,
    hover: Query<(&Interaction, &Icon)>,
    font: Option<Res<hud::HudFont>>,
    roots: Query<Entity, With<Tooltip>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    scale: Option<Res<UiScale>>,
) {
    for e in &roots {
        commands.entity(e).despawn();
    }
    let Some(font) = font else { return };
    let Some(a) = hover
        .iter()
        .find(|(i, _)| **i != Interaction::None)
        .and_then(|(_, id)| shown.0.get(id.0))
    else {
        return;
    };
    let inv = scale.as_deref().map_or(1.0, |s| s.0).max(0.01);
    let (viewport, cursor) =
        windows
            .single()
            .map_or((Vec2::new(1280.0, 768.0), Vec2::new(800.0, 100.0)), |w| {
                (
                    Vec2::new(w.width(), w.height()) / inv,
                    w.cursor_position().unwrap_or_default() / inv,
                )
            });
    let x = (cursor.x - 340.0).clamp(4.0, (viewport.x - 340.0).max(4.0));
    commands
        .spawn((
            Tooltip,
            GlobalZIndex(850),
            bevy::ui::FocusPolicy::Pass,
            BackgroundColor(Color::srgb(0.02, 0.025, 0.04)),
            BorderColor::all(if a.debuff {
                Color::srgb(0.9, 0.1, 0.1)
            } else {
                hud::GOLD_TONE
            }),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(x),
                top: Val::Px((cursor.y + 22.0).min((viewport.y - 260.0).max(4.0))),
                width: Val::Px(330.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
        ))
        .with_children(|body| {
            body.spawn(text(&font.0, &a.name, 17.0, hud::GOLD_TONE));
            body.spawn(text(
                &font.0,
                if a.debuff {
                    "Debuff"
                } else {
                    "Buff"
                },
                12.0,
                Color::WHITE,
            ));
            body.spawn(text(&font.0, &a.description, 14.0, Color::WHITE));
            body.spawn(text(
                &font.0,
                a.left
                    .map_or("Active while you remain in range.".into(), |t| {
                        format!("Time remaining: {t:.1} s")
                    }),
                12.0,
                hud::GOLD_TONE,
            ));
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn active_buffs_expire_and_dead_players_have_no_auras() {
        let mut run = Run::default();
        run.elapsed = 10.0;
        super::super::support_spells::grant(&mut run, "battle_shout", 0.4, 5.0, None);
        run.shield = 25.0;
        run.shield_t = 8.0;
        let auras = collect(&run);
        assert_eq!(auras.len(), 2);
        assert!(auras.iter().any(|a| a.left == Some(5.0)));
        run.elapsed = 16.0;
        assert_eq!(collect(&run).len(), 1);
        run.dead_t = Some(0.0);
        assert!(collect(&run).is_empty());
    }
    #[test]
    fn buffs_and_debuffs_update_in_place_show_tooltips_and_disappear() {
        let mut run = Run::default();
        run.shield = 25.0;
        run.shield_t = 8.0;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(State::new(Phase::Hub))
            .insert_resource(run)
            .init_resource::<Remote>()
            .init_resource::<Shown>()
            .insert_resource(hud::HudFont(Handle::default()))
            .add_systems(Update, (render, tooltip).chain());
        let hero = app
            .world_mut()
            .spawn((
                Hero { vy: 0.0, air: 0.0 },
                super::super::unit_frames::Status {
                    name: "Test".into(),
                    health: 100.0,
                    maximum: 100.0,
                    level: 1,
                    friendly: true,
                    debuffs: vec![(116, 3.0)],
                    debuff_stacks: vec![],
                },
            ))
            .id();
        app.update();
        let world = app.world_mut();
        let root = world
            .query_filtered::<Entity, With<Root>>()
            .single(world)
            .unwrap();
        assert_eq!(world.query::<&Icon>().iter(world).count(), 2);
        assert_eq!(world.get::<Node>(root).unwrap().right, Val::Px(204.0));
        let icon = world
            .query_filtered::<Entity, With<Icon>>()
            .iter(world)
            .next()
            .unwrap();
        world.entity_mut(icon).insert(Interaction::Hovered);
        {
            let mut run = world.resource_mut::<Run>();
            run.shield = 10.0;
            run.shield_t = 7.2;
        }
        app.update();
        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<Entity, With<Root>>()
                .single(world)
                .unwrap(),
            root
        );
        assert_eq!(
            world
                .query_filtered::<Entity, With<Tooltip>>()
                .iter(world)
                .count(),
            1
        );
        assert!(world
            .resource::<Shown>()
            .0
            .iter()
            .any(|a| a.description.contains("10 more damage")));
        {
            let mut run = world.resource_mut::<Run>();
            run.shield = 0.0;
            run.shield_t = 0.0;
        }
        world
            .get_mut::<super::super::unit_frames::Status>(hero)
            .unwrap()
            .debuffs
            .clear();
        app.update();
        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<Entity, With<Root>>()
                .iter(world)
                .count(),
            0
        );
        assert_eq!(
            world
                .query_filtered::<Entity, With<Tooltip>>()
                .iter(world)
                .count(),
            0
        );
    }
    #[test]
    fn guest_uses_the_hosts_buff_list_instead_of_incomplete_local_runtime() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(State::new(Phase::Playing))
            .init_resource::<Run>()
            .init_resource::<Shown>()
            .insert_resource(Role::Guest)
            .insert_resource(Remote(vec![Aura::buff(
                6673,
                "Battle Shout",
                "+40% damage",
                Some(4.0),
            )]))
            .insert_resource(hud::HudFont(Handle::default()))
            .add_systems(Update, render);
        app.update();
        assert_eq!(app.world().resource::<Shown>().0[0].name, "Battle Shout");
        assert_eq!(app.world().resource::<Shown>().0[0].left, Some(4.0));
    }
}
