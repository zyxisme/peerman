use sqlx::SqlitePool;

use crate::models::community::CommunityRuleRepository;
use crate::models::node::NodeRepository;
use crate::models::peer::PeerRepository;
use crate::models::probe::ProbeResultRepository;
use crate::models::settings::SettingsRepository;

#[derive(Clone)]
pub struct AppState {
    pub peer_repo: PeerRepository,
    pub settings_repo: SettingsRepository,
    pub node_repo: NodeRepository,
    pub probe_repo: ProbeResultRepository,
    pub community_repo: CommunityRuleRepository,
}

impl AppState {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            peer_repo: PeerRepository::new(pool.clone()),
            settings_repo: SettingsRepository::new(pool.clone()),
            node_repo: NodeRepository::new(pool.clone()),
            probe_repo: ProbeResultRepository::new(pool.clone()),
            community_repo: CommunityRuleRepository::new(pool),
        }
    }
}
