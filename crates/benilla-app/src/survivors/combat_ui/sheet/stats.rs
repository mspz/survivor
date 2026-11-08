//! Compact, collapsible statistic groups. Values use the combat calculations.
use super::*;
use crate::survivors::items::{Effect, Inventory, SLOTS};
use bevy::{
    input::mouse::{MouseScrollUnit, MouseWheel},
    ui::RelativeCursorPosition,
};

#[derive(Component)]
pub(super) struct CategoryText(pub u8, pub usize, pub bool);
#[derive(Component)]
pub(super) struct StatLine;
#[derive(Component)]
pub(super) struct StatTooltip;
#[derive(Component)]
pub(super) struct StatScroll;
const CATEGORIES: [&str; 6] = [
    "Attributs",
    "Défense",
    "Offensive",
    "Récupération",
    "Utilitaire",
    "Sorts",
];

pub(super) fn primary(inventory: Option<&Inventory>) -> [f32; 4] {
    let mut values = [0.0; 4];
    if let Some(inventory) = inventory {
        for (slot, _) in SLOTS {
            if let Some(item) = inventory.equipped(slot) {
                for affix in &item.affixes {
                    let index = match affix.effect {
                        Effect::Stamina => 0,
                        Effect::Strength => 1,
                        Effect::Agility => 2,
                        Effect::Intelligence => 3,
                        _ => continue,
                    };
                    values[index] += affix.value;
                }
            }
        }
    }
    values
}

pub(super) fn category_text(
    category: u8,
    run: &Run,
    inventory: Option<&Inventory>,
    selected: usize,
) -> String {
    let s = run.stats();
    match category {
        0=>{let p=primary(inventory);format!("Stamina: {:.0}\nStrength: {:.0}\nAgility: {:.0}\nIntelligence: {:.0}",p[0],p[1],p[2],p[3])},
        1=>{let mut text=format!("Health: {:.0} / {:.0}\nArmour: {:.0}\nArmour reduction: {:.1}%\nFlat reduction: {:.1}%\nDamage reduction: {:.1}%\nDamage taken: {:.1}%\nEffective health: {:.0}\nBlock chance: {:.1}%",run.hp,s.max_hp,s.armor,crate::survivors::items::armour::reduction(s.armor)*100.0,run.equipment.reduction.clamp(0.0,0.75)*100.0,run.damage_reduction()*100.0,(1.0-run.damage_reduction())*100.0,s.max_hp/(1.0-run.damage_reduction()).max(0.0001),run.equipment.block.clamp(0.0,0.75)*100.0); text.push_str(&format!("\nDodge chance: {:.1}%", crate::survivors::extra_spells::dodge_chance(run) * 100.0)); for (tag, label, total) in s.resistances.entries() { text.push_str(&format!("\n{label}: {:.1}% (total: {total:.0}%, cap: 75%)", s.resistances.reduction(tag)*100.0)); } text },
        2=>{
            let mut rows=Vec::new();
            for (kind,label) in [(Tags::ATTACK,"Attack"),(Tags::SPELL,"Spell")] {
                let owned=run.abilities.get(selected).filter(|o|run.resolve_spell(o.def,o.level).tags.contains(kind|Tags::DAMAGE))
                    .or_else(||run.abilities.iter().find(|o|run.resolve_spell(o.def,o.level).tags.contains(kind|Tags::DAMAGE)));
                if let Some(owned)=owned {
                    let p=run.resolve_spell(owned.def,owned.level);
                    let damage=if owned.def.kind==data::Kind::Dot {format!("Damage over time: {:.1}/s",p.dot)}else{format!("Damage: {:.1}",p.dmg)};
                    let no_crit=owned.def.kind==data::Kind::Dot || p.tags.contains(Tags::GUARD);
                    let crit=if no_crit {"-".into()}else{format!("{:.1}%",p.crit*100.0)};
                    let crit_damage=if no_crit {"-".into()}else{format!("{:.0}%",p.crit_multiplier*100.0)};
                    rows.push(format!("{label}: {}\n{damage}\nCritical chance: {crit}\nCritical damage: {crit_damage}\nCooldown: {:.2} s",owned.def.key.replace('_'," "),p.cooldown));
                }
            }
            let melee=run.weapon_damage_range(data::ability("heroic_strike"));
            let ranged=run.weapon_damage_range(data::ability("arcane_shot"));
            rows.push(format!("Melee weapon: {:.0}-{:.0}\nRanged weapon: {:.0}-{:.0}",melee.min,melee.max,ranged.min,ranged.max));
            rows.join("\n")
        },
        3=>format!("Regeneration: {:.1} HP/s\nLife on kill: {:.1}\nHealing received: {:+.1}%\nAbsorption: {:.0}",s.regen*(1.0+run.equipment.healing).max(0.0),run.equipment.life_on_kill*(1.0+run.equipment.healing).max(0.0),run.equipment.healing*100.0,if run.shield_t>0.0{run.shield}else{0.0}),
        4=>format!("Movement speed: {:.1} yd/s\nPickup radius: {:.1} yd\nExperience: {:+.1}%\nLuck: {:+.1}%\nMagic Find: {:+.1}%\nRun level: {}",s.speed,s.magnet,(s.growth-1.0)*100.0,run.equipment.luck*100.0,crate::survivors::items::compensated_magic_find(run.equipment.magic_find)*100.0,run.level),
        _=>spell_stats(run,selected),
    }
}

