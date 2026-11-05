//! Classic framed meter with per-spell combat statistics.
use super::*;

#[derive(Component)]
pub(super) struct DetailsRoot;
#[derive(Component)]
pub(super) struct DetailsRows;
#[derive(Component)]
pub(super) struct MeterSkill(String);
#[derive(Component)]
pub(super) enum MeterValue {
    Summary,
    Damage(String),
    Dps(String),
    Detail(usize),
}
#[derive(Component)]
pub(super) struct MeterBar(String);

pub(super) fn interact(
    mut view: ResMut<View>,
    mut selection: ResMut<Selection>,
    mut rows: Query<(&Interaction, &MeterSkill, &mut BackgroundColor), Changed<Interaction>>,
    panels: Query<&Interaction, Or<(With<MeterRoot>, With<DetailsRoot>)>>,
) {
    if panels.iter().any(|i| *i != Interaction::None) {
        selection.ui_captured = true;
    }
    for (interaction, skill, mut background) in &mut rows {
        background.0 = if *interaction == Interaction::None {
            Color::srgba(0.04, 0.035, 0.025, 0.9)
        } else {
            Color::srgba(0.18, 0.13, 0.055, 0.95)
        };
        if *interaction == Interaction::Pressed {
            view.meter_selected = Some(skill.0.clone());
            view.details_scroll = 0.0;
            selection.ui_captured = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clicking_a_spell_opens_its_details_and_icon_without_losing_the_selected_source() {
        let mut run = Run {
            elapsed: 5.0,
            ..default()
        };
        run.meter.cast("fireball");
        run.meter.record_hit("fireball", 5.0, 42.0, true, false);
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .init_state::<Phase>()
            .insert_resource(run)
            .insert_resource(View {
                meter: true,
                ..default()
            })
            .init_resource::<Selection>()
            .insert_resource(hud::HudFont(Handle::default()))
            .add_systems(Update, (interact, render_meter).chain());
        app.world_mut()
            .resource_mut::<NextState<Phase>>()
            .set(Phase::Playing);
        app.update();
        let world = app.world_mut();
        let row = world
            .query_filtered::<Entity, With<MeterSkill>>()
            .single(world)
            .unwrap();
        world.entity_mut(row).insert(Interaction::Pressed);
        app.update();
        let world = app.world_mut();
        assert_eq!(
            world.resource::<View>().meter_selected.as_deref(),
            Some("fireball")
        );
        assert!(world.resource::<Selection>().ui_captured);
        assert_eq!(
            world
                .query_filtered::<Entity, With<DetailsRoot>>()
                .iter(world)
                .count(),
            1
        );
        assert!(world
            .query::<&hud::SpellIcon>()
            .iter(world)
            .any(|icon| icon.0 == data::ability("fireball").spell));
        assert!(world
            .query::<&Text>()
            .iter(world)
            .any(|text| text.0 == "Critical hits: min / avg / max"));
        world.resource_mut::<Run>().elapsed += 0.3;
        let root = world
            .query_filtered::<Entity, With<MeterRoot>>()
            .single(world)
            .unwrap();
        let details = world
            .query_filtered::<Entity, With<DetailsRoot>>()
            .single(world)
            .unwrap();
        let row = world
            .query_filtered::<Entity, With<MeterSkill>>()
            .single(world)
            .unwrap();
        {
            let mut run = world.resource_mut::<Run>();
            run.version += 1;
            let now = run.elapsed;
            run.meter.record_hit("fireball", now, 58.0, true, false);
        }
        app.update();
        assert_eq!(
            app.world().resource::<View>().meter_selected.as_deref(),
            Some("fireball")
        );
        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<Entity, With<MeterRoot>>()
                .single(world)
                .unwrap(),
            root
        );
        assert_eq!(
            world
                .query_filtered::<Entity, With<DetailsRoot>>()
                .single(world)
                .unwrap(),
            details
        );
        assert_eq!(
            world
                .query_filtered::<Entity, With<MeterSkill>>()
                .single(world)
                .unwrap(),
            row
        );
        assert!(world
            .query::<&Text>()
            .iter(world)
            .any(|text| text.0.ends_with("100 damage")));
        {
            let mut run = world.resource_mut::<Run>();
            let now = run.elapsed;
            run.meter.record_hit("frostbolt", now, 150.0, false, false);
        }
        app.update();
        let world = app.world_mut();
        let list = world
            .query_filtered::<Entity, With<MeterRows>>()
            .single(world)
            .unwrap();
        let first = world.get::<Children>(list).unwrap()[0];
        assert_eq!(world.get::<MeterSkill>(first).unwrap().0, "frostbolt");
        let entities: std::collections::BTreeMap<_, _> = world
            .query::<(Entity, &MeterSkill)>()
            .iter(world)
            .map(|(entity, skill)| (skill.0.clone(), entity))
            .collect();
        {
            let mut run = world.resource_mut::<Run>();
            run.elapsed += 0.3;
            let now = run.elapsed;
            run.meter.record_hit("fireball", now, 500.0, false, false);
        }
        app.update();
        let world = app.world_mut();
        assert_eq!(
            world.get::<Children>(list).unwrap()[0],
            entities["fireball"]
        );
        for entity in entities.values() {
            assert!(world.get_entity(*entity).is_ok());
        }
    }
}

fn identity(run: &Run, key: &str, spells: Option<&crate::ui_action::Spells>) -> (String, u32) {
    if let Some(def) = data::ABILITIES.iter().find(|d| d.key == key) {
        return (hud::spell_name(spells, def.spell, def.key), def.spell);
    }
    if let Some(proc) = super::super::procs::views(run)
        .iter()
        .find(|p| p.key == key || p.name == key)
    {
        return (proc.name.clone(), proc.spell);
    }
    if let Some(proc) = super::super::procs::SPELLS
        .iter()
        .find(|p| p.def.key == key)
    {
        return (key.replace('_', " "), proc.def.spell);
    }
    (key.replace('_', " "), 0)
}

fn icon(ui: &mut ChildSpawnerCommands, spell: u32, size: f32) {
    let mut image = ui.spawn((
        ImageNode::default(),
        BorderColor::all(Color::srgb(0.45, 0.36, 0.18)),
        bevy::ui::FocusPolicy::Pass,
        Node {
            width: Val::Px(size),
            height: Val::Px(size),
            flex_shrink: 0.0,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
    ));
    if spell == 0 {
        image.insert(hud::UiArt::item("Interface\\Icons\\INV_Misc_QuestionMark"));
    } else {
        image.insert(hud::SpellIcon(spell));
    }
}

pub(super) fn render_meter(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    run: Res<Run>,
    view: Res<View>,
    font: Option<Res<hud::HudFont>>,
    spells: Option<Res<crate::ui_action::Spells>>,
    roots: Query<Entity, Or<(With<MeterRoot>, With<DetailsRoot>)>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    scale: Option<Res<UiScale>>,
    mut cache: Local<String>,
    mut refresh: Local<u64>,
    mut values: Query<(&MeterValue, &mut Text)>,
    mut bars: Query<(&MeterBar, &mut Node)>,
    row_entities: Query<(Entity, &MeterSkill)>,
    mut lists: Query<&mut Children, With<MeterRows>>,
) {
    let Some(font) = font else {
        return;
    };
    let show = view.meter && *phase.get() != Phase::Menu;
    let rows = run.meter.rows(run.elapsed);
    let total_dps: f64 = rows.iter().map(|r| r.dps).sum();
    let total_damage: f64 = rows.iter().map(|r| r.total).sum();
    let maximum = rows.first().map_or(1.0, |r| r.dps.max(1.0));
    let mut sources: Vec<_> = rows.iter().map(|row| row.key.as_str()).collect();
    sources.sort_unstable();
    let key = format!(
        "{show} {:?} {:?} {:?} {}",
        phase.get(),
        sources,
        view.meter_selected,
        run.class.id
    );
    if *cache == key {
        if show {
            let tick = (run.elapsed * 4.0) as u64;
            if *refresh == tick {
                return;
            }
            *refresh = tick;
            let details = view
                .meter_selected
                .as_deref()
                .map(|key| run.meter.details(key, run.elapsed))
                .unwrap_or_default();
            for (binding, mut text) in &mut values {
                let value = match binding {
                    MeterValue::Summary => {
                        Some(format!("{total_dps:.1} DPS   |   {total_damage:.0} damage"))
                    }
                    MeterValue::Damage(key) => rows.iter().find(|r| r.key == *key).map(|r| {
                        format!(
                            "{:.0} damage   •   {:.1}%",
                            r.total,
                            r.total / total_damage.max(f64::MIN_POSITIVE) * 100.0
                        )
                    }),
                    MeterValue::Dps(key) => rows
                        .iter()
                        .find(|r| r.key == *key)
                        .map(|r| format!("{:.1}", r.dps)),
                    MeterValue::Detail(index) => {
                        details.get(*index).map(|(_, value)| value.clone())
                    }
                };
                if let Some(value) = value {
                    if text.0 != value {
                        text.0 = value;
                    }
                }
            }
            for (binding, mut node) in &mut bars {
                if let Some(row) = rows.iter().find(|r| r.key == binding.0) {
                    let width = Val::Percent((row.dps / maximum * 100.0) as f32);
                    if node.width != width {
                        node.width = width;
                    }
                }
            }
            let order: Vec<_> = rows
                .iter()
                .filter_map(|row| {
                    row_entities
                        .iter()
                        .find(|(_, skill)| skill.0 == row.key)
                        .map(|(e, _)| e)
                })
                .collect();
            for mut children in &mut lists {
                if children[..] != order[..] {
                    children.sort_by_key(|child| {
                        order.iter().position(|e| e == child).unwrap_or(usize::MAX)
                    });
                }
            }
        }
        return;
    }
    *cache = key;
    for e in &roots {
        commands.entity(e).despawn();
    }
    if !show {
        return;
    }
    let f = &font.0;
    commands
        .spawn((
            MeterRoot,
            GlobalZIndex(465),
            Interaction::default(),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(view.position.x),
                top: Val::Px(view.position.y),
                width: Val::Px(METER_WIDTH),
                padding: UiRect::all(Val::Px(8.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                ..default()
            },
        ))
        .with_children(|ui| {
            skin::backdrop(ui);
            ui.spawn((
                Button,
                DragTitle(Pane::Meter),
                BackgroundColor(Color::srgba(0.20, 0.12, 0.04, 0.85)),
                Node {
                    height: Val::Px(24.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::horizontal(Val::Px(6.0)),
                    ..default()
                },
            ))
            .with_children(|header| {
                header.spawn(text(f, "Damage meter", 14.0, GOLD));
                button(header, f, Click::CloseMeter, "X", 24.0);
            });
            ui.spawn((
                MeterValue::Summary,
                text(
                    f,
                    format!("{total_dps:.1} DPS   |   {total_damage:.0} damage"),
                    12.0,
                    Color::WHITE,
                ),
            ));
            ui.spawn(text(
                f,
                "DPS: last 5s   •   Click a spell for details",
                10.0,
                GREY,
            ));
            if rows.is_empty() {
                ui.spawn(text(f, "Waiting for damage...", 13.0, GREY));
            }
            ui.spawn((
                MeterRows,
                Interaction::default(),
                bevy::ui::RelativeCursorPosition::default(),
                ScrollPosition(Vec2::new(0.0, view.meter_scroll)),
                Node {
                    width: Val::Percent(100.0),
                    max_height: Val::Px(196.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
            ))
            .with_children(|list| {
                for row in &rows {
                    let (name, spell) = identity(&run, &row.key, spells.as_deref());
                    let selected = view.meter_selected.as_deref() == Some(row.key.as_str());
                    list.spawn((
                        Button,
                        MeterSkill(row.key.clone()),
                        BorderColor::all(if selected {
                            GOLD
                        } else {
                            Color::srgb(0.22, 0.19, 0.14)
                        }),
                        BackgroundColor(Color::srgba(0.04, 0.035, 0.025, 0.9)),
                        Node {
                            height: Val::Px(28.0),
                            flex_shrink: 0.0,
                            width: Val::Percent(100.0),
                            border: UiRect::all(Val::Px(1.0)),
                            padding: UiRect::all(Val::Px(1.0)),
                            column_gap: Val::Px(5.0),
                            align_items: AlignItems::Center,
                            overflow: Overflow::clip(),
                            ..default()
                        },
                    ))
                    .with_children(|line| {
                        line.spawn((
                            MeterBar(row.key.clone()),
                            BackgroundColor(hud::rgb(run.class.color).with_alpha(0.30)),
                            bevy::ui::FocusPolicy::Pass,
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(0.0),
                                top: Val::Px(0.0),
                                width: Val::Percent((row.dps / maximum * 100.0) as f32),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                        ));
                        icon(line, spell, 22.0);
                        line.spawn(Node {
                            flex_direction: FlexDirection::Column,
                            flex_grow: 1.0,
                            min_width: Val::Px(0.0),
                            overflow: Overflow::clip(),
                            ..default()
                        })
                        .with_children(|labels| {
                            labels.spawn((
                                TextLayout::new_with_no_wrap(),
                                text(f, name, 11.0, if selected { GOLD } else { Color::WHITE }),
                            ));
                            labels.spawn((
                                MeterValue::Damage(row.key.clone()),
                                text(
                                    f,
                                    format!(
                                        "{:.0} damage   •   {:.1}%",
                                        row.total,
                                        row.total / total_damage.max(f64::MIN_POSITIVE) * 100.0
                                    ),
                                    9.0,
                                    GREY,
                                ),
                            ));
                        });
                        line.spawn((
                            MeterValue::Dps(row.key.clone()),
                            text(f, format!("{:.1}", row.dps), 12.0, GOLD),
                        ));
                    });
                }
            });
        });
    let Some(selected) = view.meter_selected.as_deref() else {
        return;
    };
    let (name, spell) = identity(&run, selected, spells.as_deref());
    let size = windows.single().map_or(Vec2::new(1280.0, 720.0), |w| {
        Vec2::new(w.width(), w.height()) / scale.as_deref().map_or(1.0, |s| s.0)
    });
    let width = 400.0_f32.min(size.x);
    let left = (view.position.x - width - 12.0).clamp(0.0, (size.x - width).max(0.0));
    commands
        .spawn((
            DetailsRoot,
            GlobalZIndex(480),
            Interaction::default(),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(left),
                top: Val::Px(view.position.y.min((size.y - 390.0).max(0.0))),
                width: Val::Px(width),
                padding: UiRect::all(Val::Px(12.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            },
        ))
        .with_children(|ui| {
            skin::backdrop(ui);
            ui.spawn(Node {
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                ..default()
            })
            .with_children(|header| {
                icon(header, spell, 30.0);
                header.spawn((
                    text(f, name, 15.0, GOLD),
                    Node {
                        flex_grow: 1.0,
                        ..default()
                    },
                ));
                button(header, f, Click::CloseMeterDetails, "X", 24.0);
            });
            ui.spawn(text(f, "Effective damage • current expedition", 12.0, GREY));
            ui.spawn((
                DetailsRows,
                Interaction::default(),
                bevy::ui::RelativeCursorPosition::default(),
                ScrollPosition(Vec2::new(0.0, view.details_scroll)),
                Node {
                    width: Val::Percent(100.0),
                    max_height: Val::Px((size.y - 140.0).clamp(100.0, 280.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
            ))
            .with_children(|ui| {
                for (index, (label, value)) in run
                    .meter
                    .details(selected, run.elapsed)
                    .into_iter()
                    .enumerate()
                {
                    ui.spawn((
                        BackgroundColor(if index % 2 == 0 {
                            Color::srgba(0.18, 0.14, 0.07, 0.45)
                        } else {
                            Color::NONE
                        }),
                        Node {
                            width: Val::Percent(100.0),
                            padding: UiRect::axes(Val::Px(4.0), Val::Px(1.0)),
                            justify_content: JustifyContent::SpaceBetween,
                            column_gap: Val::Px(10.0),
                            flex_shrink: 0.0,
                            ..default()
                        },
                    ))
                    .with_children(|row| {
                        row.spawn(text(f, label, 12.0, GREY));
                        row.spawn((
                            MeterValue::Detail(index),
                            text(f, value, 12.0, Color::WHITE),
                        ));
                    });
                }
            });
            ui.spawn(text(
                f,
                "Hits count per target. Passive procs and ticks are not casts.",
                11.0,
                GREY,
            ));
        });
}
