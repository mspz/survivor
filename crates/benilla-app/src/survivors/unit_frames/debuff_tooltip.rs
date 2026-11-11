use super::*;
use bevy::{ui::FocusPolicy, window::PrimaryWindow};

#[derive(Component)]
pub(super) struct Tooltip;

fn description(spell: u32) -> &'static str {
    match spell {
        116 => "Reduces movement speed.",
        339 => "Roots the target.",
        408 => "Stuns the target, preventing actions.",
        5782 => "The target flees in fear.",
        702 => "Reduces the target's damage dealt.",
        355 => "Forces the target to attack the taunter.",
        _ => data::ABILITIES
            .iter()
            .find(|d| d.spell == spell)
            .and_then(|d| {
                super::super::support_spells::description(d.key)
                    .or_else(|| super::super::extra_spells::description(d.key))
                    .or(if d.kind == data::Kind::Dot {
                        Some("Deals periodic damage to the target.")
                    } else {
                        None
                    })
            })
            .unwrap_or("Active debuff on the target."),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn show(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    target: Res<Target>,
    units: Query<(&Status, Option<&super::super::enemies::Rank>)>,
    icons: Query<(&Interaction, &ui::DebuffIcon)>,
    buffs: Query<(&Interaction, &ui::ModIcon)>,
    roots: Query<Entity, With<Tooltip>>,
    font: Res<hud::HudFont>,
    spells: Option<Res<crate::ui_action::Spells>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    scale: Res<UiScale>,
) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    if !matches!(*phase.get(), Phase::Playing | Phase::Hub | Phase::Paused) {
        return;
    }
    let Some((status,rank)) = target.entity.and_then(|e| units.get(e).ok()) else {
        return;
    };
    let modifier = buffs.iter().find_map(|(interaction,icon)| {
        (*interaction!=Interaction::None && rank.is_some_and(|r|r.has(icon.0)))
            .then(|| super::super::monster_specials::player::mod_aura(icon.0,None))
    });
    let debuff = icons
        .iter()
        .filter(|(i, _)| **i != Interaction::None)
        .find_map(|(_, icon)| {
            status
                .debuffs
                .iter()
                .find(|(spell, left)| *spell == icon.0 && *left > 0.0)
        });
    let (name,effect,duration,is_buff) = if let Some(aura)=modifier {
        (aura.name,aura.description,"Permanent monster modifier".to_owned(),true)
    } else if let Some((spell,left))=debuff {
        let def=data::ABILITIES.iter().find(|d|d.spell==*spell);
        (hud::spell_name(spells.as_deref(),*spell,def.map_or("Debuff",|d|d.key)),
            description(*spell).to_owned(),format!("Time remaining: {left:.1} s"),false)
    } else { return; };
    let inv = scale.0.max(0.01);
    let (viewport, cursor) =
        windows
            .single()
            .map_or((Vec2::new(1280.0, 768.0), Vec2::new(800.0, 400.0)), |w| {
                (
                    Vec2::new(w.width(), w.height()) / inv,
                    w.cursor_position().unwrap_or_default() / inv,
                )
            });
    commands
        .spawn((
            Tooltip,
            GlobalZIndex(850),
            FocusPolicy::Pass,
            BackgroundColor(Color::srgb(0.02, 0.025, 0.04)),
            BorderColor::all(if is_buff {hud::GOLD_TONE} else {Color::srgb(0.9, 0.1, 0.1)}),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px((cursor.x + 18.0).clamp(4.0, (viewport.x - 334.0).max(4.0))),
                top: Val::Px((cursor.y + 22.0).clamp(4.0, (viewport.y - 200.0).max(4.0))),
                width: Val::Px(330.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                ..default()
            },
        ))
        .with_children(|body| {
            for (value, size, color) in [
                (name, 17.0, hud::GOLD_TONE),
                (effect, 14.0, Color::WHITE),
                (duration, 12.0, hud::GOLD_TONE),
            ] {
                body.spawn((
                    Text::new(value),
                    TextFont {
                        font: font.0.clone(),
                        font_size: size,
                        ..default()
                    },
                    TextColor(color),
                    FocusPolicy::Pass,
                ));
            }
        });
}
