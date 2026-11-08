//! Compact Classic-style party health frames. Only real party players enter this list.
use super::*;

#[derive(Component)]
pub(super) struct Root;
#[derive(Component)]
pub(super) struct Member(Entity);
#[derive(Component)]
pub(super) struct Fill(Entity);
#[derive(Component)]
pub(super) struct Health(Entity);

fn text(font: &Handle<Font>, value: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(value),
        TextFont {
            font: font.clone(),
            font_size: size,
            ..default()
        },
        TextColor(color),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn party_frames_update_in_place_select_allies_and_remove_departed_members() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<Phase>()
            .init_resource::<Target>()
            .init_resource::<Selection>()
            .insert_resource(hud::HudFont(Handle::default()))
            .add_systems(Update, refresh);
        let status = Status {
            name: "Alice".into(),
            health: 75.0,
            maximum: 100.0,
            level: 20,
            friendly: true,
            debuffs: vec![],
            debuff_stacks: vec![],
        };
        let ally = app.world_mut().spawn((PartyPlayer, status.clone())).id();
        app.world_mut()
            .spawn((PartyPlayer, status.clone(), Hero { vy: 0.0, air: 0.0 }));
        app.world_mut().spawn(status);
        app.world_mut()
            .resource_mut::<NextState<Phase>>()
            .set(Phase::Playing);
        app.update();
        let world = app.world_mut();
        let root = world
            .query_filtered::<Entity, With<Root>>()
            .single(world)
            .unwrap();
        assert_eq!(world.query::<&Fill>().iter(world).count(), 1);
        assert_eq!(
            world
                .query::<(&Fill, &Node)>()
                .single(world)
                .unwrap()
                .1
                .width,
            Val::Percent(75.0)
        );
        let row = world
            .query_filtered::<Entity, With<Member>>()
            .single(world)
            .unwrap();
        world.entity_mut(row).insert(Interaction::Pressed);
        world.get_mut::<Status>(ally).unwrap().health = 0.0;
        app.update();
        let world = app.world_mut();
        assert_eq!(world.resource::<Target>().entity, Some(ally));
        assert_eq!(
            world
                .query_filtered::<Entity, With<Root>>()
                .single(world)
                .unwrap(),
            root
        );
        assert_eq!(
            world
                .query_filtered::<&Text, With<Health>>()
                .single(world)
                .unwrap()
                .0,
            "Mort"
        );
        world.despawn(ally);
        app.update();
        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<Entity, With<Root>>()
                .iter(world)
                .count(),
            0
        );
    }
}

pub(super) fn refresh(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    font: Option<Res<hud::HudFont>>,
    members: Query<(Entity, &Status), (With<PartyPlayer>, Without<Hero>)>,
    roots: Query<Entity, With<Root>>,
    mut fills: Query<(&Fill, &mut Node)>,
    mut captions: Query<(&Health, &mut Text)>,
    buttons: Query<(&Member, &Interaction)>,
    mut target: ResMut<Target>,
    mut selection: ResMut<Selection>,
    mut cache: Local<Vec<(Entity, String, u32)>>,
) {
    let Some(font) = font else { return };
    let mut party: Vec<_> = members
        .iter()
        .filter(|(_, s)| s.friendly && *phase.get() != Phase::Menu)
        .collect();
    party.sort_by_key(|(e, _)| e.to_bits());
    for (member, interaction) in &buttons {
        if *interaction != Interaction::None {
            selection.ui_captured = true;
        }
        if *interaction == Interaction::Pressed && members.contains(member.0) {
            target.entity = Some(member.0);
        }
    }
    let label = |s: &Status| {
        if s.health <= 0.0 {
            "Mort".to_owned()
        } else {
            format!("{:.0} / {:.0}", s.health, s.maximum)
        }
    };
    for (fill, mut node) in &mut fills {
        if let Ok((_, s)) = members.get(fill.0) {
            node.width = Val::Percent((s.health / s.maximum.max(1.0)).clamp(0.0, 1.0) * 100.0);
        }
    }
    for (health, mut text) in &mut captions {
        if let Ok((_, s)) = members.get(health.0) {
            let next = label(s);
            if text.0 != next {
                text.0 = next;
            }
        }
    }
    let key: Vec<_> = party
        .iter()
        .map(|(e, s)| (*e, s.name.clone(), s.level))
        .collect();
    if *cache == key {
        return;
    }
    *cache = key;
    for root in &roots {
        commands.entity(root).despawn();
    }
    if party.is_empty() {
        return;
    }
    let f = &font.0;
    commands
        .spawn((
            Root,
            GlobalZIndex(415),
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(18.0),
                top: Val::Px(204.0),
                width: Val::Px(210.0),
                align_items: AlignItems::FlexEnd,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(5.0),
                ..default()
            },
        ))
        .with_children(|ui| {
            ui.spawn((
                text(f, "Groupe", 13.0, hud::GOLD_TONE),
                TextLayout::new_with_justify(Justify::Right),
                Node {
                    width: Val::Percent(100.0),
                    ..default()
                },
            ));
            for (entity, status) in party {
                ui.spawn((
                    Button,
                    Member(entity),
                    BackgroundColor(Color::srgba(0.025, 0.025, 0.02, 0.95)),
                    BorderColor::all(hud::GOLD_TONE.with_alpha(0.65)),
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(53.0),
                        padding: UiRect::all(Val::Px(5.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(3.0),
                        ..default()
                    },
                ))
                .with_children(|frame| {
                    frame.spawn((
                        text(
                            f,
                            format!("{} · {}", status.name, status.level),
                            12.0,
                            Color::WHITE,
                        ),
                        TextLayout::new_with_justify(Justify::Right),
                        Node {
                            width: Val::Percent(100.0),
                            ..default()
                        },
                    ));
                    frame
                        .spawn((
                            BackgroundColor(Color::srgb(0.13, 0.025, 0.025)),
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(20.0),
                                justify_content: JustifyContent::FlexEnd,
                                ..default()
                            },
                        ))
                        .with_children(|bar| {
                            bar.spawn((
                                Fill(entity),
                                BackgroundColor(Color::srgb(0.12, 0.55, 0.15)),
                                Node {
                                    width: Val::Percent(
                                        (status.health / status.maximum.max(1.0)).clamp(0.0, 1.0)
                                            * 100.0,
                                    ),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                            ));
                            bar.spawn((
                                Health(entity),
                                text(f, label(status), 11.0, Color::WHITE),
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: Val::Px(0.0),
                                    top: Val::Px(1.0),
                                    width: Val::Percent(100.0),
                                    ..default()
                                },
                                TextLayout::new(Justify::Right, LineBreak::NoWrap),
                            ));
                        });
                });
            }
        });
}
