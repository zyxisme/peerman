import { createClient } from '@connectrpc/connect';
import { createConnectTransport } from '@connectrpc/connect-web';
import { PeerService, SettingsService, ClusterService } from './peerman_pb';

export const transport = createConnectTransport({
  baseUrl: '/api',
});

export const peerClient = createClient(PeerService, transport);
export const settingsClient = createClient(SettingsService, transport);
export const clusterClient = createClient(ClusterService, transport);
