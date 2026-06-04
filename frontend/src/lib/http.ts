/**
 * Fetch wrapper with automatic 401 handling.
 * Redirects to login when JWT expires.
 */
export async function fetchJson<T>(
  url: string,
  init?: RequestInit,
): Promise<T> {
  const res = await fetch(url, {
    ...init,
    credentials: 'same-origin',
  });

  if (res.status === 401) {
    const current = window.location.pathname;
    window.location.href = `/login?redirect=${encodeURIComponent(current)}`;
    throw new Error('Unauthorized');
  }

  if (!res.ok) {
    const text = await res.text().catch(() => res.statusText);
    throw new Error(`${res.status}: ${text}`);
  }

  return res.json();
}

/**
 * Fetch that returns raw Response (for non-JSON endpoints).
 * Still handles 401 redirects.
 */
export async function fetchWithAuth(
  url: string,
  init?: RequestInit,
): Promise<Response> {
  const res = await fetch(url, {
    ...init,
    credentials: 'same-origin',
  });

  if (res.status === 401) {
    const current = window.location.pathname;
    window.location.href = `/login?redirect=${encodeURIComponent(current)}`;
    throw new Error('Unauthorized');
  }

  return res;
}
