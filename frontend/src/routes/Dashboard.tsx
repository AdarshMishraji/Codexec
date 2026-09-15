import { useEffect, useRef, useState } from "react";
import { Bar, Doughnut, Line } from "react-chartjs-2";
import { getStats } from "../lib/api";
import { commonGrid, commonTicks } from "../lib/chartSetup";
import { fmtKb, fmtMs, fmtNum, fmtPct, timeAgo } from "../lib/format";
import type { StatsResponse, SubmissionStatus, SubmissionVerdict } from "../lib/types";

const STATUS_LABELS: Record<SubmissionStatus, string> = {
  queued: "Queued",
  processing: "Processing",
  completed: "Completed",
  compile_error: "Compile Error",
  runtime_error: "Runtime Error",
  time_limit_exceeded: "Time Limit Exceeded",
  memory_limit_exceeded: "Memory Limit Exceeded",
  internal_error: "Internal Error",
};
const STATUS_COLORS: Record<SubmissionStatus, string> = {
  queued: "#5b9dff",
  processing: "#4fd8e8",
  completed: "#3ddc97",
  compile_error: "#ff6b6b",
  runtime_error: "#ff6b6b",
  time_limit_exceeded: "#f5b942",
  memory_limit_exceeded: "#b88cff",
  internal_error: "#8892a6",
};
const VERDICT_LABELS: Record<SubmissionVerdict, string> = { accepted: "Accepted", wrong_answer: "Wrong Answer" };
const VERDICT_COLORS: Record<SubmissionVerdict, string> = { accepted: "#3ddc97", wrong_answer: "#ff6b6b" };
const LANG_PALETTE = ["#6d8dff", "#3ddc97", "#f5b942", "#b88cff", "#4fd8e8", "#ff6b6b", "#5b9dff", "#e879f9"];

function KpiCard({ label, value, meta, pos }: { label: string; value: string; meta: string; pos?: boolean }) {
  return (
    <div className="kpi">
      <div className="label">{label}</div>
      <div className={"value" + (value.length > 8 ? " small" : "")}>{value}</div>
      <div className={"meta" + (pos ? " pos" : "")}>{meta}</div>
    </div>
  );
}

function trendConfig(daily: StatsResponse["daily_trend"]) {
  return {
    labels: daily.map((d) => new Date(d.day).toLocaleDateString(undefined, { month: "short", day: "numeric" })),
    datasets: [
      {
        label: "Submissions",
        data: daily.map((d) => d.count),
        borderColor: "#6d8dff",
        backgroundColor: (c: { chart: { ctx: CanvasRenderingContext2D } }) => {
          const g = c.chart.ctx.createLinearGradient(0, 0, 0, 240);
          g.addColorStop(0, "rgba(109,141,255,0.35)");
          g.addColorStop(1, "rgba(109,141,255,0.0)");
          return g;
        },
        fill: true,
        tension: 0.35,
        pointRadius: 2.5,
        pointBackgroundColor: "#6d8dff",
        borderWidth: 2.2,
      },
    ],
  };
}

function statusConfig(breakdown: StatsResponse["status_breakdown"]) {
  return {
    labels: breakdown.map((b) => STATUS_LABELS[b.status] ?? b.status),
    datasets: [
      {
        data: breakdown.map((b) => b.count),
        backgroundColor: breakdown.map((b) => STATUS_COLORS[b.status] ?? "#5b6478"),
        borderColor: "#131826",
        borderWidth: 3,
      },
    ],
  };
}

function langConfig(breakdown: StatsResponse["language_breakdown"]) {
  const top = breakdown.slice(0, 8);
  return {
    labels: top.map((b) => b.display_name),
    datasets: [
      {
        data: top.map((b) => b.count),
        backgroundColor: top.map((_, i) => LANG_PALETTE[i % LANG_PALETTE.length]),
        borderRadius: 6,
        maxBarThickness: 26,
      },
    ],
  };
}

function verdictConfig(breakdown: StatsResponse["verdict_breakdown"]) {
  return {
    labels: breakdown.map((b) => VERDICT_LABELS[b.verdict] ?? b.verdict),
    datasets: [
      {
        data: breakdown.map((b) => b.count),
        backgroundColor: breakdown.map((b) => VERDICT_COLORS[b.verdict] ?? "#5b6478"),
        borderColor: "#131826",
        borderWidth: 3,
      },
    ],
  };
}

const doughnutOptions = {
  responsive: true,
  maintainAspectRatio: false,
  cutout: "68%",
  plugins: {
    legend: { position: "bottom" as const, labels: { color: "#8892a6", font: { size: 11 }, boxWidth: 10, padding: 12 } },
  },
};

