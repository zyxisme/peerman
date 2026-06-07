export interface InputProps {
  label: string;
  value: string;
  onChange: (v: string) => void;
  type?: string;
  placeholder?: string;
  required?: boolean;
  mono?: boolean;
}

export function Input({
  label, value, onChange, type = 'text', placeholder, required, mono,
}: InputProps) {
  return (
    <div className="flex flex-col gap-1">
      <label className="text-caption text-mute">{label}</label>
      <input
        type={type}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        required={required}
        className={`form-input ${mono ? 'font-mono' : ''}`}
      />
    </div>
  );
}
