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
import StatusPage from './components/status/StatusPage';
import LoginPage from './components/auth/LoginPage';
import { ProtectedRoute } from './lib/auth';

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
          <Route path="/peers/new" element={<ProtectedRoute><PeerForm /></ProtectedRoute>} />
          <Route path="/peers/:id" element={<PeerDetail />} />
          <Route path="/peers/:id/edit" element={<ProtectedRoute><PeerForm /></ProtectedRoute>} />
          <Route path="/export" element={<ProtectedRoute><ExportPage /></ProtectedRoute>} />
          <Route path="/settings" element={<ProtectedRoute><SettingsPage /></ProtectedRoute>} />
          <Route path="/nodes" element={<NodesTable />} />
          <Route path="/nodes/new" element={<ProtectedRoute><NodeForm /></ProtectedRoute>} />
          <Route path="/nodes/:id" element={<NodeDetail />} />
          <Route path="/nodes/:id/edit" element={<ProtectedRoute><NodeForm /></ProtectedRoute>} />
          <Route path="/probes" element={<ProbeDashboard />} />
          <Route path="/communities" element={<ProtectedRoute><CommunityRules /></ProtectedRoute>} />
          <Route path="/looking-glass" element={<LookingGlass />} />
          <Route path="/flaps" element={<FlapDashboard />} />
          <Route path="/login" element={<LoginPage />} />
          <Route path="/status" element={<StatusPage />} />
        </Routes>
        </ErrorBoundary>
      </div>
    </Layout>
  );
}
