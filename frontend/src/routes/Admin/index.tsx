import AdminGate from "./AdminGate";
import AdminLayout from "./AdminLayout";
import { useAdminSession } from "./useAdminSession";

export default function Admin() {
  const { token, gateError, unlock, lock } = useAdminSession();

  if (!token) return <AdminGate error={gateError} onUnlock={unlock} />;
  return <AdminLayout token={token} onLock={() => lock()} onSessionExpired={() => lock("Session expired — token was rejected.")} />;
}
