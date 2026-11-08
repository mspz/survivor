//! Categorized combat diagnostics with persistent live values and independent tabs.
use super::*;
use meter::{DiagnosticPage, DiagnosticSection};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Tab {
    #[default]
    Overview,
    Activity,
    Damage,
    Build,
    Notes,
}
impl Tab {
    const ALL: [Self; 5] = [
        Self::Overview,
        Self::Activity,
        Self::Damage,
        Self::Build,
        Self::Notes,
    ];
    fn index(self) -> usize {
        self as usize
    }
    fn label(self, effects: bool) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Activity if effects => "Applications & history",
            Self::Activity => "Casts & procs",
            Self::Damage if effects => "Targets & uptime",
            Self::Damage => "Damage types",
            Self::Build if effects => "Effect details",
            Self::Build => "Build & modifiers",
            Self::Notes => "Measurement & rules",
        }
    }
}
#[derive(Component)]
struct DebugRoot;
#[derive(Component)]
pub(super) struct DebugRows;
#[derive(Component)]
struct Value(usize);
#[derive(Component)]
enum Action {
    Open,
    Close,
    Tab(Tab),
}

pub(super) fn launch_button(ui: &mut ChildSpawnerCommands, font: &Handle<Font>) {
    debug_button(ui, font, "Details - Full debug", Action::Open, false);
}
fn debug_button(
    ui: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    label: &str,
    action: Action,
    selected: bool,
) {
    ui.spawn((
        Button,
        action,
        BackgroundColor(if selected {
            Color::srgb(0.25, 0.17, 0.06)
        } else {
            Color::srgb(0.075, 0.065, 0.045)
        }),
        BorderColor::all(if selected {
            GOLD
        } else {
            Color::srgb(0.32, 0.26, 0.15)
        }),
        Node {
            min_height: Val::Px(32.0),
            flex_shrink: 0.0,
            padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
            border: UiRect::all(Val::Px(1.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
    ))
    .with_children(|ui| {
        ui.spawn(text(
            font,
            label,
            13.0,
            if selected {
                GOLD
            } else {
                Color::srgb(0.88, 0.85, 0.75)
            },
        ));
    });
}
fn interact(
    mut view: ResMut<View>,
    mut selection: ResMut<Selection>,
    actions: Query<(&Interaction, &Action), Changed<Interaction>>,
    panels: Query<&Interaction, With<DebugRoot>>,
) {
    if view.meter_selected.is_none()
        || !matches!(
            view.meter_tab,
            meter_panel::Tab::Damage | meter_panel::Tab::BossDamage | meter_panel::Tab::Buffs
        )
    {
        view.debug_open = false;
    }
    for (interaction, action) in &actions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            Action::Open if view.meter_selected.is_some() => {
                view.debug_open = true;
                view.debug_scroll = 0.0;
                view.debug_tab = Tab::Overview;
            }
            Action::Close => view.debug_open = false,
            Action::Tab(tab) if view.debug_open => {
                view.debug_tab = *tab;
                view.debug_scroll = 0.0;
            }
            _ => {}
        }
        selection.pointer_captured = true;
    }
    if view.debug_open || panels.iter().any(|i| *i != Interaction::None) {
        selection.pointer_captured = true;
    }
}
fn build_sections(run: &Run, key: &str) -> Vec<DiagnosticSection> {
    if let Some(owned) = run.abilities.iter().find(|a| a.def.key == key) {
        let p = run.resolve_spell(owned.def, owned.level);
        let mut identity = DiagnosticSection::new(
            "Current spell and tags",
            vec![
                ("Internal key".into(), key.into()),
                (
                    "Visual ID / Rank".into(),
                    format!("{} / {}", owned.def.spell, owned.level),
                ),
                ("Gameplay tags".into(), p.tags.labels()),
                (
                    "Direct HIT converted to DoT".into(),
                    if p.direct_dot.is_some() { "Yes" } else { "No" }.into(),
                ),
            ],
        );
        identity.note =
            "Current build snapshot. Recorded damage may come from an earlier state."
                .into();
        let power = DiagnosticSection::new(
            "Power and timing",
            vec![
                ("Resolved base HIT".into(), format!("{:.3}", p.dmg)),
                ("DoT per second".into(), format!("{:.3}", p.dot)),
                (
                    "Duration / Cooldown".into(),
                    format!("{:.2} s / {:.2} s", p.duration, p.cooldown),
                ),
                (
                    "Remaining cooldown".into(),
                    format!("{:.2} s", owned.cd.max(0.0)),
                ),
                (
                    "Critical chance / Multiplier".into(),
                    format!("{:.2} % / x{:.3}", p.crit * 100.0, p.crit_multiplier),
                ),
                (
                    "Repeat chance".into(),
                    format!("{:.2} %", p.recast * 100.0),
                ),
            ],
        );
        let geometry = DiagnosticSection::new(
            "Targets and area",
            vec![
                (
                    "Range / Radius".into(),
                    format!("{:.2} yd / {:.2} yd", p.range, p.radius),
                ),
                (
                    "Area size".into(),
                    format!("{:.2} yd²", std::f32::consts::PI * p.radius * p.radius),
                ),
                ("Targets or projectiles".into(), p.count.to_string()),
                (
                    "Chains / Forks".into(),
                    format!("{} / {}", p.chains, p.forks),
                ),
                (
                    "Chain range".into(),
                    format!("{:.2} yd", p.link_range),
                ),
                (
                    "Extra projectile / melee / chain / fork chances".into(),
                    p.extra_chances
                        .iter()
                        .map(|v| format!("{:.1} %", v * 100.0))
                        .collect::<Vec<_>>()
                        .join(" / "),
                ),
            ],
        );
        let mut modifiers = DiagnosticSection::new(
            "Matching modifiers",
            run.spell_modifiers
                .entries()
                .iter()
                .filter(|m| m.filter.matches(key, p.tags))
                .map(|m| (format!("Source 0x{:X}", m.source), m.change.description()))
                .collect(),
        );
        modifiers.note = "These values precede target defenses and hit-time bonuses.".into();
        vec![identity, power, geometry, modifiers]
    } else if let Some(proc) = super::super::procs::views(run)
        .into_iter()
        .find(|p| p.key == key || p.name == key)
    {
        let mut section = DiagnosticSection::new(
            "Triggered effect",
            vec![
                ("Name".into(), proc.name),
                ("Internal key".into(), proc.key.into()),
                ("Visual ID".into(), proc.spell.to_string()),
            ],
        );
        section.note = proc.lines.join("\n\n");
        vec![section]
    } else {
        let mut section =
            DiagnosticSection::new("Combined attribution", vec![("Source".into(), key.into())]);
        section.note = "This row combines hidden effects or pet damage. The Damage types tab shows their observed gameplay tags and origins.".into();
        vec![section]
    }
}
fn bound_value(
    ui: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    value: &str,
    index: &mut usize,
    size: f32,
    color: Color,
    node: Node,
) {
    ui.spawn((Value(*index), text(font, value, size, color), node));
    *index += 1;
}
fn render(
    mut commands: Commands,
    view: Res<View>,
    run: Res<Run>,
    phase: Res<State<Phase>>,
    font: Option<Res<hud::HudFont>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    scale: Option<Res<UiScale>>,
    roots: Query<Entity, With<DebugRoot>>,
    mut values: Query<(&Value, &mut Text)>,
    mut cache: Local<String>,
    mut refreshed: Local<u64>,
) {
    let Some(font) = font else {
        return;
    };
    let show = view.meter
        && view.debug_open
        && view.meter_selected.is_some()
        && *phase.get() != Phase::Menu;
    let size = windows.single().map_or(Vec2::new(1280.0, 720.0), |w| {
        Vec2::new(w.width(), w.height()) / scale.as_deref().map_or(1.0, |s| s.0)
    });
    let context = format!(
        "{show}:{:?}:{:?}:{:?}:{size:?}",
        view.meter_selected, view.meter_tab, view.debug_tab
    );
    if !show {
        if *cache != context {
            for entity in &roots {
                commands.entity(entity).despawn();
            }
            *cache = context;
        }
        return;
    }
    let selected = view.meter_selected.as_deref().unwrap();
    let boss = view.meter_tab == meter_panel::Tab::BossDamage;
    let meter = if boss { &run.boss_meter } else { &run.meter };
    let tick = (run.elapsed * 4.0) as u64;
    if cache.starts_with(&format!("{context}#")) && *refreshed == tick {
        return;
    }
    *refreshed = tick;
    let effects = view.meter_tab == meter_panel::Tab::Buffs;
    let mut page: DiagnosticPage = if effects { run.effect_meter.debug_page(selected, run.elapsed) } else { meter.debug_page(selected, run.elapsed, boss) };
    if !effects { page.sections[3] = build_sections(&run, selected); }
    let sections = &page.sections[view.debug_tab.index()];
    let shape: Vec<_> = sections
        .iter()
        .map(|s| {
            (
                &s.title,
                s.rows.iter().map(|r| &r.0).collect::<Vec<_>>(),
                &s.note,
            )
        })
        .collect();
    let stamp = format!("{context}#{shape:?}");
    let data: Vec<_> = page
        .cards
        .iter()
        .map(|r| &r.1)
        .chain(sections.iter().flat_map(|s| s.rows.iter().map(|r| &r.1)))
        .collect();
    if *cache == stamp {
        for (binding, mut text) in &mut values {
            if let Some(value) = data.get(binding.0) {
                if text.0 != **value {
                    text.0.clone_from(value);
                }
            }
        }
        return;
    }
    *cache = stamp;
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    let width = (size.x - 32.0).clamp(1.0, 1080.0);
    let height = (size.y - 32.0).clamp(1.0, 860.0);
    let (name, spell) = if effects { run.effect_meter.identity(selected) } else { meter_panel::identity(&run, selected, None) };
    commands.spawn((DebugRoot, GlobalZIndex(1100), Interaction::default(), BackgroundColor(Color::srgba(0.0,0.0,0.0,0.72)),
        Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() },
    )).with_children(|overlay| {
        overlay.spawn((BackgroundColor(Color::srgb(0.035,0.03,0.025)), BorderColor::all(Color::srgb(0.65,0.49,0.17)),
            Node { width: Val::Px(width), height: Val::Px(height), max_width: Val::Percent(100.0), padding: UiRect::all(Val::Px(16.0)), border: UiRect::all(Val::Px(2.0)), flex_direction: FlexDirection::Column, row_gap: Val::Px(12.0), overflow: Overflow::clip(), ..default() },
        )).with_children(|ui| {
            ui.spawn(Node { align_items: AlignItems::Center, column_gap: Val::Px(12.0), flex_shrink: 0.0, ..default() }).with_children(|header| {
                if spell != 0 { header.spawn((ImageNode::default(), hud::SpellIcon(spell), Node { width: Val::Px(42.0), height: Val::Px(42.0), flex_shrink: 0.0, ..default() })); }
                header.spawn(Node { flex_grow: 1.0, min_width: Val::Px(0.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() }).with_children(|title| {
                    title.spawn(text(&font.0, name, 21.0, GOLD));
                    title.spawn(text(&font.0, if effects { "Buff / debuff analysis · Allies and enemies" } else if boss { "Combat analysis · Bosses, elites and training dummy" } else { "Combat analysis · All targets" }, 12.0, GREY));
                });
                debug_button(header, &font.0, "Close", Action::Close, false);
            });
            let mut index = 0;
            ui.spawn(Node { flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(8.0), row_gap: Val::Px(8.0), flex_shrink: 0.0, ..default() }).with_children(|cards| {
                let columns = if width < 700.0 { 2.0 } else { 4.0 };
                for (i, (label, value)) in page.cards.iter().enumerate() {
                    cards.spawn((BackgroundColor(Color::srgb(0.065,0.055,0.045)), BorderColor::all(Color::srgb(0.26,0.22,0.14)), Node { width: Val::Px((width-36.0-(columns-1.0)*8.0)/columns), min_width: Val::Px(0.0), padding: UiRect::all(Val::Px(10.0)), border: UiRect::all(Val::Px(1.0)), flex_direction: FlexDirection::Column, row_gap: Val::Px(5.0), ..default() })).with_children(|card| {
                        card.spawn(text(&font.0, label, 11.0, GREY));
                        let color = [GOLD, Color::WHITE, Color::srgb(1.0,0.65,0.35), Color::srgb(0.78,0.6,1.0)][i % 4];
                        bound_value(card, &font.0, value, &mut index, 19.0, color, Node { min_width: Val::Px(0.0), ..default() });
                    });
                }
            });
            ui.spawn(Node { flex_wrap: FlexWrap::Wrap, column_gap: Val::Px(5.0), row_gap: Val::Px(5.0), flex_shrink: 0.0, ..default() }).with_children(|tabs| {
                for tab in Tab::ALL { debug_button(tabs, &font.0, tab.label(effects), Action::Tab(tab), view.debug_tab == tab); }
            });
            ui.spawn((DebugRows, Interaction::default(), bevy::ui::RelativeCursorPosition::default(), ScrollPosition(Vec2::new(0.0,view.debug_scroll)),
                Node { width: Val::Percent(100.0), min_height: Val::Px(0.0), flex_grow: 1.0, flex_basis: Val::Px(0.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(10.0), overflow: Overflow::scroll_y(), padding: UiRect::right(Val::Px(8.0)), ..default() },
            )).with_children(|body| {
                for section in sections {
                    body.spawn((BackgroundColor(Color::srgb(0.055,0.047,0.037)), BorderColor::all(Color::srgb(0.23,0.19,0.12)), Node { width: Val::Percent(100.0), flex_shrink: 0.0, border: UiRect::all(Val::Px(1.0)), padding: UiRect::all(Val::Px(12.0)), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..default() })).with_children(|group| {
                        group.spawn(text(&font.0, &section.title, 15.0, GOLD));
                        for (i, (label, value)) in section.rows.iter().enumerate() {
                            group.spawn((BackgroundColor(if i % 2 == 0 { Color::srgba(1.0,0.9,0.6,0.025) } else { Color::NONE }), Node { width: Val::Percent(100.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)), column_gap: Val::Px(12.0), align_items: AlignItems::Start, ..default() })).with_children(|row| {
                                row.spawn((text(&font.0, label, 13.0, Color::srgb(0.7,0.68,0.61)), Node { width: Val::Percent(43.0), min_width: Val::Px(0.0), ..default() }));
                                bound_value(row, &font.0, value, &mut index, 13.0, Color::srgb(0.94,0.92,0.85), Node { flex_grow: 1.0, flex_basis: Val::Px(0.0), min_width: Val::Px(0.0), ..default() });
                            });
                        }
                        if !section.note.is_empty() { group.spawn((text(&font.0, &section.note, 12.0, GREY), Node { width: Val::Percent(100.0), margin: UiRect::top(Val::Px(5.0)), ..default() })); }
                    });
                }
            });
            ui.spawn(text(&font.0, "Since last reset · Live updates · Esc to return", 11.0, GREY));
        });
    });
}
pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        interact.after(meter_panel::interact).before(render_meter),
    )
    .add_systems(Update, render.after(render_meter));
}

