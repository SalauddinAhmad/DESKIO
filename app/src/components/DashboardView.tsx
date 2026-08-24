import React from "react";
import { InstalledApp, OrphanGroup, JunkGroup, Section } from "../types";
import { humanSize } from "../api";
import { IconApps, IconBroom, IconTerminal, IconShield, IconCheck, IconWarn } from "./icons";

interface DashboardViewProps {
  apps: InstalledApp[];
  orphans: OrphanGroup[];
  junk: JunkGroup[];
  fdaGranted: boolean;
  onNavigate: (section: Section) => void;
  onRefreshAll?: () => void;
}

export const DashboardView: React.FC<DashboardViewProps> = ({
  apps,
  orphans,
  junk,
  fdaGranted,
  onNavigate,
  onRefreshAll: _onRefreshAll,
}) => {
  const totalAppsSize = apps.reduce((sum, a) => sum + (a.size_bytes || 0), 0);
  const totalOrphansSize = orphans.reduce((sum, o) => sum + (o.size_bytes || 0), 0);
  const totalJunkSize = junk.filter(j => j.removable).reduce((sum, j) => sum + (j.size_bytes || 0), 0);
  const totalCleanable = totalOrphansSize + totalJunkSize;

  const runningAppsCount = apps.filter(a => a.is_running).length;

  return (
    <div className="dashboard-container">
      {/* Header Banner */}
      <div className="dashboard-hero">
        <div className="hero-content">
          <div className="hero-badge">
            <span className="pulse-dot"></span>
            <span>DESKIO Engine v1.0.0</span>
          </div>
          <h1>DESKIO</h1>
          <p>Organize. Clean. Simplify.</p>
        </div>

        <div className="hero-stats-card">
          <div className="stat-circle">
            <svg viewBox="0 0 36 36" className="circular-chart cyan">
              <path
                className="circle-bg"
                d="M18 2.0845 a 15.9155 15.9155 0 0 1 0 31.831 a 15.9155 15.9155 0 0 1 0 -31.831"
              />
              <path
                className="circle"
                strokeDasharray="92, 100"
                d="M18 2.0845 a 15.9155 15.9155 0 0 1 0 31.831 a 15.9155 15.9155 0 0 1 0 -31.831"
              />
              <text x="18" y="20.35" className="percentage">92%</text>
            </svg>
          </div>
          <div className="stat-text">
            <span className="stat-label">System Health</span>
            <span className="stat-value">Optimal</span>
          </div>
        </div>
      </div>

      {/* Quick Action Grid */}
      <div className="dashboard-grid">
        <div className="dash-card cleanable-card">
          <div className="card-header">
            <div className="icon-wrapper cyan">
              <IconBroom />
            </div>
            <div>
              <h3>Reclaimable Space</h3>
              <p className="card-sub">Leftovers & Junk files ready for cleanup</p>
            </div>
          </div>
          <div className="card-big-value">
            {humanSize(totalCleanable)}
          </div>
          <div className="card-actions">
            <button className="btn-primary" onClick={() => onNavigate("cleanup")}>
              Clean Junk Files
            </button>
            <button className="btn-secondary" onClick={() => onNavigate("remaining")}>
              Review Leftovers
            </button>
          </div>
        </div>

        <div className="dash-card apps-summary-card">
          <div className="card-header">
            <div className="icon-wrapper violet">
              <IconApps />
            </div>
            <div>
              <h3>Applications</h3>
              <p className="card-sub">{apps.length} installed applications</p>
            </div>
          </div>
          <div className="card-big-value">
            {humanSize(totalAppsSize)}
          </div>
          <div className="meta-row">
            <span className="meta-badge">{runningAppsCount} Running Apps</span>
            <span className="meta-badge">{orphans.length} Leftover Groups</span>
          </div>
          <div className="card-actions">
            <button className="btn-secondary" onClick={() => onNavigate("applications")}>
              Manage Applications
            </button>
          </div>
        </div>

        <div className="dash-card dev-summary-card">
          <div className="card-header">
            <div className="icon-wrapper emerald">
              <IconTerminal />
            </div>
            <div>
              <h3>Developer Caches</h3>
              <p className="card-sub">node_modules, Cargo targets & build junk</p>
            </div>
          </div>
          <div className="card-big-value">
            {humanSize(totalJunkSize)}
          </div>
          <div className="card-actions">
            <button className="btn-secondary" onClick={() => onNavigate("dev_clean")}>
              Scan Dev Build Cache
            </button>
          </div>
        </div>
      </div>

      {/* Safety & System Status */}
      <div className="dash-status-row">
        <div className={`status-pill ${fdaGranted ? "granted" : "warning"}`}>
          <div className="pill-icon">
            {fdaGranted ? <IconCheck /> : <IconWarn />}
          </div>
          <div className="pill-info">
            <span className="pill-title">
              Full Disk Access: {fdaGranted ? "Granted" : "Limited Access"}
            </span>
            <span className="pill-desc">
              {fdaGranted
                ? "DESKIO has full permissions to perform deep leftover sweeps."
                : "Grant access in System Settings for complete leftover detection."}
            </span>
          </div>
        </div>

        <div className="status-pill safe">
          <div className="pill-icon">
            <IconShield />
          </div>
          <div className="pill-info">
            <span className="pill-title">100% Reversible Trash Safety</span>
            <span className="pill-desc">
              Every item is moved to system Trash before deletion. Restore anytime.
            </span>
          </div>
        </div>
      </div>
    </div>
  );
};
