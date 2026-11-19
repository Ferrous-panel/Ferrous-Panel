export type ActixGatewaySubsystem =
  | 'UFW'
  | 'NGINX'
  | 'SYSTEMD'
  | 'SYSCTL'
  | 'SYSCALL';

/**
 * Shared authenticated API fetch helper attaching the active administrator
 * Bearer session token, X-FP-Session header, and same-origin cookies.
 */
export async function authenticatedFetch(
  input: RequestInfo | URL,
  init?: RequestInit
): Promise<Response> {
  const sessionToken =
    typeof window !== 'undefined'
      ? window.localStorage.getItem('fp_admin_session_token')
      : null;

  const authHeaders: Record<string, string> = sessionToken
    ? {
        Authorization: `Bearer ${sessionToken}`,
        'X-FP-Session': sessionToken
      }
    : {};

  const mergedHeaders: Record<string, string> = {
    ...authHeaders,
    ...(init?.headers ? (init.headers as Record<string, string>) : {})
  };

  return fetch(input, {
    credentials: 'same-origin',
    ...init,
    headers: mergedHeaders
  });
}

/**
 * Gateway API client forwarding system subsystem requests with the active
 * administrator Bearer session token stored on the client.
 */
export async function actixGatewayFetch(
  subsystem: ActixGatewaySubsystem,
  endpointPath: string,
  init?: RequestInit
): Promise<Response> {
  const cleanSubPath = endpointPath.startsWith('/api/')
    ? endpointPath.slice(4)
    : endpointPath.startsWith('/')
      ? endpointPath
      : `/${endpointPath}`;
  const gatewayUrl = `/api/actix-gateway/${subsystem.toLowerCase()}${cleanSubPath}`;

  const mergedHeaders: Record<string, string> = {
    'X-Ferrous-Subsystem': subsystem,
    ...(init?.headers ? (init.headers as Record<string, string>) : {})
  };
  return authenticatedFetch(gatewayUrl, {
    ...init,
    headers: mergedHeaders
  });
}
