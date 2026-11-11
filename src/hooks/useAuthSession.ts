import { useEffect, useState, useCallback } from 'react';

export function useAuthSession(onAuthenticatedFetch?: () => void) {
  const [sessionToken, setSessionToken] = useState<string | null>(() => {
    return localStorage.getItem('fp_admin_session_token') || null;
  });
  const [isAuthenticated, setIsAuthenticated] = useState<boolean>(() => {
    return Boolean(localStorage.getItem('fp_admin_session_token'));
  });
  const [currentUser, setCurrentUser] = useState<{ username: string } | null>(
    () => {
      const savedUser = localStorage.getItem('fp_admin_user');
      return savedUser ? { username: savedUser } : null;
    }
  );

  useEffect(() => {
    const verifySession = async () => {
      const token = localStorage.getItem('fp_admin_session_token');
      if (!token) {
        setIsAuthenticated(false);
        setCurrentUser(null);
        return;
      }
      try {
        const res = await fetch('/api/auth/session', {
          headers: { Authorization: `Bearer ${token}` }
        });
        if (res.ok) {
          const data = await res.json();
          if (data.authenticated) {
            setIsAuthenticated(true);
            setCurrentUser({
              username: data.user?.username || 'admin_ferrous'
            });
          } else {
            setIsAuthenticated(false);
            setCurrentUser(null);
            localStorage.removeItem('fp_admin_session_token');
            localStorage.removeItem('fp_admin_user');
          }
        } else {
          setIsAuthenticated(false);
          setCurrentUser(null);
          localStorage.removeItem('fp_admin_session_token');
          localStorage.removeItem('fp_admin_user');
        }
      } catch {
        // network issue - maintain offline session if present
      }
    };
    verifySession();
  }, []);

  const handleLoginSuccess = useCallback(
    (token: string, user: { username: string }) => {
      localStorage.setItem('fp_admin_session_token', token);
      localStorage.setItem('fp_admin_user', user.username);
      setSessionToken(token);
      setCurrentUser(user);
      setIsAuthenticated(true);
      if (onAuthenticatedFetch) {
        onAuthenticatedFetch();
      }
    },
    [onAuthenticatedFetch]
  );

  const handleLogout = useCallback(async () => {
    try {
      if (sessionToken) {
        await fetch('/api/auth/logout', {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            Authorization: `Bearer ${sessionToken}`
          },
          body: JSON.stringify({ token: sessionToken })
        });
      }
    } catch {
      // ignore
    } finally {
      localStorage.removeItem('fp_admin_session_token');
      localStorage.removeItem('fp_admin_user');
      setSessionToken(null);
      setCurrentUser(null);
      setIsAuthenticated(false);
    }
  }, [sessionToken]);

  return {
    sessionToken,
    isAuthenticated,
    currentUser,
    handleLoginSuccess,
    handleLogout
  };
}
