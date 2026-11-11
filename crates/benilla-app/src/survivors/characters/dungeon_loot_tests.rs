use super::*;

fn in_details(world: &World, mut entity: Entity) -> bool {
    loop {
        if world.get::<DetailsScroll>(entity).is_some() {
            return true;
        }
        let Some(parent) = world.get::<ChildOf>(entity) else {
            return false;
        };
        entity = parent.parent();
    }
}

#[test]
fn selected_dungeon_lists_its_drops_in_the_right_panel_without_hover_popups() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(State::new(Phase::Hub))
        .insert_resource(Roster {
            expeditions: true,
            ..default()
        })
        .insert_resource(Selection {
            map: data::DUNGEONS[0],
            test_character: true,
            ..default()
        })
        .init_resource::<talents::Account>()
        .insert_resource(View {
            open: [false, true, false],
            ..default()
        })
        .insert_resource(hud::HudFont(Handle::default()))
        .add_systems(Update, (super::super::render, hover).chain());
    for &map in data::DUNGEONS {
        {
            let mut selection = app.world_mut().resource_mut::<Selection>();
            selection.map = map;
            selection.version += 1;
        }
        app.update();
        let texts: Vec<_> = app
            .world_mut()
            .query::<(Entity, &Text)>()
            .iter(app.world())
            .map(|(entity, t)| (entity, t.0.clone()))
            .collect();
        assert!(
            texts
                .iter()
                .any(|(e, t)| t == "DROPS" && in_details(app.world(), *e))
        );
        for (name, description) in super::super::super::items::legendary::dungeon_uniques(map) {
            for value in [name, description] {
                assert!(
                    texts
                        .iter()
                        .any(|(entity, t)| t == value && in_details(app.world(), *entity)),
                    "missing {name} in selected dungeon's right panel"
                );
            }
        }
        for &other in data::DUNGEONS.iter().filter(|&&other| other != map) {
            for (name, _) in super::super::super::items::legendary::dungeon_uniques(other) {
                assert!(
                    !texts.iter().any(|(_, t)| t == name),
                    "wrong dungeon's drop: {name}"
                );
            }
        }
        let buttons: Vec<_> = app
            .world_mut()
            .query_filtered::<Entity, With<Button>>()
            .iter(app.world())
            .collect();
        for entity in buttons {
            app.world_mut()
                .entity_mut(entity)
                .insert(Interaction::Hovered);
        }
        app.update();
        assert!(
            app.world_mut()
                .query::<&GlobalZIndex>()
                .iter(app.world())
                .all(|z| z.0 < 800),
            "hover must not create a floating loot tooltip"
        );
    }
    app.world_mut().spawn((
        Window {
            resolution: bevy::window::WindowResolution::new(600, 800),
            ..default()
        },
        bevy::window::PrimaryWindow,
    ));
    app.update();
    let compact_texts: Vec<_> = app
        .world_mut()
        .query::<(Entity, &Text)>()
        .iter(app.world())
        .filter(|(e, _)| in_details(app.world(), *e))
        .map(|(_, t)| t.0.clone())
        .collect();
    assert!(
        compact_texts.iter().any(|t| t == "DROPS"),
        "compact layouts also keep the drop section"
    );
}
