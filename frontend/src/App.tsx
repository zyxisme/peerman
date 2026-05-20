import { Routes, Route } from 'react-router-dom';
import Layout from './components/layout/Layout';
import ErrorBoundary from './components/ErrorBoundary';
import PeerTable from './components/peers/PeerTable';
import PeerForm from './components/peers/PeerForm';
import PeerDetail from './components/peers/PeerDetail';
import SettingsPage from './components/settings/SettingsForm';
import WireGuardAllConfig from './components/config/WireGuardConfig';
import BirdAllConfig from './components/config/BirdConfig';
import NodesTable from './components/nodes/NodesTable';
import NodeForm from './components/nodes/NodeForm';
import NodeDetail from './components/nodes/NodeDetail';
import ProbeDashboard from './components/probes/ProbeDashboard';
import CommunityRules from './components/communities/CommunityRules';
import LookingGlass from './components/bird/LookingGlass';
import FlapDashboard from './components/flaps/FlapDashboard';

function ExportPage() {
  return (
    <div className="space-y-lg animate-fade-in">
      <h1 className="text-display-md text-ink">Export Configurations</h1>
      <div className="card">
        <WireGuardAllConfig />
      </div>
      <div className="card">
        <BirdAllConfig />
      </div>
    </div>
  );
}

function HomePage() {
  return (
    <div className="space-y-lg animate-fade-in">
      <PeerTable />
    </div>
  );
}

export default function App() {
  return (
    <Layout>
      <div className="px-lg py-xl">
        <ErrorBoundary>
        <Routes>
          <Route path="/" element={<HomePage />} />
          <Route path="/peers/new" element={<PeerForm />} />
          <Route path="/peers/:id" element={<PeerDetail />} />
          <Route path="/peers/:id/edit" element={<PeerForm />} />
          <Route path="/export" element={<ExportPage />} />
          <Route path="/settings" element={<SettingsPage />} />
          <Route path="/nodes" element={<NodesTable />} />
          <Route path="/nodes/new" element={<NodeForm />} />
          <Route path="/nodes/:id" element={<NodeDetail />} />
          <Route path="/nodes/:id/edit" element={<NodeForm />} />
          <Route path="/probes" element={<ProbeDashboard />} />
          <Route path="/communities" element={<CommunityRules />} />
          <Route path="/looking-glass" element={<LookingGlass />} />
          <Route path="/flaps" element={<FlapDashboard />} />
        </Routes>
        </ErrorBoundary>
      </div>
    </Layout>
  );
}
