//! Fork policy for helmets: keep the scalp, hide styled hair regardless of the DBC mask.
use super::{EquipGeosets, REGION_BASES};

pub(super) fn hide_hair(selected: &mut [u16], equip: &EquipGeosets) {
    // The app sets helm_vis only for an equipped display that names a head model.
    // This also covers displays whose hide-mask row is zero or absent from the DBC.
    if equip.helm_vis.is_some() {
        selected[0] = REGION_BASES[0];
    }
}

#[cfg(test)]
mod tests {
    use super::super::CharacterGeosets;
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn helmets_hide_hair_for_every_race_and_sex_and_removal_restores_it() {
        let hair = [1, 2, 3, 4, 5, 6, 7, 8]
            .into_iter()
            .flat_map(|race| [0, 1].into_iter().map(move |sex| ((race, sex, 3), 5)))
            .collect();
        let geosets = CharacterGeosets {
            hair,
            facial: HashMap::new(),
            helmet_vis: HashMap::new(),
        };
        for race in 1..=8 {
            for sex in 0..=1 {
                for mask in [[0, 0], [99999, 99999]] {
                    let helmet = EquipGeosets {
                        helm_vis: Some(mask),
                        ..Default::default()
                    };
                    let covered = geosets.visible_geosets(race, sex, 3, 0, &helmet);
                    assert!(covered.contains(&1));
                    assert!((2..100).all(|id| !covered.contains(&id)));
                    for id in [101, 201, 301, 702] {
                        assert!(
                            covered.contains(&id),
                            "helmet policy must preserve facial hair/ears"
                        );
                    }
                }
                let uncovered = geosets.visible_geosets(race, sex, 3, 0, &EquipGeosets::default());
                assert!(uncovered.contains(&5));
                assert!(!uncovered.contains(&1));
            }
        }
    }
}
