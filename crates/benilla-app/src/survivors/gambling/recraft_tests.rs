use super::*;

fn press(app: &mut App, predicate: impl Fn(&Click) -> bool) {
    let world = app.world_mut();
    let buttons: Vec<_> = world
        .query::<(Entity, &Click)>()
        .iter(world)
        .map(|(entity, click)| (entity, predicate(click)))
        .collect();
    let pressed = buttons.iter().find(|(_, matched)| *matched).unwrap().0;
    for (entity, _) in buttons {
        world.entity_mut(entity).insert(Interaction::None);
    }
    world.entity_mut(pressed).insert(Interaction::Pressed);
    app.update();
}

#[test]
fn recraft_tab_edits_owned_legendaries_at_actual_ilvl_without_changing_other_rolls() {
    let request = CraftRequest {
        slot: SlotKind::Ring,
        equipment: EquipmentType::Universal,
        profile: CraftProfile::Strength,
        rarity: Rarity::Legendary,
        affixes: Some(vec!["strength".into()]),
    };
    let mut item = crate::survivors::items::Config::builtin()
        .generate_training_craft(
            &request,
            60,
            1,
            Some(&crate::survivors::items::legendary::Roll {
                unique: 403,
                milestone: None,
            }),
            &mut Rng(4),
        )
        .unwrap();
    item.raise_item_level(110).unwrap();
    let original = serde_json::to_value(&item.affixes[0]).unwrap();
    let mut inventory = Inventory::fixture(vec![item]);
    inventory
        .add_money(100_000 * crate::survivors::currency::GOLD)
        .unwrap();
    let mut account = talents::Account::fresh();
    account.character = Some(1);
    account.xp = talents::threshold(60);
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(State::new(Phase::Hub))
        .insert_resource(Selection {
            in_hub: true,
            ..default()
        })
        .insert_resource(inventory)
        .insert_resource(account)
        .init_resource::<View>()
        .init_resource::<Rng>()
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(hud::HudFont(Handle::default()))
        .add_systems(Update, (input, super::super::render, tooltip).chain());
    app.world_mut().spawn((
        hero::Hero { vy: 0.0, air: 0.0 },
        Transform::from_translation(position()),
    ));
    app.update();
    press(&mut app, |c| matches!(c, Click::Open));
    press(&mut app, |c| matches!(c, Click::Tab(true)));
    app.world_mut().resource_scope(|world, mut view: Mut<View>| {
        view.place_recraft_item(1, world.resource::<Inventory>());
    });
    app.update();
    assert_eq!(app.world().resource::<View>().ilvl, Some(110));
    assert!(
        app.world_mut()
            .query::<&Click>()
            .iter(app.world())
            .all(|c| !matches!(
                c,
                Click::Level(_) | Click::Rarity(_) | Click::SelectLegendary(_) | Click::Dropdown(6)
            ))
    );
    let choices = app.world().resource::<Inventory>().recraft_affixes(1);
    let index = choices
        .iter()
        .position(|a| a.kind == crate::survivors::items::AffixKind::Prefix)
        .unwrap();
    press(&mut app, |c| matches!(c, Click::Dropdown(0)));
    press(
        &mut app,
        |c| matches!(c, Click::SelectAffix(0, Some(i)) if *i == index),
    );
    assert_eq!(edits(app.world().resource::<View>()).len(), 1);
    press(&mut app, |c| matches!(c, Click::Dropdown(0)));
    press(&mut app, |c| matches!(c, Click::RestoreAffix(0)));
    assert!(edits(app.world().resource::<View>()).is_empty());
    press(&mut app, |c| matches!(c, Click::Dropdown(0)));
    press(
        &mut app,
        |c| matches!(c, Click::SelectAffix(0, Some(i)) if *i == index),
    );
    let before = app.world().resource::<Inventory>().money();
    press(&mut app, |c| matches!(c, Click::Buy));
    let inventory = app.world().resource::<Inventory>();
    assert_eq!(inventory.money(), before - service::price(110).unwrap());
    let item = inventory.item(1).unwrap();
    assert_eq!(item.level, 110);
    assert_eq!(item.legendary.as_ref().unwrap().unique, 403);
    assert_eq!(
        serde_json::to_value(item.affixes.iter().find(|a| a.id == "strength").unwrap()).unwrap(),
        original
    );
    assert!(edits(app.world().resource::<View>()).is_empty());
    let world = app.world_mut();
    assert!(
        world
            .query::<&crate::survivors::combat_ui::bags::DropTarget>()
            .iter(world)
            .any(|target| matches!(
                target,
                crate::survivors::combat_ui::bags::DropTarget::Recraft
            ))
    );
    press(&mut app, |c| matches!(c, Click::Tab(false)));
    assert!(!app.world().resource::<View>().recraft);
}
