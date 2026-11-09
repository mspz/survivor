//! The coin sound: the 1.12 client has no money kit; a `CMirrorHandler` callback on
//! `PLAYER_FIELD_COINAGE` (`0x5ddf30`), the same watcher that fires `PLAYER_MONEY`, plays
//! `LOOTWINDOWCOINSOUND` (kit 895) on every change, so buying, selling, looting money and any
//! other purse change all sound. Played 2D on the SFX bucket.
//!
//! The first value after login or reconnect seeds without playing; whether the reference's watcher
//! fires on the login set is untraced. Deviation: the reference also plays an optimistic coin at
//! the loot-money click, so looted money sounds twice; benilla plays the watcher's coin alone,
//! because one coin on the confirmed change reads cleanly and the round trip the early coin hides
//! is imperceptible on a local server.

use bevy::prelude::*;

use crate::net::{ObjectStore, SelfPlayer};
use benilla_assets::WorldAssets;

use super::kit::{self, KitRef, SoundKits};
use super::{SoundConfig, SoundOutput};

/// The SoundEntries name the client plays on any coinage change (kit 895, `0x5ddf30`).
const COIN_SOUND: &str = "LOOTWINDOWCOINSOUND";

/// Play the coin whenever the self-player's coinage changes, in either direction. The previous
/// value resets with no self-player, so a reconnect re-seeds instead of replaying a stale delta.
fn play_coin_on_coinage_change(
    self_q: Query<&ObjectStore, With<SelfPlayer>>,
    mut prev: Local<Option<u32>>,
    kits: Option<ResMut<SoundKits>>,
    assets: Option<Res<WorldAssets>>,
    mut out: NonSendMut<SoundOutput>,
    config: Res<SoundConfig>,
) {
    let Some(store) = self_q.iter().next() else {
        *prev = None;
        return;
    };
    let Some(money) = store.0.player_money() else {
        return;
    };
    // Advanced even without a catalog, so a late-loading catalog never replays this delta.
    let old = prev.replace(money);
    if !matches!(old, Some(p) if p != money) {
        return; // first sight or unchanged
    }
    let (Some(mut kits), Some(assets)) = (kits, assets) else {
        return;
    };
    if let Err(e) = kit::play_kit(
        &mut kits,
        &assets,
        &mut out,
        &config,
        Vec3::ZERO,
        KitRef::Name(COIN_SOUND),
        None,
        kit::SoundCategory::Sfx,
    ) {
        debug!("sound(money): coin — {e:#}");
    }
}

/// A one-shot coin, written by game code that wants the gold sound without a coinage change
/// (e.g. collecting a survivors boss item).
#[derive(Message, Clone, Copy, Default)]
pub(crate) struct CoinSound;

/// Play the coin once per [`CoinSound`]; with a catalog missing the queue is dropped.
fn play_coin_requests(
    mut reqs: MessageReader<CoinSound>,
    kits: Option<ResMut<SoundKits>>,
    assets: Option<Res<WorldAssets>>,
    mut out: NonSendMut<SoundOutput>,
    config: Res<SoundConfig>,
) {
    if reqs.is_empty() {
        return;
    }
    let (Some(mut kits), Some(assets)) = (kits, assets) else {
        reqs.clear();
        return;
    };
    for _ in reqs.read() {
        if let Err(e) = kit::play_kit(
            &mut kits,
            &assets,
            &mut out,
            &config,
            Vec3::ZERO,
            KitRef::Name(COIN_SOUND),
            None,
            kit::SoundCategory::Sfx,
        ) {
            debug!("sound(money): coin request — {e:#}");
        }
    }
}

/// A one-shot interface file (`Sound\Interface\...`), 2D on the SFX slider: game code's
/// `PlaySoundFile`, e.g. the survivors achievement fanfare.
#[derive(Message, Clone, Copy)]
pub(crate) struct FileSound(pub &'static str);

fn play_file_requests(
    mut reqs: MessageReader<FileSound>,
    kits: Option<ResMut<SoundKits>>,
    assets: Option<Res<WorldAssets>>,
    mut out: NonSendMut<SoundOutput>,
    config: Res<SoundConfig>,
) {
    if reqs.is_empty() {
        return;
    }
    let (Some(mut kits), Some(assets)) = (kits, assets) else {
        reqs.clear();
        return;
    };
    for FileSound(path) in reqs.read() {
        if let Err(e) = kit::play_file(&mut kits, &assets, &mut out, &config, path, kit::SoundCategory::Sfx) {
            debug!("sound(file): {path} — {e:#}");
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_message::<CoinSound>()
        .add_message::<FileSound>()
        .add_systems(
            Update,
            (play_coin_on_coinage_change, play_coin_requests, play_file_requests),
        );
}
