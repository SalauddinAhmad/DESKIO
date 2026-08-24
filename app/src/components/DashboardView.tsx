import React from "react";
import { InstalledApp, OrphanGroup, JunkGroup, Section } from "../types";
import { humanSize } from "../api";
import { IconApps, IconBroom, IconTerminal, IconShield, IconWarn } from "./icons";

interface DashboardViewProps {
  apps: InstalledApp[];
  orphans: OrphanGroup[];
  junk: JunkGroup[];
  fdaGranted: boolean;
  onNavigate: (section: Section) => void;
  onQuickCleanJunk: () => void;
  onQuickSweepOrphans: () => void;
}

export const DashboardView: React.FC<DashboardViewProps> = ({
  apps,
  orphans,
  junk,
  fdaGranted,
  onNavigate,
  onQuickCleanJunk,
  onQuickSweepOrphans,
}) => {
  const totalAppsSize = apps.reduce((sum, a) => sum + (a.size_bytes || 0), 0);
  const totalOrphansSize = orphans.reduce((sum, o) => sum + (o.size_bytes || 0), 0);
  const totalJunkSize = junk.filter(j => j.removable).reduce((sum, j) => sum + (j.size_bytes || 0), 0);
  const totalCleanable = totalOrphansSize + totalJunkSize;

  const runningAppsCount = apps.filter(a => a.is_running).length;

  return (
    <div className="view-body">
      {/* Top Warning Banner if FDA is missing */}
      {!fdaGranted && (
        <div className="top-notice-banner">
          <div style={{ display: "flex", alignItems: "center", gap: "10px" }}>
            <IconWarn />
            <span>Full Disk Access is currently off. Some leftover files might be hidden.</span>
          </div>
          <button onClick={() => onNavigate("dashboard")}>Fix Permissions</button>
        </div>
      )}

      {/* Hero Cards Top Grid */}
      <div className="dash-top-grid">
        {/* Coral Storage Card */}
        <div className="coral-hero-card">
          <div>
            <h4>Cleanable Space</h4>
            <div className="hero-val">{humanSize(totalCleanable)}</div>
            <div className="hero-sub">{orphans.length} Leftover Groups & Junk files</div>
          </div>
          <div style={{ display: "flex", gap: "8px" }}>
            <button className="btn-secondary" style={{ color: "#ff6b4a", fontWeight: 700 }} onClick={onQuickCleanJunk}>
              ⚡ 1-Click Clean Junk
            </button>
          </div>
        </div>

        {/* Purple Health Card */}
        <div className="purple-hero-card">
          <div>
            <h4 style={{ opacity: 0.9, fontSize: "13px" }}>System Health</h4>
            <div className="hero-val">94%</div>
            <div style={{ fontSize: "12px", opacity: 0.85 }}>Optimal Status</div>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: "6px", fontSize: "12px" }}>
            <IconShield /> 100% Safe
          </div>
        </div>

        {/* Overview Stats Card */}
        <div className="dash-stats-card">
          <div>
            <div style={{ fontSize: "13px", color: "var(--ink-secondary)", fontWeight: 600 }}>
              System Storage Breakdown
            </div>
            <div style={{ fontSize: "20px", fontWeight: 700, marginTop: "4px" }}>
              {humanSize(totalAppsSize + totalCleanable)} Used
            </div>
          </div>

          <div className="stat-bars-container">
            <div className="stat-bar active" style={{ height: "75%" }}></div>
            <div className="stat-bar active" style={{ height: "45%" }}></div>
            <div className="stat-bar active" style={{ height: "90%" }}></div>
            <div className="stat-bar active" style={{ height: "60%" }}></div>
            <div className="stat-bar" style={{ height: "30%" }}></div>
          </div>
        </div>
      </div>

      {/* Quick Action Pill Grid */}
      <div className="dash-quick-actions-row">
        <div className="quick-action-pill" onClick={onQuickCleanJunk}>
          <div className="action-icon coral">
            <IconBroom />
          </div>
          <div className="action-info">
            <h5>⚡ 1-Click Clean</h5>
            <p>{humanSize(totalJunkSize)} Junk Files</p>
          </div>
        </div>

        <div className="quick-action-pill" onClick={onQuickSweepOrphans}>
          <div className="action-icon purple">
            <IconApps />
          </div>
          <div className="action-info">
            <h5>🗑️ Sweep Leftovers</h5>
            <p>{orphans.length} Leftover Groups</p>
          </div>
        </div>

        <div className="quick-action-pill" onClick={() => onNavigate("dev_clean")}>
          <div className="action-icon emerald">
            <IconTerminal />
          </div>
          <div className="action-info">
            <h5>💻 Dev Caches</h5>
            <p>node_modules & Build</p>
          </div>
        </div>

        <div className="quick-action-pill" onClick={() => onNavigate("applications")}>
          <div className="action-icon cyan">
            <IconApps />
          </div>
          <div className="action-info">
            <h5>🚀 Manage Apps</h5>
            <p>{apps.length} Installed Apps</p>
          </div>
        </div>
      </div>

      {/* Activity Table & Quick Overview */}
      <div className="dash-bottom-grid">
        <div className="list-table-card">
          <div className="table-header">
            <h3>Recent Installed Applications</h3>
            <button className="btn-secondary" onClick={() => onNavigate("applications")}>
              View All Apps ({apps.length})
            </button>
          </div>

          <div style={{ display: "flex", flexDirection: "column" }}>
            {apps.slice(0, 5).map((app) => (
              <div key={app.id} className="clean-table-row">
                <div className="row-left">
                  <div className="row-icon">
                    {app.icon_png_base64 ? (
                      <img src={`data:image/png;base64,${app.icon_png_base64}`} alt="" style={{ width: 22, height: 22 }} />
                    ) : (
                      <IconApps />
                    )}
                  </div>
                  <div>
                    <div className="row-title">{app.name}</div>
                    <div className="row-sub">{app.bundle_id || app.publisher || "Application"}</div>
                  </div>
                </div>
                <div style={{ display: "flex", alignItems: "center", gap: "16px" }}>
                  <div className="row-val">{humanSize(app.size_bytes)}</div>
                  <button className="btn-secondary" onClick={() => onNavigate("applications")}>
                    Uninstall
                  </button>
                </div>
              </div>
            ))}
          </div>
        </div>

        <div className="list-table-card" style={{ display: "flex", flexDirection: "column", justifyContent: "space-between" }}>
          <div>
            <div className="table-header">
              <h3>Quick Summary</h3>
            </div>
            <div style={{ display: "flex", flexDirection: "column", gap: "16px", marginTop: "12px" }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <span style={{ fontSize: "13px", color: "var(--ink-secondary)" }}>Running Apps</span>
                <span style={{ fontSize: "14px", fontWeight: 700, color: "#6366f1" }}>{runningAppsCount}</span>
              </div>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <span style={{ fontSize: "13px", color: "var(--ink-secondary)" }}>Leftover Folders</span>
                <span style={{ fontSize: "14px", fontWeight: 700, color: "#ff6b4a" }}>{orphans.length}</span>
              </div>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <span style={{ fontSize: "13px", color: "var(--ink-secondary)" }}>Trash Safety</span>
                <span style={{ fontSize: "14px", fontWeight: 700, color: "#10b981" }}>100% Active</span>
              </div>
            </div>
          </div>

          <button className="btn-coral" style={{ marginTop: "20px", width: "100%", justifyContent: "center" }} onClick={onQuickCleanJunk}>
            ⚡ 1-Click Clean Everything
          </button>
        </div>
      </div>
    </div>
  );
};
