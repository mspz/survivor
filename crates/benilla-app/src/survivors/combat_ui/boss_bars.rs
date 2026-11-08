//! One screen-anchored health bar per living boss, independent of distance and camera.
use super::super::enemies::Enemy;
use super::*;

#[derive(Component)]
struct Root;
#[derive(Component)]
struct Row(Entity);
#[derive(Component)]
struct Fill(Entity);
#[derive(Component)]
struct Health(Entity);

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (sync, refresh)
            .chain()
            .after(super::super::abilities::apply_hits),
    );
}

fn sync(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    selection: Res<Selection>,
    font: Option<Res<hud::HudFont>>,
    enemies: Query<(Entity, &Enemy)>,
    roots: Query<Entity, With<Root>>,
    rows: Query<(Entity, &Row)>,
) {
    let visible = !selection.in_hub
        && matches!(
            *phase.get(),
            Phase::Playing | Phase::Paused | Phase::LevelUp | Phase::CharacterSheet
        );
    let mut bosses: Vec<_> = enemies
        .iter()
        .filter(|(_, enemy)| enemy.boss && !enemy.dead && enemy.hp > 0.0)
        .collect();
    if !visible || bosses.is_empty() {
        for root in &roots {
            commands.entity(root).despawn();
        }
        return;
    }
    let Some(font) = font else { return };
    let root = roots.iter().next().unwrap_or_else(|| {
        commands
            .spawn((
                Root,
                GlobalZIndex(450),
                Node {
                    position_type: PositionType::Absolute,
                    // Below the existing clock, centred across the screen.
                    top: Val::Px(82.0),
                    left: Val::Px(0.0),
                    right: Val::Px(0.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(7.0),
                    ..default()
                },
            ))
            .id()
    });
    for (entity, row) in &rows {
        if !bosses.iter().any(|(boss, _)| *boss == row.0) {
            commands.entity(entity).despawn();
        }
    }
    bosses.sort_by_key(|(entity, _)| entity.to_bits());
    for (boss, enemy) in bosses {
        if rows.iter().any(|(_, row)| row.0 == boss) {
            continue;
        }
        let row = commands
            .spawn((
                Row(boss),
                Node {
                    width: Val::Px(460.0),
                    max_width: Val::Vw(80.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(2.0),
                    ..default()
                },
            ))
            .with_children(|ui| {
                ui.spawn(text(&font.0, enemy.def.name, 16.0, GOLD));
                ui.spawn((
                    BackgroundColor(Color::srgb(0.055, 0.015, 0.025)),
                    BorderColor::all(Color::srgb(0.6, 0.48, 0.25)),
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(20.0),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                ))
                .with_children(|bar| {
                    bar.spawn((
                        Fill(boss),
                        BackgroundColor(Color::srgb(0.65, 0.035, 0.13)),
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                    ));
                    bar.spawn((Node {
                        position_type: PositionType::Absolute,
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },))
                        .with_children(|label| {
                            label.spawn((Health(boss), text(&font.0, "", 12.0, Color::WHITE)));
                        });
                });
            })
            .id();
        commands.entity(root).add_child(row);
    }
}

fn refresh(
    enemies: Query<&Enemy>,
    mut fills: Query<(&Fill, &mut Node)>,
    mut labels: Query<(&Health, &mut Text)>,
) {
    for (fill, mut node) in &mut fills {
        if let Ok(enemy) = enemies.get(fill.0) {
            let ratio = if enemy.max_hp > 0.0 {
                (enemy.hp / enemy.max_hp).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let width = Val::Percent(ratio * 100.0);
            if node.width != width {
                node.width = width;
            }
        }
    }
    for (label, mut text) in &mut labels {
        if let Ok(enemy) = enemies.get(label.0) {
            let hp = enemy.hp.clamp(0.0, enemy.max_hp.max(0.0));
            let ratio = if enemy.max_hp > 0.0 {
                hp / enemy.max_hp
            } else {
                0.0
            };
            let value = format!(
                "{hp:.0} / {:.0}  ({:.0}%)",
                enemy.max_hp.max(0.0),
                ratio * 100.0
            );
            if text.0 != value {
                text.0 = value;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    static DEF: super::super::super::data::EnemyDef = super::super::super::data::EnemyDef {
        name: "Test boss",
        display: 1,
        hp: 100.0,
        speed: 1.0,
        damage: 1.0,
        scale: 1.0,
        xp: 1,
        level: 1,
        big: true,
    };
    fn enemy(boss: bool, hp: f32) -> Enemy {
        Enemy {
            def: &DEF,
            hp,
            max_hp: 100.0,
            speed: 1.0,
            damage: 1.0,
            radius: 1.0,
            xp: 1,
            boss,
            attack_cd: 0.0,
            slow: 1.0,
            slow_t: 0.0,
            root_t: 0.0,
            stun_t: 0.0,
            fear_t: 0.0,
            weaken: 1.0,
            weaken_t: 0.0,
            dots: Vec::new(),
            dead: false,
        }
    }
    #[test]
    fn multiple_bosses_stack_update_independently_and_disappear_on_death_or_leaving() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<Phase>()
            .insert_resource(Selection {
                in_hub: false,
                ..default()
            })
            .insert_resource(hud::HudFont(Handle::default()));
        plugin(&mut app);
        let first = app.world_mut().spawn(enemy(true, 100.0)).id();
        let second = app.world_mut().spawn(enemy(true, 50.0)).id();
        app.world_mut().spawn(enemy(false, 100.0));
        let mut dummy = enemy(false, 100.0);
        dummy.def = &super::super::super::training::DUMMY;
        app.world_mut().spawn(dummy);
        app.world_mut()
            .resource_mut::<NextState<Phase>>()
            .set(Phase::Playing);
        app.update();
        assert_eq!(app.world_mut().query::<&Row>().iter(app.world()).count(), 2);
        let root = app
            .world_mut()
            .query_filtered::<&Node, With<Root>>()
            .single(app.world())
            .unwrap();
        assert_eq!(root.flex_direction, FlexDirection::Column);
        app.world_mut().get_mut::<Enemy>(first).unwrap().hp = 25.0;
        app.update();
        for (fill, node) in app.world_mut().query::<(&Fill, &Node)>().iter(app.world()) {
            assert_eq!(
                node.width,
                Val::Percent(if fill.0 == first { 25.0 } else { 50.0 })
            );
        }
        app.world_mut().get_mut::<Enemy>(second).unwrap().dead = true;
        app.update();
        assert_eq!(app.world_mut().query::<&Row>().iter(app.world()).count(), 1);
        app.world_mut()
            .resource_mut::<NextState<Phase>>()
            .set(Phase::CharacterSheet);
        app.update();
        assert_eq!(app.world_mut().query::<&Row>().iter(app.world()).count(), 1);
        app.world_mut()
            .resource_mut::<NextState<Phase>>()
            .set(Phase::Hub);
        app.update();
        assert_eq!(app.world_mut().query::<&Row>().iter(app.world()).count(), 0);
    }
}
