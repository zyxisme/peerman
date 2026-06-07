export interface ToggleProps {
  label: string;
  description?: string;
  checked: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
}

export function Toggle({
  label, description, checked, onChange, disabled,
}: ToggleProps) {
  return (
    <div className="flex items-start gap-sm">
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        disabled={disabled}
        onClick={() => onChange(!checked)}
        className={`relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none ${checked ? 'bg-cyan' : 'bg-hairline'}`}
      >
        <span
          className={`pointer-events-none inline-block h-4 w-4 rounded-full bg-white shadow-sm ring-0 transition duration-200 ease-in-out ${checked ? 'translate-x-4' : 'translate-x-0'}`}
        />
      </button>
      <div className="flex flex-col gap-0.5">
        <span className="text-body-sm-strong text-ink">{label}</span>
        {description && <span className="text-caption text-mute">{description}</span>}
      </div>
    </div>
  );
}