pub(super) fn draw(
    panel: &mut ChildSpawnerCommands,
    font: &Handle<Font>,
    run: &Run,
    view: &View,
    height: f32,
) {
    panel
        .spawn((
            StatScroll,
            RelativeCursorPosition::default(),
            Interaction::default(),
            ScrollPosition(Vec2::new(0.0, view.stats_scroll)),
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                overflow: Overflow::scroll_y(),
                ..rect(16.0, 45.0, 340.0, height - 65.0)
            },
        ))
        .with_children(|list| {
            for (category, name) in CATEGORIES.iter().enumerate() {
                let collapsed = view.stats_collapsed & (1 << category) != 0;
                list.spawn((
                    Button,
                    Click::ToggleStats(category as u8),
                    BackgroundColor(Color::srgb(0.07, 0.06, 0.04)),
                    BorderColor::all(Color::srgb(0.32, 0.28, 0.19)),
                    Node {
                        border: UiRect::all(Val::Px(1.0)),
                        padding: UiRect::axes(Val::Px(9.0), Val::Px(5.0)),
                        width: Val::Percent(100.0),
                        min_height: Val::Px(31.0),
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                ))
                .with_children(|header| {
                    skin::backdrop(header);
                    header.spawn(text(font, *name, 14.0, GOLD));
                    header.spawn(text(font, if collapsed { "+" } else { "-" }, 16.0, GOLD));
                });
                if collapsed {
                    continue;
                }
                if category == 5 {
                    list.spawn(Node {
                        width: Val::Percent(100.0),
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: Val::Px(5.0),
                        row_gap: Val::Px(4.0),
                        ..default()
                    })
                    .with_children(|icons| {
                        let procs = super::super::super::procs::views(run);
                        let icons_list = run
                            .abilities
                            .iter()
                            .map(|o| o.def.spell)
                            .chain(procs.iter().map(|p| p.spell));
                        for (index, spell) in icons_list.enumerate() {
                            icons
                                .spawn((
                                    Button,
                                    Click::Skill(index),
                                    BorderColor::all(if index == view.selected {
                                        GOLD
                                    } else {
                                        GREY
                                    }),
                                    Node {
                                        width: Val::Px(32.0),
                                        height: Val::Px(32.0),
                                        border: UiRect::all(Val::Px(1.0)),
                                        ..default()
                                    },
                                ))
                                .with_children(|icon| {
                                    icon.spawn((
                                        ImageNode::default(),
                                        hud::SpellIcon(spell),
                                        Node {
                                            width: Val::Percent(100.0),
                                            height: Val::Percent(100.0),
                                            ..default()
                                        },
                                    ));
                                });
                        }
                    });
                    list.spawn((SkillTitle, text(font, "", 14.0, GOLD)));
                    list.spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        flex_shrink: 0.0,
                        ..default()
                    })
                    .with_children(|list| {
                        for (index, line) in spell_stats(run, view.selected).lines().enumerate() {
                            list.spawn((
                                StatsText(index),
                                StatLine,
                                Interaction::default(),
                                text(font, line, 13.0, Color::WHITE),
                                Node {
                                    width: Val::Percent(100.0),
                                    flex_shrink: 0.0,
                                    ..default()
                                },
                            ));
                        }
                    });
                } else {
                    list.spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        flex_shrink: 0.0,
                        ..default()
                    })
                    .with_children(|list| {
                        for (index, line) in category_text(category as u8, run, None, view.selected)
                            .lines()
                            .enumerate()
                        {
                            let (label, value) = line.split_once(':').unwrap_or((line, ""));
                            list.spawn((
                                BackgroundColor(if index % 2 == 0 {
                                    Color::srgb(0.045, 0.041, 0.033)
                                } else {
                                    Color::srgb(0.025, 0.023, 0.02)
                                }),
                                Node {
                                    width: Val::Percent(100.0),
                                    min_height: Val::Px(22.0),
                                    flex_shrink: 0.0,
                                    padding: UiRect::axes(Val::Px(7.0), Val::Px(3.0)),
                                    ..default()
                                },
                            ))
                            .with_children(|row| {
                                row.spawn((
                                    CategoryText(category as u8, index, false),
                                    StatLine,
                                    Interaction::default(),
                                    text(
                                        font,
                                        format!("{label}:"),
                                        12.0,
                                        Color::srgb(0.82, 0.73, 0.5),
                                    ),
                                    Node {
                                        width: Val::Percent(48.0),
                                        flex_shrink: 0.0,
                                        ..default()
                                    },
                                ));
                                row.spawn((
                                    CategoryText(category as u8, index, true),
                                    text(font, value.trim(), 12.0, Color::srgb(0.95, 0.93, 0.86)),
                                    TextLayout::new_with_justify(Justify::Right),
                                    Node {
                                        width: Val::Percent(52.0),
                                        flex_shrink: 0.0,
                                        ..default()
                                    },
                                ));
                            });
                        }
                    });
                }
            }
        });
}
pub(super) fn scroll(
    mut wheel: MessageReader<MouseWheel>,
    mut view: ResMut<View>,
    mut nodes: Query<
        (&ComputedNode, &RelativeCursorPosition, &mut ScrollPosition),
        With<StatScroll>,
    >,
) {
    for event in wheel.read() {
        let amount = match event.unit {
            MouseScrollUnit::Line => event.y * 28.0,
            MouseScrollUnit::Pixel => event.y,
        };
        for (node, cursor, mut position) in &mut nodes {
            if !cursor.cursor_over() {
                continue;
            }
            position.0.y = (position.0.y - amount).clamp(
                0.0,
                ((node.content_size.y - node.size.y) * node.inverse_scale_factor).max(0.0),
            );
            view.stats_scroll = position.0.y;
        }
    }
}