#[cfg(test)]
mod tests {
    use super::*;
    fn press(app: &mut App, wanted: impl Fn(&Action) -> bool) {
        let world = app.world_mut();
        let entity = world
            .query::<(Entity, &Action)>()
            .iter(world)
            .find(|(_, a)| wanted(a))
            .unwrap()
            .0;
        world.entity_mut(entity).insert(Interaction::Pressed);
        app.update();
    }
    fn has_text(app: &mut App, value: &str) -> bool {
        let world = app.world_mut();
        world
            .query::<&Text>()
            .iter(world)
            .any(|text| text.0.contains(value))
    }
    #[test]
    fn buffs_open_categorized_effect_debug_and_return_to_summary() {
        let mut run = Run::default();
        run.effect_meter = super::super::buff_meter::EffectMeter::fixture();
        let key = run.effect_meter.rows(0.0)[0].key.clone();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(State::new(Phase::Playing))
            .insert_resource(run)
            .insert_resource(View {
                meter: true,
                meter_tab: meter_panel::Tab::Buffs,
                meter_selected: Some(key),
                ..default()
            })
            .init_resource::<Selection>()
            .insert_resource(hud::HudFont(Handle::default()))
            .add_systems(Update, (meter_panel::interact, interact, render_meter, render).chain());
        app.update();
        press(&mut app, |a| matches!(a, Action::Open));
        assert!(has_text(&mut app, "Effect overview"));
        assert!(has_text(&mut app, "Applications & history"));
        for (tab, text) in [
            (Tab::Activity, "Recent effect events"),
            (Tab::Damage, "Host / self"),
            (Tab::Build, "Observed effect metadata"),
            (Tab::Notes, "How effects are measured"),
        ] {
            press(&mut app, |a| matches!(a, Action::Tab(t) if *t == tab));
            assert!(has_text(&mut app, text));
        }
        press(&mut app, |a| matches!(a, Action::Close));
        assert!(!app.world().resource::<View>().debug_open);
    }

