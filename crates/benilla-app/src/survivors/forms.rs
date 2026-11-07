//! Talent forms are derived from each player's build, never saved as their base appearance.
use super::*;
use crate::net::{FieldChanged, Guid, NetEntity, ObjectStore};
use benilla_protocol::messages::{ObjectFields, ObjectType};

pub(super) const CAT_DISPLAY: u32 = 892;

pub(super) fn cat(run: &Run) -> bool {
    run.class.id == 11 && run.talent_build[1][8] > 0 || run.legendary.grants.iter().any(|g|g.option==75)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feral_form_persists_and_respec_restores_body_without_losing_equipment() {
        let mut app = App::new();
        let mut run = Run::default();
        run.class = &data::CLASSES[8];
        run.talent_build[1][8] = 1;
        app.insert_resource(run)
            .add_message::<FieldChanged>()
            .add_systems(Update, sync);
        let entity = app
            .world_mut()
            .spawn((
                hero::Hero { vy: 0.0, air: 0.0 },
                Guid(123),
                NetEntity {
                    kind: benilla_protocol::EntityKind::Player,
                    display_id: Some(55),
                    scale: 1.0,
                },
                ObjectStore(ObjectFields::from_pairs(&[
                    (131, 55),
                    (132, 55),
                    (138, 0x12003456),
                    (440, 987),
                ])),
            ))
            .id();
        for _ in 0..100 {
            app.update();
        }
        assert_eq!(
            app.world().get::<NetEntity>(entity).unwrap().display_id,
            Some(CAT_DISPLAY)
        );
        let fields = &app.world().get::<ObjectStore>(entity).unwrap().0;
        assert_eq!(fields.unit_shapeshift_form(), 1);
        assert_eq!(fields.unit_native_displayid(), Some(55));
        assert!(fields.raw_fields().any(|pair| pair == (440, 987)));
        assert!(fields.raw_fields().any(|pair| pair == (138, 0x12013456)));
        app.world_mut().resource_mut::<Run>().talent_build[1][8] = 0;
        app.update();
        assert_eq!(
            app.world().get::<NetEntity>(entity).unwrap().display_id,
            Some(55)
        );
        assert_eq!(
            app.world()
                .get::<ObjectStore>(entity)
                .unwrap()
                .0
                .unit_shapeshift_form(),
            0
        );
    }

    #[test]
    fn other_bleed_milestones_do_not_grant_cat_form() {
        let mut run = Run::default();
        run.talent_build[1][8] = 1;
        assert!(!cat(&run));
        run.class = &data::CLASSES[8];
        assert!(cat(&run));
        run.talent_build[1][8] = 0;
        assert!(!cat(&run));
    }
}

pub(super) fn sync(
    run: Res<Run>,
    players: Option<Res<multiplayer::Companions>>,
    phase: Option<Res<State<Phase>>>,
    mut heroes: Query<(
        Entity,
        &Guid,
        &mut NetEntity,
        &mut ObjectStore,
        Option<&hero::Hero>,
        Option<&multiplayer::RemoteOwner>,
    )>,
    mut changes: MessageWriter<FieldChanged>,
) {
    if phase.is_some_and(|p| *p.get() == Phase::Menu) {
        return;
    }
    for (entity, guid, mut net, mut store, hero, owner) in &mut heroes {
        let player = if let Some(owner) = owner {
            players
                .as_ref()
                .and_then(|p| p.0.get(&owner.0))
                .map(|p| &p.run)
        } else if hero.is_some() {
            Some(&*run)
        } else {
            None
        };
        let Some(player) = player else { continue };
        let enabled = cat(player);
        if !enabled && store.0.unit_shapeshift_form() != 1 {
            continue;
        }
        let native = store
            .0
            .unit_native_displayid()
            .filter(|v| *v > 0)
            .map(|v| v as u32)
            .or(net.display_id)
            .unwrap_or(0);
        if native == 0 {
            continue;
        }
        let display = if enabled { CAT_DISPLAY } else { native };
        let bytes = store
            .0
            .raw_fields()
            .find(|(i, _)| *i == 138)
            .map_or(0, |(_, v)| v);
        let fields = [
            (131, display),
            (132, native),
            (138, (bytes & !(0xff << 16)) | (u32::from(enabled) << 16)),
        ];
        net.display_id = Some(display);
        store
            .0
            .merge_diff(ObjectFields::from_pairs(&fields), |index, old, new| {
                changes.write(FieldChanged {
                    entity,
                    guid: guid.0,
                    kind: if net.kind == benilla_protocol::EntityKind::Player {
                        ObjectType::Player
                    } else {
                        ObjectType::Unit
                    },
                    index,
                    old,
                    new,
                });
            });
    }
}
