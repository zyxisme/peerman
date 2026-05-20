import { useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { useAuth } from '../../lib/auth';

export default function LoginPage() {
  const { login } = useAuth();
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [submitting, setSubmitting] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    setSubmitting(true);

    const ok = await login(username, password);
    if (ok) {
      const redirect = searchParams.get('redirect') || '/';
      navigate(redirect, { replace: true });
    } else {
      setError('Invalid username or password');
      setSubmitting(false);
    }
  };

  return (
    <div className="max-w-md mx-auto mt-24">
      <div className="card">
        <h1 className="text-display-sm text-ink mb-lg">Login</h1>
        <form onSubmit={handleSubmit} className="space-y-md">
          <div>
            <label htmlFor="username" className="text-body-sm-strong text-ink block mb-xs">
              Username
            </label>
            <input
              id="username"
              type="text"
              className="form-input"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              autoFocus
              required
            />
          </div>
          <div>
            <label htmlFor="password" className="text-body-sm-strong text-ink block mb-xs">
              Password
            </label>
            <input
              id="password"
              type="password"
              className="form-input"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              required
            />
          </div>
          {error && (
            <p className="text-error text-body-sm">{error}</p>
          )}
          <button type="submit" className="btn-primary w-full" disabled={submitting}>
            {submitting ? 'Logging in...' : 'Login'}
          </button>
        </form>
      </div>
    </div>
  );
}
