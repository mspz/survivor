#[test]
fn monster_mod_buffs_wrap_above_debuffs_and_show_their_own_effect_tooltips() {
    use super::super::super::enemies::Rank;
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
    let rank = Rank::MODS
        .into_iter()
        .take(7)
        .fold(Rank::Normal, |r, m| r.with(m))
        .with(Rank::Echo);
    let target = app
        .world_mut()
        .spawn((
            rank,
            Status {
                name: "Anvilrage Warden".into(),
                health: 100.0,
                maximum: 100.0,
                level: 116,
                friendly: false,
                debuffs: vec![(1943, 4.0)],
                debuff_stacks: vec![(1943, 2)],
            },
        ))
        .id();
    app.world_mut().resource_mut::<Target>().entity = Some(target);
    app.world_mut()
        .resource_mut::<NextState<Phase>>()
        .set(Phase::Playing);
    app.update();
    let icons: Vec<_> = app
        .world_mut()
        .query::<(Entity, &ModIcon)>()
        .iter(app.world())
        .map(|(entity, icon)| (entity, icon.0))
        .collect();
    assert_eq!(
        icons.len(),
        8,
        "each modifier has its own icon, even when they share a WoW spell icon"
    );
    let buffs = app
        .world_mut()
        .query_filtered::<Entity, With<Buffs>>()
        .single(app.world())
        .unwrap();
    let debuffs = app
        .world_mut()
        .query_filtered::<Entity, With<Debuffs>>()
        .single(app.world())
        .unwrap();
    let parent = app.world().get::<ChildOf>(buffs).unwrap().parent();
    assert_eq!(
        app.world().get::<ChildOf>(debuffs).unwrap().parent(),
        parent
    );
    let children = app.world().get::<Children>(parent).unwrap();
    assert_eq!(children[0], buffs);
    assert_eq!(children[1], debuffs);
    assert_eq!(
        app.world().get::<Node>(parent).unwrap().flex_direction,
        FlexDirection::Column
    );
    assert_eq!(
        app.world().get::<Node>(buffs).unwrap().flex_wrap,
        FlexWrap::Wrap
    );
    assert_eq!(app.world().get::<Children>(debuffs).unwrap().len(), 1);
    app.world_mut().get_mut::<Status>(target).unwrap().health = 90.0;
    app.update();
    assert!(
        icons
            .iter()
            .all(|(entity, _)| app.world().get::<ModIcon>(*entity).is_some()),
        "health updates must not rebuild buff icons"
    );
    let brutal = icons.iter().find(|(_, m)| *m == Rank::Brutal).unwrap().0;
    *app.world_mut().get_mut::<Interaction>(brutal).unwrap() = Interaction::Hovered;
    app.update();
    let expected = super::super::super::monster_specials::player::mod_aura(Rank::Brutal, None);
    let texts: Vec<_> = app
        .world_mut()
        .query::<&Text>()
        .iter(app.world())
        .map(|t| t.0.clone())
        .collect();
    assert!(texts.contains(&expected.name));
    assert!(texts.contains(&expected.description));
    assert!(texts.iter().any(|t| t == "Permanent monster modifier"));
    app.world_mut().resource_mut::<Target>().entity = None;
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&ModIcon>()
            .iter(app.world())
            .count(),
        0
    );
    assert_eq!(
        app.world_mut()
            .query::<&DebuffIcon>()
            .iter(app.world())
            .count(),
        0
    );
}
