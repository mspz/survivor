use super::*;

#[test]
fn learned_procs_have_stable_hud_icons_live_tooltips_and_disappear_on_respec() {
    let mut app = app();
    {
        let mut run = app.world_mut().resource_mut::<Run>();
        run.class = &data::CLASSES[0];
        run.talent_build[1][8] = 1;
        super::super::talent_runtime::configure(&mut run);
    }
    app.update();
    let tip = TipFor::Proc("proc_bleed");
    let icon = slot(app.world_mut(), tip);
    app.world_mut()
        .entity_mut(icon)
        .insert(Interaction::Hovered);
    app.update();
    assert!(visible(app.world_mut()));
    let before = contents(app.world_mut());
    assert!(before.contains("Deep Wounds"));
    assert!(before.contains("Per 100 triggering hit damage"));
    assert!(before.contains("Damage over time:"));
    app.world_mut()
        .resource_mut::<Run>()
        .replace_spell_modifiers(
            999,
            [super::super::spell_engine::Modifier {
                source: 999,
                filter: super::super::spell_engine::Filter::default(),
                change: super::super::spell_engine::Change::IncreasedDotDamage(1.0),
            }],
        );
    app.update();
    assert_eq!(slot(app.world_mut(), tip), icon);
    assert!(visible(app.world_mut()));
    assert_ne!(contents(app.world_mut()), before);
    {
        let mut run = app.world_mut().resource_mut::<Run>();
        run.talent_build[1][8] = 0;
        super::super::talent_runtime::configure(&mut run);
        assert_eq!(run.abilities.len(), 1);
    }
    app.update();
    assert!(!app
        .world_mut()
        .query::<&TipFor>()
        .iter(app.world())
        .any(|p| matches!(p, TipFor::Proc(_))));
    assert!(!visible(app.world_mut()));
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
        .init_state::<Phase>()
        .insert_resource(Run {
            abilities: vec![Owned {
                def: data::ability("fireball"),
                level: 1,
                cd: 0.0,
            }],
            passives: vec![(&data::PASSIVES[0], 1)],
            ..default()
        })
        .init_resource::<Selection>()
        .insert_resource(HudFont(Handle::default()))
        .init_resource::<UiScale>()
        .init_resource::<Assets<Image>>()
        .init_resource::<Assets<SweepUiMaterial>>()
        .add_systems(Update, (refresh_hud, icon_tooltip).chain());
    app.world_mut().spawn((AbilityRow, Node::default()));
    app.world_mut().spawn((PassiveRow, Node::default()));
    app.world_mut()
        .resource_mut::<NextState<Phase>>()
        .set(Phase::Playing);
    app.update();
    app
}
fn slot(world: &mut World, tip: TipFor) -> Entity {
    world
        .query::<(Entity, &TipFor)>()
        .iter(world)
        .find(|(_, value)| **value == tip)
        .unwrap()
        .0
}
fn contents(world: &mut World) -> String {
    world
        .query::<&Text>()
        .iter(world)
        .map(|t| t.0.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}
fn visible(world: &mut World) -> bool {
    *world
        .query_filtered::<&Visibility, With<GameTooltip>>()
        .single(world)
        .unwrap()
        == Visibility::Inherited
}

#[test]
fn spell_and_passive_tooltips_survive_continuous_version_updates() {
    for tip in [TipFor::Ability(0), TipFor::Passive(0)] {
        let mut app = app();
        let icon = slot(app.world_mut(), tip);
        app.world_mut()
            .entity_mut(icon)
            .insert(Interaction::Hovered);
        app.update();
        assert!(visible(app.world_mut()));
        let before = contents(app.world_mut());
        assert!(!before.is_empty());
        let original_texts: Vec<_> = app
            .world_mut()
            .query_filtered::<Entity, With<Text>>()
            .iter(app.world())
            .collect();
        for _ in 0..4 {
            app.world_mut().resource_mut::<Run>().version += 1;
            app.update();
            assert_eq!(
                slot(app.world_mut(), tip),
                icon,
                "Unchanged build icons must keep their hover state"
            );
            assert!(visible(app.world_mut()));
            for &text in &original_texts {
                assert!(
                    app.world().get_entity(text).is_ok(),
                    "Unchanged tooltip content must not be rebuilt"
                );
            }
        }
        assert_eq!(contents(app.world_mut()), before);
        app.world_mut().entity_mut(icon).insert(Interaction::None);
        app.update();
        assert!(!visible(app.world_mut()));
    }
}

#[test]
fn hovered_spell_updates_its_values_without_disappearing_and_hides_in_menu() {
    let mut app = app();
    let icon = slot(app.world_mut(), TipFor::Ability(0));
    app.world_mut()
        .entity_mut(icon)
        .insert(Interaction::Hovered);
    app.update();
    let before = contents(app.world_mut());
    app.world_mut()
        .resource_mut::<Run>()
        .replace_spell_modifiers(
            42,
            [super::super::spell_engine::Modifier {
                source: 42,
                filter: default(),
                change: super::super::spell_engine::Change::IncreasedDamage(0.5),
            }],
        );
    app.update();
    assert!(visible(app.world_mut()));
    assert_eq!(slot(app.world_mut(), TipFor::Ability(0)), icon);
    assert_ne!(contents(app.world_mut()), before);
    app.world_mut()
        .resource_mut::<NextState<Phase>>()
        .set(Phase::Menu);
    app.update();
    assert!(!visible(app.world_mut()));
}
