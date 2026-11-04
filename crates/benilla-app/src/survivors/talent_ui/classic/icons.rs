//! Use the real specialization's talent art, deduplicated by texture rather than spell rank.
use super::*;
use std::collections::BTreeSet;

fn texture_key(path: &str) -> String {
    let key = path.replace('/', "\\").to_ascii_lowercase();
    key.strip_suffix(".blp").unwrap_or(&key).to_owned()
}

fn assign(
    preferred: [u32; NODES],
    candidates: &[u32],
    mut lookup: impl FnMut(u32) -> Option<String>,
) -> [u32; NODES] {
    let mut result = [0; NODES];
    let mut used = BTreeSet::new();
    // Reserve the meaningful milestone art before filling the passive nodes.
    for node in [8, 12, 18]
        .into_iter()
        .chain((0..NODES).filter(|n| !matches!(n, 8 | 12 | 18)))
    {
        for spell in std::iter::once(preferred[node]).chain(candidates.iter().copied()) {
            let Some(path) = lookup(spell) else {
                continue;
            };
            let key = texture_key(&path);
            if key.is_empty() || key.ends_with("inv_misc_questionmark") || !used.insert(key) {
                continue;
            }
            result[node] = spell;
            break;
        }
    }
    result
}

pub(super) fn for_tree(
    spec: &talents::Spec,
    original: &[u32],
    class_spells: &[u32],
    catalog: &benilla_formats::SpellCatalog,
) -> [u32; NODES] {
    let preferred = std::array::from_fn(|node| {
        if super::super::super::talent_design::milestone(node).is_some() {
            super::super::super::talent_design::icon(spec, node)
        } else {
            original.get(node).copied().unwrap_or(0)
        }
    });
    let mut candidates: Vec<_> = original
        .iter()
        .copied()
        .chain(spec.spells.iter().map(|key| data::ability(key).spell))
        .chain(class_spells.iter().copied())
        .collect();
    let lookup = |id| catalog.get(id).and_then(|s| s.icon.clone());
    let selected = assign(preferred, &candidates, lookup);
    if !selected.contains(&0) {
        return selected;
    }
    // Sparse or modified client talent tables still get distinct, stable icons.
    let mut fallback: Vec<_> = catalog.iter().map(|(id, _)| id).collect();
    fallback.sort_unstable();
    candidates.extend(fallback);
    assign(preferred, &candidates, |id| {
        catalog.get(id).and_then(|s| s.icon.clone())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_spell_ranks_cannot_reuse_a_texture_and_milestones_keep_priority() {
        let mut preferred = [1; NODES];
        preferred[8] = 2;
        let candidates: Vec<_> = (1..=30).collect();
        let lookup = |id| {
            Some(match id {
                1 => "Interface/Icons/Same.BLP".into(),
                2 => "interface\\icons\\same".into(),
                _ => format!("Interface\\Icons\\Talent{id}"),
            })
        };
        let selected = assign(preferred, &candidates, lookup);
        assert_eq!(selected[8], 2);
        let paths: BTreeSet<_> = selected
            .iter()
            .map(|id| texture_key(&lookup(*id).unwrap()))
            .collect();
        assert_eq!(paths.len(), NODES);
        assert!(!selected.contains(&0));
    }

    #[test]
    fn every_loaded_tree_has_nineteen_distinct_readable_icons() {
        let path = benilla_formats::wow_data_or_skip!();
        let mut chain = benilla_formats::open_chain(&path).unwrap();
        let talents = benilla_formats::load_talent_catalog(&mut chain).unwrap();
        let spells = benilla_formats::load_spell_catalog(&mut chain).unwrap();
        for (class, trees) in data::CLASSES.iter().zip(TREES.iter()) {
            let tabs = talents.tabs_for_class(1, class.id);
            let class_spells: Vec<_> = tabs
                .iter()
                .flat_map(|tab| talents.talents_in_tab(tab.id).iter().map(|t| t.ranks[0]))
                .collect();
            for (index, spec) in trees.iter().enumerate() {
                let tab = tabs
                    .iter()
                    .find(|tab| tab.name == spec.name)
                    .copied()
                    .unwrap_or(tabs[index]);
                let original: Vec<_> = talents
                    .talents_in_tab(tab.id)
                    .iter()
                    .map(|t| t.ranks[0])
                    .collect();
                let icons = for_tree(spec, &original, &class_spells, &spells);
                let mut paths = BTreeSet::new();
                for id in icons {
                    let icon = spells.get(id).unwrap().icon.as_ref().unwrap();
                    assert!(
                        paths.insert(texture_key(icon)),
                        "{} / {}",
                        class.name,
                        spec.name
                    );
                    let path = if icon.to_ascii_lowercase().ends_with(".blp") {
                        icon.clone()
                    } else {
                        format!("{icon}.blp")
                    };
                    assert!(!chain.read_file(&path).unwrap().is_empty(), "{path}");
                }
            }
        }
    }
}
