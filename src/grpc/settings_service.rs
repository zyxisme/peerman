use tonic::{Request, Response, Status};

use super::generated::{
    settings_service_server::SettingsService, GetSettingsRequest, SaveSettingsRequest, Settings,
};

use crate::models::settings::SettingsRepository;

pub struct SettingsServiceImpl {
    pub settings_repo: SettingsRepository,
    pub jwt_secret: std::sync::Arc<String>,
}

fn settings_to_proto(s: &crate::models::settings::Settings) -> Settings {
    Settings {
        local_asn: s.local_asn,
        bird_template_name: s.bird_template_name.clone(),
        bird_router_id: s.bird_router_id.clone(),
        wg_default_listen_port: s.wg_default_listen_port as u32,
        dn42_ipv4_prefix: s.dn42_ipv4_prefix.clone(),
        dn42_ipv6_prefix: s.dn42_ipv6_prefix.clone(),
        wg_table: s.wg_table.clone(),
        wg_mtu: s.wg_mtu as u32,
        wg_fwmark: s.wg_fwmark as u32,
        wg_post_up: s.wg_post_up.clone(),
        wg_post_down: s.wg_post_down.clone(),
        roa_mode: s.roa_mode.clone(),
        roa_static_v4_url: s.roa_static_v4_url.clone(),
        roa_static_v6_url: s.roa_static_v6_url.clone(),
        roa_rtr_address: s.roa_rtr_address.clone(),
        roa_rtr_port: s.roa_rtr_port as u32,
        bird_import_limit: s.bird_import_limit as u32,
        bird_export_filter: s.bird_export_filter.clone(),
        bird_import_filter: s.bird_import_filter.clone(),
        enable_community_filters: s.enable_community_filters,
        enable_bfd: s.enable_bfd,
        bfd_interval_ms: s.bfd_interval_ms as u32,
        bfd_multiplier: s.bfd_multiplier as u32,
        cluster_tunnel_ipv6_range: s.cluster_tunnel_ipv6_range.clone(),
        enable_confederation: s.enable_confederation,
        confederation_local_asn: s.confederation_local_asn,
    }
}

fn apply_settings(s: &mut crate::models::settings::Settings, proto: &Settings) {
    if proto.local_asn != 0 {
        s.local_asn = proto.local_asn;
    }
    if !proto.bird_template_name.is_empty() {
        s.bird_template_name = proto.bird_template_name.clone();
    }
    if !proto.bird_router_id.is_empty() {
        s.bird_router_id = proto.bird_router_id.clone();
    }
    if proto.wg_default_listen_port != 0 {
        s.wg_default_listen_port = proto.wg_default_listen_port as i64;
    }
    if !proto.dn42_ipv4_prefix.is_empty() {
        s.dn42_ipv4_prefix = proto.dn42_ipv4_prefix.clone();
    }
    if !proto.dn42_ipv6_prefix.is_empty() {
        s.dn42_ipv6_prefix = proto.dn42_ipv6_prefix.clone();
    }
    if !proto.wg_table.is_empty() {
        s.wg_table = proto.wg_table.clone();
    }
    if proto.wg_mtu != 0 {
        s.wg_mtu = proto.wg_mtu as i64;
    }
    if proto.wg_fwmark != 0 {
        s.wg_fwmark = proto.wg_fwmark as i64;
    }
    if !proto.wg_post_up.is_empty() {
        s.wg_post_up = proto.wg_post_up.clone();
    }
    if !proto.wg_post_down.is_empty() {
        s.wg_post_down = proto.wg_post_down.clone();
    }
    if !proto.roa_mode.is_empty() {
        s.roa_mode = proto.roa_mode.clone();
    }
    if !proto.roa_static_v4_url.is_empty() {
        s.roa_static_v4_url = proto.roa_static_v4_url.clone();
    }
    if !proto.roa_static_v6_url.is_empty() {
        s.roa_static_v6_url = proto.roa_static_v6_url.clone();
    }
    if !proto.roa_rtr_address.is_empty() {
        s.roa_rtr_address = proto.roa_rtr_address.clone();
    }
    if proto.roa_rtr_port != 0 {
        s.roa_rtr_port = proto.roa_rtr_port as i64;
    }
    if proto.bird_import_limit != 0 {
        s.bird_import_limit = proto.bird_import_limit as i64;
    }
    if !proto.bird_export_filter.is_empty() {
        s.bird_export_filter = proto.bird_export_filter.clone();
    }
    if !proto.bird_import_filter.is_empty() {
        s.bird_import_filter = proto.bird_import_filter.clone();
    }
    // Bool field: always apply (false is a valid value)
    s.enable_community_filters = proto.enable_community_filters;
    s.enable_bfd = proto.enable_bfd;
    if proto.bfd_interval_ms > 0 {
        s.bfd_interval_ms = proto.bfd_interval_ms as i64;
    }
    if proto.bfd_multiplier > 0 {
        s.bfd_multiplier = proto.bfd_multiplier as i64;
    }
    // Always apply cluster_tunnel_ipv6_range (empty string is valid — clears the range)
    s.cluster_tunnel_ipv6_range = proto.cluster_tunnel_ipv6_range.clone();
    // Bool field: always apply (false is a valid value)
    s.enable_confederation = proto.enable_confederation;
    if proto.confederation_local_asn != 0 {
        s.confederation_local_asn = proto.confederation_local_asn;
    }
}

#[tonic::async_trait]
impl SettingsService for SettingsServiceImpl {
    async fn get_settings(
        &self,
        _request: Request<GetSettingsRequest>,
    ) -> Result<Response<Settings>, Status> {
        let settings = self
            .settings_repo
            .load()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(settings_to_proto(&settings)))
    }

    async fn save_settings(
        &self,
        request: Request<SaveSettingsRequest>,
    ) -> Result<Response<Settings>, Status> {
        crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
        let req = request.into_inner();
        let proto_settings = req
            .settings
            .ok_or_else(|| Status::invalid_argument("settings is required"))?;

        let mut settings = self
            .settings_repo
            .load()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        apply_settings(&mut settings, &proto_settings);

        let settings = self
            .settings_repo
            .save(&settings)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(settings_to_proto(&settings)))
    }
}
