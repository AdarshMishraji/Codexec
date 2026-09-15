export function fmtNum(n: number | null | undefined): string {
  if (n === null || n === undefined) return "—";
  return new Intl.NumberFormat().format(n);
}

export function fmtMs(n: number | null | undefined): string {
  if (n === null || n === undefined) return "—";
  if (n < 1000) return n.toFixed(1) + " ms";
  return (n / 1000).toFixed(2) + " s";
}

export function fmtKb(n: number | null | undefined): string {
  if (n === null || n === undefined) return "—";
  if (n < 1024) return n.toFixed(0) + " KB";
  return (n / 1024).toFixed(1) + " MB";
}

export function fmtPct(n: number | null | undefined): string {
  return n === null || n === undefined ? "—" : n.toFixed(1) + "%";
}

export function timeAgo(iso: string | null | undefined): string {
  if (!iso) return "—";
  const diff = (Date.now() - new Date(iso).getTime()) / 1000;
  if (diff < 60) return Math.max(0, Math.floor(diff)) + "s ago";
  if (diff < 3600) return Math.floor(diff / 60) + "m ago";
  if (diff < 86400) return Math.floor(diff / 3600) + "h ago";
  return Math.floor(diff / 86400) + "d ago";
}

export function fmtDate(iso: string | null | undefined): string {
  return iso ? new Date(iso).toLocaleString() : "—";
}
