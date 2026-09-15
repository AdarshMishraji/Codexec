import { useCallback, useState } from "react";
import { verifyAdminToken } from "../../lib/api";
import { ADMIN_TOKEN_KEY } from "../../lib/storage";

export function useAdminSession() {
  const [token, setToken] = useState<string | null>(() => localStorage.getItem(ADMIN_TOKEN_KEY));
  const [gateError, setGateError] = useState<string | null>(null);

  const unlock = useCallback(async (candidate: string) => {
    setGateError(null);
    try {
      await verifyAdminToken(candidate);
      localStorage.setItem(ADMIN_TOKEN_KEY, candidate);
      setToken(candidate);
    } catch {
      setGateError("Invalid token.");
    }
  }, []);

  const lock = useCallback((reason?: string) => {
    localStorage.removeItem(ADMIN_TOKEN_KEY);
    setToken(null);
    setGateError(reason ?? null);
  }, []);

  return { token, gateError, unlock, lock };
}