fn explanation(label: &str) -> Option<&'static str> {
    Some(match label {
        "Stamina" => "Increases your maximum health.",
        "Strength" => "Boosts attack damage for Warriors and Paladins.",
        "Agility" => "Boosts attack damage for Hunters, Rogues, Shamans and Druids.",
        "Intelligence" => "Increases spell damage, healing and shields for caster classes.",
        "Health" => "Your current health and the maximum you can recover.",
        "Armour" => "Reduces incoming damage, with smaller gains at higher values.",
        "Armour reduction" => "The percentage of Physical damage prevented by armour.",
        "Fire resistance" | "Cold resistance" | "Lightning resistance" | "Nature resistance" | "Arcane resistance" | "Holy resistance" | "Shadow resistance" => "Adds resistance percentages from equipment, up to 75%. Reduces hits and damage over time of the matching element; unaffected by armour penetration.",
        "Flat reduction" => "Reduces the damage remaining after armour.",
        "Damage reduction" => "Combined protection against Physical damage. Elemental damage uses its matching resistance.",
        "Damage taken" => "The percentage of incoming damage left after reductions.",
        "Effective health" => "The raw damage your full health can withstand after reductions.",
        "Dodge chance" => "Chance to avoid physical and magical damage, including boss areas. Capped at 75%.",
        "Block chance" => "Your chance to completely block an incoming melee hit.",
        "Attack" => "The attack skill used to preview your offensive stats.",
        "Spell" => "The spell used to preview your offensive stats.",
        "Damage" => "Damage dealt by one hit before the target's defenses.",
        "Damage over time" => "Damage dealt each second while the effect lasts.",
        "Critical chance" => "Your chance for a hit to deal critical damage.",
        "Critical damage" => "The damage a critical hit deals compared with a normal hit.",
        "Cooldown" => "The time between uses of this skill.",
        "Melee weapon" => "Base damage from your equipped melee weapons.",
        "Ranged weapon" => "Base damage from your equipped ranged weapon.",
        "Regeneration" => "Health automatically restored each second.",
        "Life on kill" => "Health restored when you kill an enemy.",
        "Healing received" => "Increases the health restored by healing effects.",
        "Absorption" => "Damage your shield absorbs before you lose health.",
        "Movement speed" => "The distance you can move each second.",
        "Pickup radius" => "How close you must be to automatically collect pickups.",
        "Experience" => "Increases experience gained during the run.",
        "Luck" => "Improves your chances of receiving higher-rarity rewards.",
        "Magic Find" => {
            "Improves item quality chances. The displayed value includes doubled effectiveness for both boss and ordinary monster loot. Does not affect drop frequency, affix selection or value rolls."
        }
        "Run level" => "Your current level in this expedition.",
        "Healing" => "Health restored by this skill.",
        "Damage bonus" => "Increases damage from stats and compatible bonuses.",
        "Healing bonus" => "Increases the amount of health restored by this skill.",
        "Absorption bonus" => "Increases the amount of damage this shield absorbs.",
        "Flat damage" => "Extra damage added before percentage bonuses.",
        "More damage" => "Multiplies damage after your other damage bonuses.",
        "Cooldown reduction" => "How much shorter this skill's cooldown is than its base cooldown.",
        "Cooldown recovery" => "Makes skills recover faster so they can be used more often.",
        "Attack speed" => "Makes attack skills strike more often.",
        "Targets" => "The number of targets or projectiles in the initial cast.",
        "Range" => "The maximum distance at which this skill can reach an enemy.",
        "Radius" => "The distance an area effect reaches from its center.",
        "Duration" => "How long this skill's effect lasts.",
        "Chains" => "How many times a hit can jump to another enemy.",
        "Forks" => "How many times projectiles can split after hitting an enemy.",
        "Repeat chance" => "The chance to cast this skill again without waiting for its cooldown.",
        "Life leech" => "Restores health from a portion of attack damage dealt.",
        "Armour penetration" => "Lets physical damage ignore part of the enemy's armour.",
        "Boss damage" => "Increases damage against bosses.",
        "Full-health damage" => "Increases damage against enemies at full health.",
        "Low-health damage" => "Increases damage against enemies below 35% health.",
        "Bleed chance" => "The chance for physical attacks to cause bleeding.",
        "Ignite chance" => "The chance for Fire hits to burn enemies over time.",
        "Chill chance" => "The chance for Frost hits to slow enemies.",
        "Knockback chance" => "The chance for projectile hits to push enemies away.",
        _ => return None,
    })
}