export default function Dashboard() {
  const [stats, setStats] = useState<StatsResponse | null>(null);
  const [refreshLabel, setRefreshLabel] = useState("live");
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    async function refresh() {
      try {
        const s = await getStats();
        if (!mounted.current) return;
        setStats(s);
        setRefreshLabel("updated " + new Date().toLocaleTimeString());
      } catch (e) {
        if (!mounted.current) return;
        setRefreshLabel("connection error");
        console.error(e);
      }
    }
    refresh();
    const id = setInterval(refresh, 15000);
    return () => {
      mounted.current = false;
      clearInterval(id);
    };
  }, []);

  return (
    <div className="wrap">
      <header className="top">
        <div className="brand">
          <img className="brand-mark" src="/icon.png" alt="" />
          <div>
            <h1>Codexec</h1>
            <span className="sub">Code execution platform — live statistics</span>
          </div>
        </div>
        <div className="top-actions">
          <span className="live-pill">
            <span className="live-dot"></span> <span>{refreshLabel}</span>
          </span>
          <a className="link" href="/docs">API Docs</a>
          <a className="link" href="/admin">Admin Portal →</a>
        </div>
      </header>

      <section className="kpi-grid">
        {stats && (
          <>
            <KpiCard label="Total Submissions" value={fmtNum(stats.total_submissions)} meta={`${fmtNum(stats.submissions_last_24h)} in last 24h`} />
            <KpiCard label="Success Rate" value={fmtPct(stats.success_rate_pct)} meta={`${fmtNum(stats.completed_submissions)} completed`} pos />
            <KpiCard label="Active Languages" value={`${stats.languages_active} / ${stats.languages_total}`} meta="registered plugins" />
            <KpiCard label="API Keys" value={`${stats.api_keys_active} / ${stats.api_keys_total}`} meta="active / total" />
            <KpiCard label="Avg CPU Time" value={fmtMs(stats.avg_cpu_time_ms)} meta={`p95 ${fmtMs(stats.p95_cpu_time_ms)}`} />
            <KpiCard label="Avg Memory" value={fmtKb(stats.avg_memory_kb)} meta={`wall avg ${fmtMs(stats.avg_wall_time_ms)}`} />
          </>
        )}
      </section>

      <section className="charts-grid">
        <div className="panel">
          <h2>
            Submissions — last 14 days
            <span className="hint">{stats ? `${stats.daily_trend.reduce((a, d) => a + d.count, 0)} total` : ""}</span>
          </h2>
          <div className="chart-box tall">
            {stats && (
              <Line
                data={trendConfig(stats.daily_trend)}
                options={{
                  responsive: true,
                  maintainAspectRatio: false,
                  plugins: { legend: { display: false } },
                  scales: {
                    x: { grid: { display: false }, ticks: commonTicks },
                    y: { beginAtZero: true, grid: commonGrid, ticks: { ...commonTicks, precision: 0 } },
                  },
                }}
              />
            )}
          </div>
        </div>
        <div className="panel">
          <h2>Status breakdown</h2>
          <div className="chart-box tall">
            {stats && (stats.status_breakdown.length ? (
              <Doughnut data={statusConfig(stats.status_breakdown)} options={doughnutOptions} />
            ) : (
              <div className="empty-state">No submissions yet</div>
            ))}
          </div>
        </div>
      </section>

      <section className="charts-grid secondary">
        <div className="panel">
          <h2>Submissions by language</h2>
          <div className="chart-box">
            {stats && (stats.language_breakdown.length ? (
              <Bar
                data={langConfig(stats.language_breakdown)}
                options={{
                  indexAxis: "y" as const,
                  responsive: true,
                  maintainAspectRatio: false,
                  plugins: { legend: { display: false } },
                  scales: {
                    x: { beginAtZero: true, grid: commonGrid, ticks: { ...commonTicks, precision: 0 } },
                    y: { grid: { display: false }, ticks: commonTicks },
                  },
                }}
              />
            ) : (
              <div className="empty-state">No submissions yet</div>
            ))}
          </div>
        </div>
        <div className="panel">
          <h2>
            Verdicts <span className="hint">graded submissions only</span>
          </h2>
          <div className="chart-box">
            {stats && (stats.verdict_breakdown.length ? (
              <Doughnut data={verdictConfig(stats.verdict_breakdown)} options={doughnutOptions} />
            ) : (
              <div className="empty-state">No graded submissions yet</div>
            ))}
          </div>
        </div>
      </section>

      <section className="panel" style={{ marginBottom: 16 }}>
        <h2>
          Recent submissions <span className="hint">metadata only — no source or output</span>
        </h2>
        {stats && (stats.recent_submissions.length ? (
          <div style={{ overflowX: "auto" }}>
            <table className="recent">
              <thead>
                <tr>
                  <th>ID</th>
                  <th>Language</th>
                  <th>Status</th>
                  <th>Verdict</th>
                  <th>CPU Time</th>
                  <th>Memory</th>
                  <th>Submitted</th>
                </tr>
              </thead>
              <tbody>
                {stats.recent_submissions.map((r) => (
                  <tr key={r.id}>
                    <td className="mono">{r.id.slice(0, 8)}</td>
                    <td>{r.language_slug}</td>
                    <td>
                      <span className={"badge " + r.status}>{STATUS_LABELS[r.status] ?? r.status}</span>
                    </td>
                    <td>
                      <span className={"verdict-tag " + (r.verdict ?? "none")}>
                        {r.verdict ? VERDICT_LABELS[r.verdict] : "—"}
                      </span>
                    </td>
                    <td>{fmtMs(r.cpu_time_used_ms)}</td>
                    <td>{fmtKb(r.memory_used_kb)}</td>
                    <td className="mono">{timeAgo(r.submitted_at)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <div className="empty-state">No submissions yet</div>
        ))}
      </section>

      <footer className="page">codexec platform dashboard · refreshes automatically every 15s</footer>
    </div>
  );
}
