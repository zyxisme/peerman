import type { ReactNode } from 'react';
import NavBar from './NavBar';

interface LayoutProps {
  children: ReactNode;
}

export default function Layout({ children }: LayoutProps) {
  return (
    <div className="min-h-screen flex flex-col">
      <NavBar />
      <main className="flex-1 max-w-[1400px] w-full mx-auto">
        {children}
      </main>
      <footer className="bg-canvas border-t border-hairline px-lg py-2xl text-body-sm text-body">
        <div className="max-w-[1400px] mx-auto">
          Peerman &mdash; DN42 Peer Manager
        </div>
      </footer>
    </div>
  );
}
