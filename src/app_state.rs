use sqlx::SqlitePool;

use crate::cluster::cache::ClusterCache;
use crate::models::community::CommunityRuleRepository;
use crate::models::flap_event::FlapEventRepository;
use crate::models::node::NodeRepository;
use crate::models::peer::PeerRepository;
use crate::models::probe::ProbeResultRepository;
use crate::models::settings::SettingsRepository;

/// Focused sub-state for peer-related gRPC services.
/// Groups the three repositories that PeerServiceImpl and apply_wg_bird need.
#[derive(Clone)]
pub struct PeerState {
    pub peer_repo: PeerRepository,
    pub settings_repo: SettingsRepository,
    pub node_repo: NodeRepository,
}

#[derive(Clone)]
pub struct AppState {
    pub peer_repo: PeerRepository,
    pub settings_repo: SettingsRepository,
    pub node_repo: NodeRepository,
    pub probe_repo: ProbeResultRepository,
    pub community_repo: CommunityRuleRepository,
    pub flap_event_repo: FlapEventRepository,
    pub cluster_cache: ClusterCache,
}

impl AppState {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            peer_repo: PeerRepository::new(pool.clone()),
            settings_repo: SettingsRepository::new(pool.clone()),
            node_repo: NodeRepository::new(pool.clone()),
            probe_repo: ProbeResultRepository::new(pool.clone()),
            community_repo: CommunityRuleRepository::new(pool.clone()),
            flap_event_repo: FlapEventRepository::new(pool),
            cluster_cache: ClusterCache::new(),
        }
    }

    /// Extract a PeerState containing the repos needed by PeerServiceImpl.
    pub fn peer_state(&self) -> PeerState {
        PeerState {
            peer_repo: self.peer_repo.clone(),
            settings_repo: self.settings_repo.clone(),
            node_repo: self.node_repo.clone(),
        }
    }
}