    #[test]
    fn categorized_debug_tabs_keep_live_values_scroll_and_close_to_summary() {
        let mut run = Run::default();
        run.elapsed = 1.0;
        run.abilities.push(super::super::super::Owned {
            def: data::ability("fireball"),
            level: 3,
            cd: 0.5,
        });
        run.meter.cast("fireball");
        run.meter.debug_cast("fireball", false, false);
        run.meter.record_hit("fireball", 1.0, 42.0, true, false);
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(State::new(Phase::Playing))
            .insert_resource(run)
            .insert_resource(View {
                meter: true,
                meter_selected: Some("fireball".into()),
                ..default()
            })
            .init_resource::<Selection>()
            .insert_resource(hud::HudFont(Handle::default()))
            .add_systems(
                Update,
                (meter_panel::interact, interact, render_meter, render).chain(),
            );
        app.update();
        press(&mut app, |a| matches!(a, Action::Open));
        assert!(has_text(&mut app, "42.000"));
        assert!(has_text(&mut app, "Damage breakdown"));
        assert!(!has_text(&mut app, "OBSERVED SINCE RESET"));
        let world = app.world_mut();
        let root = world
            .query_filtered::<&GlobalZIndex, With<DebugRoot>>()
            .single(world)
            .unwrap();
        assert!(root.0 > 1000, "Modal must cover HUD tooltips");
        assert_eq!(
            world
                .query::<&Action>()
                .iter(world)
                .filter(|a| matches!(a, Action::Tab(_)))
                .count(),
            5
        );
        let scroll = world
            .query_filtered::<Entity, With<DebugRows>>()
            .single(world)
            .unwrap();
        world.get_mut::<ScrollPosition>(scroll).unwrap().0.y = 150.0;
        world.resource_mut::<Run>().elapsed = 2.0;
        world
            .resource_mut::<Run>()
            .meter
            .record_hit("fireball", 2.0, 8.0, false, true);
        app.update();
        assert!(has_text(&mut app, "50.000"));
        assert_eq!(
            app.world().get::<ScrollPosition>(scroll).unwrap().0.y,
            150.0
        );
        for (tab, category) in [
            (Tab::Activity, "Per-trigger efficiency"),
            (Tab::Damage, "Damage channels"),
            (Tab::Build, "Matching modifiers"),
            (Tab::Notes, "How to read these numbers"),
        ] {
            press(&mut app, |a| matches!(a, Action::Tab(t) if *t == tab));
            assert_eq!(app.world().resource::<View>().debug_tab, tab);
            assert!(has_text(&mut app, category));
            assert_eq!(app.world().resource::<View>().debug_scroll, 0.0);
        }
        press(&mut app, |a| matches!(a, Action::Close));
        let world = app.world_mut();
        assert!(!world.resource::<View>().debug_open);
        assert_eq!(
            world.resource::<View>().meter_selected.as_deref(),
            Some("fireball")
        );
        assert_eq!(world.query::<&DebugRoot>().iter(world).count(), 0);
        assert_eq!(
            world
                .query::<&meter_panel::DetailsRoot>()
                .iter(world)
                .count(),
            1
        );
    }
}
