import { useState, useEffect } from 'react';
import { Download, Copy, Check } from 'lucide-react';
import { peerClient } from '../../lib/grpc';
import type { ConfigResponse } from '../../lib/peerman_pb';

export default function BirdAllConfig() {
  const [content, setContent] = useState('');
  const [loading, setLoading] = useState(true);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    (async () => {
      try {
        const res: ConfigResponse = await peerClient.exportAllBird({});
        setContent(res.content);
      } finally {
        setLoading(false);
      }
    })();
  }, []);

  const handleCopy = async () => {
    await navigator.clipboard.writeText(content);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleDownload = () => {
    const blob = new Blob([content], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'bird-peers.conf';
    a.click();
    URL.revokeObjectURL(url);
  };

  if (loading) return <div className="text-body-sm text-body py-lg">Generating...</div>;

  return (
    <div>
      <div className="flex items-center justify-between mb-sm">
        <h3 className="text-body-sm-strong text-ink">All Peers — BIRD2 Config</h3>
        <div className="flex items-center gap-1">
          <button onClick={handleCopy} className="btn-ghost text-xs">
            {copied ? <Check className="w-3 h-3" /> : <Copy className="w-3 h-3" />}
            {copied ? 'Copied' : 'Copy'}
          </button>
          <button onClick={handleDownload} className="btn-ghost text-xs">
            <Download className="w-3 h-3" />
            Download
          </button>
        </div>
      </div>
      <pre className="code-block text-xs">{content || 'No peers configured.'}</pre>
    </div>
  );
}
