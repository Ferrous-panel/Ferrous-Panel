export type ActixGatewaySubsystem =
  | 'UFW'
  | 'NGINX'
  | 'SYSTEMD'
  | 'SYSCTL'
  | 'SYSCALL';

const INTERNAL_GATEWAY_BASIC_AUTH =
  'Basic ZmVycm91c19pbnRlcm5hbF9ndzpGcCNBY3RpeEd3MjAyNiFTdHJpY3Q=';

/**
 * Secure Local Actix-Web Gateway Client (`127.0.0.1:9443` -> `rust-backend/target/release/ferrous-system-core`)
 * Ensures all system-sensitive operations (UFW, Nginx/FerrousX, Systemd, Sysctl, Syscall)
 * route exclusively through the compiled Rust binary via the local Actix-Web security gateway
 * with HTTP Basic Authentication.
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
    Authorization: INTERNAL_GATEWAY_BASIC_AUTH,
    'X-Actix-Basic-Auth': INTERNAL_GATEWAY_BASIC_AUTH,
    'X-Ferrous-Gateway-Target': 'actix-web-local://127.0.0.1:9443',
    'X-Ferrous-Binary-Enforce':
      'rust-backend/target/release/ferrous-system-core',
    'X-Ferrous-Subsystem': subsystem,
    ...(init?.headers ? (init.headers as Record<string, string>) : {})
  };
  return fetch(gatewayUrl, {
    ...init,
    headers: mergedHeaders
  });
}
