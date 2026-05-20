use tonic::{Request, Response, Status};

use super::generated::{
    settings_service_server::SettingsService, GetSettingsRequest, SaveSettingsRequest, Settings,
};

use crate::models::settings::SettingsRepository;

pub struct SettingsServiceImpl {
    pub settings_repo: SettingsRepository,
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