pub(super) fn tooltip(
    mut commands: Commands,
    selection: Res<Selection>,
    font: Option<Res<hud::HudFont>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    scale: Option<Res<UiScale>>,
    lines: Query<(Entity, &Interaction, &Text), With<StatLine>>,
    mut roots: Query<(Entity, &mut Node), With<StatTooltip>>,
    mut shown: Local<Option<(Entity, String)>>,
) {
    let hovered = selection
        .sheet_open
        .then(|| {
            lines.iter().find_map(|(entity, interaction, text)| {
                if *interaction == Interaction::None {
                    return None;
                }
                let label = text.0.split_once(':')?.0;
                Some((entity, label.to_owned(), explanation(label)?))
            })
        })
        .flatten();
    let (Some((entity, label, effect)), Some(font)) = (hovered, font) else {
        for (entity, _) in &mut roots {
            commands.entity(entity).despawn();
        }
        *shown = None;
        return;
    };
    let (viewport, cursor) = windows.single().map_or(
        (Vec2::new(1365.0, 768.0), Vec2::new(800.0, 400.0)),
        |window| {
            let scale = scale.as_deref().map_or(1.0, |s| s.0).max(0.01);
            (
                Vec2::new(window.width(), window.height()) / scale,
                window.cursor_position().unwrap_or(Vec2::ZERO) / scale,
            )
        },
    );
    let width = 280.0_f32.min((viewport.x - 16.0).max(1.0));
    let x = if cursor.x > viewport.x * 0.5 {
        cursor.x - width - 18.0
    } else {
        cursor.x + 18.0
    };
    let left = Val::Px(x.clamp(8.0, (viewport.x - width - 8.0).max(8.0)));
    let top = Val::Px((cursor.y + 14.0).clamp(8.0, (viewport.y - 104.0).max(8.0)));
    if shown.as_ref() == Some(&(entity, label.clone())) && !roots.is_empty() {
        for (_, mut node) in &mut roots {
            node.left = left;
            node.top = top;
        }
        return;
    }
    for (entity, _) in &mut roots {
        commands.entity(entity).despawn();
    }
    *shown = Some((entity, label.clone()));
    commands
        .spawn((
            StatTooltip,
            GlobalZIndex(650),
            BackgroundColor(Color::srgb(0.02, 0.025, 0.04)),
            BorderColor::all(Color::srgb(0.45, 0.45, 0.45)),
            Node {
                position_type: PositionType::Absolute,
                left,
                top,
                width: Val::Px(width),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(2.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(5.0),
                ..default()
            },
        ))
        .with_children(|ui| {
            ui.spawn(text(&font.0, label, 14.0, GOLD));
            ui.spawn(text(&font.0, effect, 14.0, Color::WHITE));
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    #[test]
    fn stat_hover_shows_one_short_explanation_and_closing_the_sheet_hides_it() {
        let mut world = World::new();
        world.insert_resource(Selection {
            sheet_open: true,
            ..default()
        });
        world.insert_resource(hud::HudFont(Handle::default()));
        let row = world
            .spawn((
                StatLine,
                Interaction::Hovered,
                Text::new("Critical chance: 5.0%"),
            ))
            .id();
        world.run_system_once(tooltip).unwrap();
        let root = world
            .query_filtered::<Entity, With<StatTooltip>>()
            .single(&world)
            .unwrap();
        let children = world.get::<Children>(root).unwrap();
        let lines: Vec<_> = children
            .iter()
            .filter_map(|child| world.get::<Text>(child).map(|t| t.0.as_str()))
            .collect();
        assert_eq!(
            lines,
            [
                "Critical chance",
                "Your chance for a hit to deal critical damage."
            ]
        );
        world.entity_mut(row).insert(Interaction::None);
        world.run_system_once(tooltip).unwrap();
        assert_eq!(world.query::<&StatTooltip>().iter(&world).count(), 0);
        world.entity_mut(row).insert(Interaction::Hovered);
        world.run_system_once(tooltip).unwrap();
        world.resource_mut::<Selection>().sheet_open = false;
        world.run_system_once(tooltip).unwrap();
        assert_eq!(world.query::<&StatTooltip>().iter(&world).count(), 0);
    }
    #[test]
    fn the_equipped_rune_shows_in_offense_and_utility() {
        use crate::survivors::runes::{self, Rune, RuneStat, Runes, Slot};
        let stat = |id: &str, value: f32, bonus: f32| RuneStat {
            id: id.into(),
            value,
            bonus,
        };
        let mut runes = Runes::default();
        runes.save.slots = vec![Slot {
            rune: Some(Rune {
                rarity: 0,
                level: 2,
                stats: vec![
                    stat("crit_damage", 30.0, 10.0),
                    stat("movement_speed", 10.0, 0.0),
                    stat("experience", 20.0, 0.0),
                ],
            }),
        }];
        runes.save.equipped = Some(0);
        runes.revision = 1;
        let mut run = Run::default();
        run.abilities.push(crate::survivors::Owned {
            def: data::ability("heroic_strike"),
            level: 1,
            cd: 0.0,
        });
        let offense = category_text(2, &run, None, 0);
        assert!(offense.contains("Critical damage: 200%"), "{offense}");
        let speed = run.stats().speed;
        runes::apply(&runes, &mut run);
        // Roll and upgrade bonus both reach the sheet: 200 % + 30 % + 10 %.
        let offense = category_text(2, &run, None, 0);
        assert!(offense.contains("Critical damage: 240%"), "{offense}");
        let utility = category_text(4, &run, None, 0);
        assert!(
            utility.contains(&format!("Movement speed: {:.1} yd/s", speed * 1.1)),
            "{utility}"
        );
        assert!(utility.contains("Experience: +20.0%"), "{utility}");
    }
    #[test]
    fn defense_shows_the_actual_combined_reduction_instead_of_adding_percentages() {
        let mut run = Run::default();
        run.equipment_armour = 100.0;
        run.equipment.reduction = 0.25;
        run.equipment.resistances.fire = 90.0;
        let text = category_text(1, &run, None, 0);
        assert!(text.contains("Armour reduction: 41.9%"));
        assert!(text.contains("Flat reduction: 25.0%"));
        assert!(text.contains("Damage reduction: 56.4%"));
        assert!(text.contains("Damage taken: 43.6%"));
        assert!(text.contains("Fire resistance: 75.0% (total: 90%, cap: 75%)"));
        let damage = crate::survivors::hero::hurt_hero(&mut run, 100.0);
        assert!((damage - 43.609_413).abs() < 0.001);
        run.equipment.reduction = 0.0;
        assert!(
            (run.damage_reduction() - crate::survivors::items::armour::reduction(100.0)).abs()
                < 0.00001
        );
    }
}
