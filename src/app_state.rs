use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::Mutex;

use crate::cluster::cache::ClusterCache;
use crate::models::community::CommunityRuleRepository;
use crate::models::flap_event::FlapEventRepository;
use crate::models::node::NodeRepository;
use crate::models::peer::PeerRepository;
use crate::models::probe::ProbeResultRepository;
use crate::models::settings::SettingsRepository;

/// Shared state tracking the last WG+BIRD config apply result.
#[derive(Clone)]
pub struct ApplyStatus {
    pub last_apply_at: Arc<Mutex<Option<String>>>,
    pub last_apply_error: Arc<Mutex<Option<String>>>,
    pub pending: Arc<AtomicBool>,
    pub managed_interfaces: Arc<Mutex<Vec<String>>>,
}

impl ApplyStatus {
    pub fn new() -> Self {
        Self {
            last_apply_at: Arc::new(Mutex::new(None)),
            last_apply_error: Arc::new(Mutex::new(None)),
            pending: Arc::new(AtomicBool::new(false)),
            managed_interfaces: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

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
    pub apply_status: ApplyStatus,
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
            apply_status: ApplyStatus::new(),
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
