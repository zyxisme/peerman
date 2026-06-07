import { type ReactNode } from 'react';

interface ResponsiveTableProps {
  children: ReactNode;
  className?: string;
}

export function ResponsiveTable({ children, className }: ResponsiveTableProps) {
  return (
    <div className={className}>
      <table className="data-table responsive-table">
        {children}
      </table>
    </div>
  );
}
