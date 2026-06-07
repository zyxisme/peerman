export interface TextareaProps {
  label: string;
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  code?: boolean;
}

export function Textarea({
  label, value, onChange, placeholder, code,
}: TextareaProps) {
  return (
    <div className="flex flex-col gap-1">
      <label className="text-caption text-mute">{label}</label>
      <textarea
        value={value}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
        rows={3}
        className={code ? 'form-input font-mono' : 'form-input'}
        style={code ? { fontFamily: 'Geist Mono, ui-monospace, monospace', fontSize: '13px' } : undefined}
      />
    </div>
  );
}
