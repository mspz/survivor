use super::portraits::Portrait;
use super::*;
use bevy::{ui::FocusPolicy, window::PrimaryWindow};

#[derive(Component)]
pub(super) struct Frame(bool);
#[derive(Component)]
pub(super) struct Fill(bool);
#[derive(Component)]
pub(super) struct Shield;
#[derive(Component)]
pub(super) struct Caption(bool, u8);
#[derive(Component)]
pub(super) struct Debuffs;
#[derive(Component)]
pub(super) struct DebuffIcon(pub u32);
#[derive(Component)]
pub(super) struct DebuffTimer(u32);
#[derive(Component)]
pub(super) struct DebuffStacks(u32);
#[derive(Resource, Default)]
pub(super) struct Cache {
    spells: Vec<u32>,
}
const W: f32 = 240.0;
fn rect(x: f32, y: f32, w: f32, h: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(x),
        top: Val::Px(y),
        width: Val::Px(w),
        height: Val::Px(h),
        ..default()
    }
}
fn text(font: &Handle<Font>, value: &str, size: f32) -> impl Bundle {
    (
        Text::new(value),
        TextFont {
            font: font.clone(),
            font_size: size,
            ..default()
        },
        TextColor(Color::WHITE),
        TextShadow {
            offset: Vec2::ONE,
            color: Color::BLACK,
        },
        TextLayout::new(Justify::Left, LineBreak::NoWrap),
        FocusPolicy::Pass,
    )
}
pub(super) fn setup(mut commands: Commands, font: Res<hud::HudFont>) {
    commands.init_resource::<Cache>();
    for mirrored in [false, true] {
        commands
            .spawn((
                Frame(mirrored),
                Interaction::default(),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(if mirrored { 68.0 } else { 32.0 }),
                    margin: UiRect::left(Val::Px(-W / 2.0)),
                    bottom: Val::Percent(22.0),
                    width: Val::Px(W),
                    height: Val::Px(76.0),
                    display: Display::None,
                    ..default()
                },
                GlobalZIndex(410),
                FocusPolicy::Pass,
            ))
            .with_children(|ui| {
                let portrait_x = if mirrored { W - 66.0 } else { 0.0 };
                let bar_x = if mirrored { 1.0 } else { 61.0 };
                ui.spawn((
                    rect(bar_x, 13.0, 178.0, 47.0),
                    BackgroundColor(Color::srgb(0.025, 0.025, 0.02)),
                    BorderColor::all(Color::srgb(0.5, 0.48, 0.4)),
                    FocusPolicy::Pass,
                ))
                .insert(Node {
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..rect(bar_x, 13.0, 178.0, 47.0)
                });
                ui.spawn((
                    rect(bar_x + 3.0, 31.0, 172.0, 18.0),
                    BackgroundColor(Color::srgb(0.11, 0.025, 0.025)),
                    FocusPolicy::Pass,
                ))
                .with_children(|bar| {
                    bar.spawn((
                        Fill(mirrored),
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.06, 0.65, 0.07)),
                        ImageNode::default(),
                        hud::UiArt::tinted(
                            "Interface\\TargetingFrame\\UI-StatusBar",
                            Color::srgb(0.06, 0.65, 0.07),
                        ),
                        FocusPolicy::Pass,
                    ));
                    if !mirrored {
                        bar.spawn((
                            Shield,
                            Node {
                                position_type: PositionType::Absolute,
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.65, 0.85, 1.0, 0.65)),
                            FocusPolicy::Pass,
                        ));
                    }
                });
                for (kind, y, value, size) in [
                    (0, 15.0, "", 12.0),
                    (1, 32.0, "", 11.0),
                    (2, 51.0, "", 10.0),
                ] {
                    ui.spawn((
                        Caption(mirrored, kind),
                        Node {
                            overflow: Overflow::clip(),
                            ..rect(bar_x + 7.0, y, 164.0, 16.0)
                        },
                        text(&font.0, value, size),
                    ));
                }
                // Circular live portrait and the Classic gold rim, mirrored on the target.
                ui.spawn((
                    Node {
                        border: UiRect::all(Val::Px(3.0)),
                        border_radius: BorderRadius::MAX,
                        overflow: Overflow::clip(),
                        ..rect(portrait_x, 4.0, 66.0, 66.0)
                    },
                    BorderColor::all(hud::GOLD_TONE),
                    BackgroundColor(Color::srgb(0.045, 0.035, 0.02)),
                    FocusPolicy::Pass,
                ))
                .with_children(|circle| {
                    circle.spawn((
                        Portrait(mirrored),
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            border_radius: BorderRadius::MAX,
                            ..default()
                        },
                        ImageNode::default(),
                        FocusPolicy::Pass,
                    ));
                });
                if mirrored {
                    ui.spawn((
                        Debuffs,
                        Node {
                            position_type: PositionType::Absolute,
                            right: Val::Px(4.0),
                            top: Val::Px(80.0),
                            width: Val::Px(W),
                            flex_wrap: FlexWrap::Wrap,
                            justify_content: JustifyContent::FlexEnd,
                            column_gap: Val::Px(3.0),
                            row_gap: Val::Px(3.0),
                            ..default()
                        },
                        FocusPolicy::Pass,
                    ));
                }
            });
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn refresh(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    run: Res<Run>,
    mut target: ResMut<Target>,
    font: Res<hud::HudFont>,
    units: Query<&Status>,
    heroes: Query<Entity, With<Hero>>,
    mut frames: Query<(&Frame, &mut Node), Without<Fill>>,
    mut fills: Query<(&Fill, &mut Node, &mut BackgroundColor, &mut ImageNode), Without<Frame>>,
    mut shields: Query<&mut Node, (With<Shield>, Without<Fill>, Without<Frame>)>,
    mut captions: Query<(&Caption, &mut Text), Without<DebuffStacks>>,
    labels: (
        Query<(&DebuffTimer, &mut Text), (Without<Caption>, Without<DebuffStacks>)>,
        Query<(&DebuffStacks, &mut Text), (Without<Caption>, Without<DebuffTimer>)>,
    ),
    debuffs: Query<Entity, With<Debuffs>>,
    mut cache: ResMut<Cache>,
    windows: Query<&Window, With<PrimaryWindow>>,
    scale: Res<UiScale>,
) {
    let (mut timers, mut stacks) = labels;
    let show = !matches!(*phase.get(), Phase::Menu | Phase::GameOver);
    let player = heroes.single().ok().and_then(|e| units.get(e).ok());
    let victim = target.entity.and_then(|e| units.get(e).ok());
    if target.entity.is_some() && victim.is_none() {
        target.entity = None;
    }
    let narrow = windows.single().is_ok_and(|w| w.width() / scale.0 < 800.0);
    for (frame, mut node) in &mut frames {
        let status = if frame.0 { victim } else { player };
        node.display = if show && status.is_some() {
            Display::Flex
        } else {
            Display::None
        };
        node.left = Val::Percent(if narrow {
            50.0
        } else if frame.0 {
            68.0
        } else {
            32.0
        });
        node.margin.left = Val::Px(if narrow {
            if frame.0 {
                12.0
            } else {
                -W - 12.0
            }
        } else {
            -W / 2.0
        });
        // Keep icons above the XP strip and leave room for target debuffs below.
        node.bottom = Val::Percent(22.0);
    }
    for (fill, mut node, mut color, mut image) in &mut fills {
        if let Some(status) = if fill.0 { victim } else { player } {
            node.width =
                Val::Percent((status.health / status.maximum.max(1.0)).clamp(0.0, 1.0) * 100.0);
            color.0 = if status.friendly {
                Color::srgb(0.08, 0.65, 0.08)
            } else {
                Color::srgb(0.7, 0.12, 0.09)
            };
            image.color = color.0;
        }
    }
    for mut shield in &mut shields {
        shield.width = Val::Percent(if run.shield_t > 0.0 {
            (run.shield / run.stats().max_hp).clamp(0.0, 1.0) * 100.0
        } else {
            0.0
        });
    }
    for (caption, mut label) in &mut captions {
        if let Some(status) = if caption.0 { victim } else { player } {
            let next = match caption.1 {
                0 => format!("{}   {}", status.name, status.level),
                1 => format!(
                    "{:.0}%     {:.0} / {:.0}",
                    (status.health / status.maximum.max(1.0) * 100.0).clamp(0.0, 100.0),
                    status.health,
                    status.maximum
                ),
                _ => {
                    if caption.0 {
                        if status.friendly {
                            "Friendly".into()
                        } else {
                            "Hostile".into()
                        }
                    } else if run.shield_t > 0.0 {
                        format!("Absorb: {:.0}", run.shield)
                    } else {
                        run.class.name.into()
                    }
                }
            };
            if label.0 != next {
                label.0 = next;
            }
        }
    }
    let effects: Vec<_> = victim
        .into_iter()
        .flat_map(|s| s.debuffs.iter())
        .take(24)
        .copied()
        .collect();
    let spells: Vec<_> = effects.iter().map(|e| e.0).collect();
    for (timer, mut label) in &mut timers {
        if let Some((_, left)) = effects.iter().find(|(spell, _)| *spell == timer.0) {
            let value = format!("{:.0}s", left.ceil());
            if label.0 != value {
                label.0 = value;
            }
        }
    }
    let stack_label = |spell| {
        victim
            .and_then(|s| s.debuff_stacks.iter().find(|(id, _)| *id == spell))
            .map_or_else(String::new, |(_, count)| count.to_string())
    };
    for (counter, mut label) in &mut stacks {
        let value = stack_label(counter.0);
        if label.0 != value {
            label.0 = value;
        }
    }
    if cache.spells != spells {
        cache.spells = spells;
        for parent in &debuffs {
            commands
                .entity(parent)
                .despawn_children()
                .with_children(|ui| {
                    for &(spell, left) in &effects {
                        ui.spawn((
                            DebuffIcon(spell),
                            Interaction::default(),
                            Node {
                                width: Val::Px(26.0),
                                height: Val::Px(33.0),
                                ..default()
                            },
                            FocusPolicy::Pass,
                        ))
                        .with_children(|slot| {
                            slot.spawn((
                                Node {
                                    border_radius: BorderRadius::all(Val::Px(2.0)),
                                    ..rect(0.0, 0.0, 26.0, 26.0)
                                },
                                ImageNode::default(),
                                hud::SpellIcon(spell),
                                FocusPolicy::Pass,
                            ));
                            slot.spawn((
                                DebuffStacks(spell),
                                rect(12.0, 10.0, 14.0, 16.0),
                                text(&font.0, &stack_label(spell), 14.0),
                            ));
                            slot.spawn((
                                DebuffTimer(spell),
                                rect(0.0, 23.0, 26.0, 12.0),
                                text(&font.0, &format!("{:.0}s", left.ceil()), 10.0),
                            ));
                        });
                    }
                });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frames_update_health_debuffs_and_clear_despawned_targets() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<Phase>()
            .init_resource::<Target>()
            .init_resource::<Run>()
            .init_resource::<UiScale>()
            .insert_resource(hud::HudFont(Handle::default()))
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (refresh, super::super::debuff_tooltip::show).chain(),
            );
        let status = Status {
            name: "Hero".into(),
            health: 75.0,
            maximum: 100.0,
            level: 60,
            friendly: true,
            debuffs: vec![],
            debuff_stacks: vec![],
        };
        app.world_mut()
            .spawn((Hero { vy: 0.0, air: 0.0 }, status.clone()));
        let enemy = app
            .world_mut()
            .spawn(Status {
                name: "Target".into(),
                health: 25.0,
                friendly: false,
                debuffs: vec![(1943, 4.0)],
                debuff_stacks: vec![(1943, 3)],
                ..status
            })
            .id();
        app.world_mut().resource_mut::<Target>().entity = Some(enemy);
        app.world_mut()
            .resource_mut::<NextState<Phase>>()
            .set(Phase::Playing);
        app.update();
        assert_eq!(
            app.world_mut()
                .query::<(&Frame, &Node)>()
                .iter(app.world())
                .filter(|(_, n)| n.display == Display::Flex)
                .count(),
            2
        );
        assert!(app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .any(|t| t.0 == "25%     25 / 100"));
        assert_eq!(
            app.world_mut()
                .query::<&hud::SpellIcon>()
                .iter(app.world())
                .count(),
            1
        );
        let icon = app
            .world_mut()
            .query_filtered::<Entity, With<DebuffIcon>>()
            .single(app.world())
            .unwrap();
        assert!(app
            .world_mut()
            .query::<(&DebuffStacks, &Text)>()
            .iter(app.world())
            .any(|(s, t)| s.0 == 1943 && t.0 == "3"));
        *app.world_mut().get_mut::<Interaction>(icon).unwrap() = Interaction::Hovered;
        app.update();
        assert!(app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .any(|t| t.0 == "Time remaining: 4.0 s"));
        app.world_mut().get_mut::<Status>(enemy).unwrap().debuffs[0].1 = 2.4;
        app.world_mut()
            .get_mut::<Status>(enemy)
            .unwrap()
            .debuff_stacks[0]
            .1 = 1;
        app.update();
        assert!(app.world().get::<DebuffIcon>(icon).is_some());
        assert!(app
            .world_mut()
            .query::<(&DebuffStacks, &Text)>()
            .iter(app.world())
            .any(|(_, t)| t.0 == "1"));
        assert!(app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .any(|t| t.0 == "Time remaining: 2.4 s"));
        app.world_mut()
            .get_mut::<Status>(enemy)
            .unwrap()
            .debuffs
            .clear();
        app.update();
        assert!(!app
            .world_mut()
            .query::<&Text>()
            .iter(app.world())
            .any(|t| t.0.starts_with("Time remaining:")));
        assert_eq!(
            app.world_mut()
                .query::<&hud::SpellIcon>()
                .iter(app.world())
                .count(),
            0
        );
        app.world_mut().despawn(enemy);
        app.update();
        assert_eq!(app.world().resource::<Target>().entity, None);
        assert_eq!(
            app.world_mut()
                .query::<(&Frame, &Node)>()
                .iter(app.world())
                .filter(|(_, n)| n.display == Display::Flex)
                .count(),
            1
        );
    }
}
