use sqlx::SqlitePool;

use crate::models::peer::PeerRepository;
use crate::models::settings::SettingsRepository;

#[derive(Clone)]
pub struct AppState {
    pub peer_repo: PeerRepository,
    pub settings_repo: SettingsRepository,
}

impl AppState {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            peer_repo: PeerRepository::new(pool.clone()),
            settings_repo: SettingsRepository::new(pool),
        }
    }
}
