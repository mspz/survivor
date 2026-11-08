//! Three simultaneous specialization cards, with a central milestone spine.
use super::super::super::talent_design;
use super::*;

fn bounds(x: f32, y: f32, w: f32, h: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(x),
        top: Val::Percent(y),
        width: Val::Percent(w),
        height: Val::Percent(h),
        ..default()
    }
}
fn position(node: usize) -> (f32, f32) {
    let x = if talent_design::milestone(node).is_some() {
        37.0
    } else {
        [8.0, 37.0, 60.0, 80.0][talents::column(node)]
    };
    (x, 15.0 + talents::tier(node) as f32 * 11.5)
}

#[allow(clippy::too_many_arguments)]
pub(in crate::survivors::talent_ui) fn render(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    selection: Res<Selection>,
    view: Res<View>,
    account: Res<Account>,
    art: Res<Art>,
    fonts: Option<Res<super::super::super::hud::HudFont>>,
    roots: Query<Entity, With<Root>>,
    mut cache: Local<Option<Stamp>>,
) {
    let Some(fonts) = fonts else {
        return;
    };
    let f = &fonts.0;
    let class = view.class.or(selection.class).unwrap_or(0);
    let menu = matches!(
        *phase.get(),
        Phase::Hub | Phase::Playing | Phase::CharacterSheet
    );
    let ranks = (0..3)
        .flat_map(|t| (0..NODES).map(move |n| (t, n)))
        .map(|(t, n)| account.rank(class, t, n))
        .collect();
    let key = (
        menu,
        selection.talents_open,
        class,
        0,
        account.xp,
        ranks,
        art.ready,
        editable(*phase.get(), &selection),
    );
    if cache.as_ref() == Some(&key) {
        return;
    }
    *cache = Some(key);
    for root in &roots {
        commands.entity(root).despawn();
    }
    if !menu {
        return;
    }
    if !selection.talents_open {
        return;
    }
    let level = talents::level(account.xp);
    let (progress, experience) = if level >= 60 {
        (100.0, "Character level 60 · Maximum level".to_owned())
    } else {
        let earned = account.xp.saturating_sub(talents::threshold(level));
        let required = talents::threshold(level + 1) - talents::threshold(level);
        let progress = earned as f32 / required as f32 * 100.0;
        (
            progress,
            format!(
                "Character XP · Level {level} → {}: {earned} / {required} ({progress:.0}%)",
                level + 1
            ),
        )
    };
    commands.spawn((Root,Button,GlobalZIndex(600),BackgroundColor(Color::srgba(0.0,0.0,0.0,0.94)),Node{position_type:PositionType::Absolute,width:Val::Percent(100.0),height:Val::Percent(100.0),padding:UiRect::all(Val::Px(12.0)),flex_direction:FlexDirection::Column,row_gap:Val::Px(8.0),..default()})).with_children(|modal| {
        modal.spawn(Node{width:Val::Percent(100.0),justify_content:JustifyContent::SpaceBetween,align_items:AlignItems::Center,..default()}).with_children(|header| {
            header.spawn(text(f,format!("{} · Level {} · {} points available",data::CLASSES[class].name,talents::level(account.xp),account.available(class)),21.0,GOLD));
            header.spawn((button(Action::Toggle,Color::srgb(0.2,0.04,0.03)),Node{padding:UiRect::axes(Val::Px(14.0),Val::Px(6.0)),..default()})).with_children(|b|{b.spawn(text(f,"Close [T / Esc]",15.0,GOLD));});
        });
        modal.spawn((Node{width:Val::Percent(100.0),height:Val::Px(25.0),flex_shrink:0.0,border:UiRect::all(Val::Px(1.0)),overflow:Overflow::clip(),..default()},BackgroundColor(Color::srgb(0.08,0.04,0.12)),BorderColor::all(GOLD))).with_children(|bar| {
            bar.spawn((Node{width:Val::Percent(progress),height:Val::Percent(100.0),..default()},BackgroundColor(Color::srgb(0.48,0.22,0.75))));
            bar.spawn(Node{position_type:PositionType::Absolute,left:Val::Px(0.0),top:Val::Px(0.0),width:Val::Percent(100.0),height:Val::Percent(100.0),justify_content:JustifyContent::Center,align_items:AlignItems::Center,..default()}).with_children(|label| {label.spawn(text(f,experience,14.0,Color::WHITE));});
        });
        modal.spawn(Node{width:Val::Percent(100.0),flex_grow:1.0,min_height:Val::Px(0.0),column_gap:Val::Px(12.0),..default()}).with_children(|row| {
            for tree in 0..3 {
                let design=&talent_design::DESIGNS[class*3+tree];let spec=&TREES[class][tree];
                row.spawn((BackgroundColor(Color::srgb(0.055,0.045,0.025)),BorderColor::all(GOLD),Node{flex_basis:Val::Px(0.0),flex_grow:1.0,min_width:Val::Px(0.0),height:Val::Percent(100.0),border:UiRect::all(Val::Px(1.0)),border_radius:BorderRadius::all(Val::Px(12.0)),overflow:Overflow::clip(),..default()})).with_children(|card| {
                    for (piece,(x,y,w,h)) in [(0.0,14.0,80.0,56.0),(80.0,14.0,20.0,56.0),(0.0,70.0,80.0,28.0),(80.0,70.0,20.0,28.0)].into_iter().enumerate() {
                        if let Some(image)=art.backgrounds.get(class*3+tree).and_then(|p|p[piece].clone()) {card.spawn((ImageNode::new(image),bounds(x,y,w,h)));}
                    }
                    card.spawn((BackgroundColor(Color::srgba(0.0,0.0,0.0,0.22)),bounds(0.0,14.0,100.0,86.0)));
                    card.spawn((ImageNode::default(),SpellIcon(data::ability(spec.cap).spell),Node{position_type:PositionType::Absolute,left:Val::Px(12.0),top:Val::Px(12.0),width:Val::Px(34.0),height:Val::Px(34.0),..default()}));
                    card.spawn((text(f,design.title,23.0,GOLD),Node{position_type:PositionType::Absolute,left:Val::Px(54.0),top:Val::Px(12.0),..default()}));
                    card.spawn((text(f,format!("{} points",account.tree_points(class,tree)),14.0,GOLD),Node{position_type:PositionType::Absolute,right:Val::Px(12.0),top:Val::Px(43.0),..default()}));
                    card.spawn((text(f,design.focus,12.0,Color::srgb(0.8,0.76,0.6)),bounds(3.0,8.0,92.0,6.0)));
                    for node in [12,18] {
                        let parent=talents::prerequisite(node).unwrap();let(x,y)=position(node);let(_,py)=position(parent);
                        let learned=account.rank(class,tree,parent)>0;
                        card.spawn((BackgroundColor(if learned{GOLD}else{GREY}),Node{position_type:PositionType::Absolute,left:Val::Percent(x),top:Val::Percent(py),width:Val::Px(3.0),height:Val::Percent(y-py),margin:UiRect::left(Val::Px(24.0)),..default()}));
                    }
                    for node in 0..NODES {
                        let rank=account.rank(class,tree,node);let milestone=talent_design::milestone(node).is_some();
                        let enabled=editable(*phase.get(),&selection)&&account.can_buy(class,tree,node);
                        let color=if rank==talents::max_rank(node){GOLD}else if enabled{GREEN}else{GREY};
                        let(x,y)=position(node);
                        card.spawn((Button,Action::Buy(tree,node),BackgroundColor(Color::BLACK),Node{position_type:PositionType::Absolute,left:Val::Percent(x),top:Val::Percent(y),width:Val::Px(48.0),height:Val::Px(48.0),border:UiRect::all(Val::Px(if milestone{3.0}else{1.5})),border_radius:BorderRadius::all(Val::Px(6.0)),..default()},BorderColor::all(if milestone&&rank>0{Color::srgb(0.8,0.4,1.0)}else{color}))).with_children(|b| {
                            b.spawn((ImageNode::default(),SpellIcon(art.icons.get(class*3+tree).map_or_else(||talent_design::icon(spec,node),|icons|icons[node])),Node{width:Val::Percent(100.0),height:Val::Percent(100.0),..default()}));
                            if !enabled&&rank==0 {b.spawn((BackgroundColor(Color::srgba(0.0,0.0,0.0,0.68)),bounds(0.0,0.0,100.0,100.0)));}
                            b.spawn((text(f,format!("{rank}/{}",talents::max_rank(node)),12.0,color),BackgroundColor(Color::BLACK),Node{position_type:PositionType::Absolute,right:Val::Px(-5.0),bottom:Val::Px(-9.0),padding:UiRect::horizontal(Val::Px(3.0)),..default()}));
                        });
                    }
                });
            }
        });
        modal.spawn(Node{width:Val::Percent(100.0),justify_content:JustifyContent::SpaceBetween,align_items:AlignItems::Center,..default()}).with_children(|footer| {
            footer.spawn(text(f,"Milestones: 10 / 20 / 30 points in the tree · Gold: learned · Green: available · Gray: locked",13.0,GOLD));
            if editable(*phase.get(),&selection){footer.spawn((button(Action::Reset,Color::srgb(0.25,0.035,0.025)),Node{padding:UiRect::all(Val::Px(8.0)),..default()})).with_children(|b|{b.spawn(text(f,"Reset for free",14.0,GOLD));});}
            else {footer.spawn(text(f,"Changes are only available in the hub",13.0,GREY));}
        });
        modal.spawn((TipFrame,Visibility::Hidden,GlobalZIndex(700),BackgroundColor(Color::srgba(0.015,0.015,0.025,0.98)),BorderColor::all(GOLD),Node{position_type:PositionType::Absolute,left:Val::Percent(28.0),bottom:Val::Px(50.0),width:Val::Percent(44.0),padding:UiRect::all(Val::Px(14.0)),border:UiRect::all(Val::Px(1.0)),row_gap:Val::Px(6.0),flex_direction:FlexDirection::Column,..default()})).with_children(|tip| {
            tip.spawn((TipTitle,text(f,"",20.0,GOLD)));tip.spawn((TipBody,text(f,"",14.0,Color::WHITE)));
        });
    });
}
