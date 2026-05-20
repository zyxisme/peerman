import { useState } from 'react';
import { Copy, Download, Check } from 'lucide-react';

interface ConfigViewerProps {
  content: string;
  loading: boolean;
  filename: string;
  title: string;
}

export default function ConfigViewer({ content, loading, filename, title }: ConfigViewerProps) {
  const [copied, setCopied] = useState(false);

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
    a.download = filename;
    a.click();
    URL.revokeObjectURL(url);
  };

  if (loading) {
    return <div className="text-body-sm text-body py-lg">Loading configuration...</div>;
  }

  if (!content) {
    return <div className="text-body-sm text-mute py-lg">No configuration available.</div>;
  }

  return (
    <div>
      <div className="flex items-center justify-between mb-sm">
        <h3 className="text-body-sm-strong text-ink">{title}</h3>
        <div className="flex items-center gap-1">
          <button
            onClick={handleCopy}
            className="btn-ghost text-xs"
          >
            {copied ? <Check className="w-3 h-3" /> : <Copy className="w-3 h-3" />}
            {copied ? 'Copied' : 'Copy'}
          </button>
          <button onClick={handleDownload} className="btn-ghost text-xs">
            <Download className="w-3 h-3" />
            Download
          </button>
        </div>
      </div>
      <pre className="code-block text-xs">{content}</pre>
    </div>
  );
}
