//! Runtime-only WoW art; frame geometry follows the player's Blizzard_TalentUI.xml.
use super::super::hud::SpellIcon;
use super::*;
use crate::glue::backdrop::{backdrop_edges, BackdropEdges};

const GREEN: Color = Color::srgb(0.1, 1.0, 0.1);
const GREY: Color = Color::srgb(0.45, 0.45, 0.45);
mod icons;
#[derive(Resource, Default)]
pub(super) struct Art {
    ready: bool,
    backgrounds: Vec<[Option<Handle<Image>>; 4]>,
    icons: Vec<[u32; NODES]>,
    border: Option<BackdropEdges>,
}

pub(super) fn load_art(
    mut art: ResMut<Art>,
    assets: Option<ResMut<benilla_assets::WorldAssets>>,
    images: Option<ResMut<Assets<Image>>>,
    spells: Option<Res<crate::ui_action::Spells>>,
) {
    if art.ready {
        return;
    }
    let (Some(mut assets), Some(mut images), Some(spells)) = (assets, images, spells) else {
        return;
    };
    let catalog = {
        let Ok(mut chain) = assets.chain.lock() else {
            return;
        };
        benilla_formats::load_talent_catalog(&mut chain).ok()
    };
    for (class, specs) in data::CLASSES.iter().zip(TREES.iter()) {
        let tabs = catalog
            .as_ref()
            .map(|c| c.tabs_for_class(1, class.id))
            .unwrap_or_default();
        let class_spells: Vec<_> = tabs
            .iter()
            .flat_map(|tab| {
                catalog
                    .as_ref()
                    .unwrap()
                    .talents_in_tab(tab.id)
                    .iter()
                    .map(|t| t.ranks[0])
            })
            .chain(class.pool.iter().map(|key| data::ability(key).spell))
            .collect();
        for (index, spec) in specs.iter().enumerate() {
            let tab = tabs
                .iter()
                .find(|t| t.name == spec.name)
                .copied()
                .or_else(|| tabs.get(index).copied());
            let original: Vec<_> = tab
                .map(|tab| {
                    catalog
                        .as_ref()
                        .unwrap()
                        .talents_in_tab(tab.id)
                        .iter()
                        .map(|t| t.ranks[0])
                        .collect()
                })
                .unwrap_or_default();
            art.icons.push(icons::for_tree(
                spec,
                &original,
                &class_spells,
                &spells.catalog,
            ));
            let pieces = std::array::from_fn(|piece| {
                let suffix = ["TopLeft", "TopRight", "BottomLeft", "BottomRight"][piece];
                tab.and_then(|tab| {
                    assets.sprite_texture(
                        &format!("Interface\\TalentFrame\\{}-{suffix}.blp", tab.background),
                        &mut images,
                    )
                })
            });
            art.backgrounds.push(pieces);
        }
    }
    art.border = backdrop_edges(
        &mut assets,
        "Interface\\Tooltips\\UI-Tooltip-Border.blp",
        &mut images,
    );
    art.ready = true;
}

#[derive(Component)]
pub(super) struct TipTitle;
#[derive(Component)]
pub(super) struct TipBody;
#[derive(Component)]
pub(super) struct TipFrame;

type Stamp = (bool, bool, usize, usize, u64, Vec<u8>, bool, bool);
mod forest;
pub(super) use forest::render;
#[allow(clippy::type_complexity)]
pub(super) fn tooltip(
    selection: Res<Selection>,
    view: Res<View>,
    account: Res<Account>,
    buttons: Query<(&Interaction, &Action)>,
    mut frames: Query<&mut Visibility, With<TipFrame>>,
    mut titles: Query<&mut Text, (With<TipTitle>, Without<TipBody>)>,
    mut bodies: Query<&mut Text, (With<TipBody>, Without<TipTitle>)>,
) {
    let hovered = buttons.iter().find_map(|(interaction, action)| {
        if *interaction == Interaction::None {
            return None;
        }
        if let Action::Buy(tree, node) = *action {
            Some((tree, node))
        } else {
            None
        }
    });
    for mut frame in &mut frames {
        let visibility = if hovered.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *frame != visibility {
            *frame = visibility;
        }
    }
    let Some((tree, node)) = hovered else {
        return;
    };
    let class = view.class.or(selection.class).unwrap_or(0);
    let spec = &TREES[class][tree];
    let rank = account.rank(class, tree, node);
    let title = super::super::talent_effects::name(spec, node);
    let mut body = super::super::talent_effects::description(spec, node, rank);
    if selection.in_hub && rank > 0 {
        body.push_str(if account.can_refund(class, tree, node) {
            "\nRight-click to refund one point."
        } else {
            "\nCannot refund: other learned talents require this point."
        });
    }
    for mut t in &mut titles {
        if t.0 != title {
            t.0 = title.clone();
        }
    }
    for mut t in &mut bodies {
        if t.0 != body {
            t.0 = body.clone();
        }
    }
}
