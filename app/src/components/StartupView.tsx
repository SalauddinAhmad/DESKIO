/** Startup Programs: what runs when you log in.
 *
 *  Nothing here is deleted. A switch turns an entry off through the operating
 *  system's own mechanism, so it can always be turned back on — which is why
 *  this section has switches rather than a Remove button.
 */
import { useMemo, useState } from "react";
import type { StartupItem } from "../types";
import { STARTUP_KIND_LABEL } from "../types";
import { ArtEmpty, IconSearch, IconWarn } from "./icons";

type Filter = "all" | "active" | "disabled";

interface Props {
  items: StartupItem[];
  banner: React.ReactNode;
  loading: boolean;
  busyId: string | null;
  error: string | null;
  onToggle: (item: StartupItem, enabled: boolean) => void;
}

export function StartupView({ items, banner, loading, busyId, error, onToggle }: Props) {
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [focusedId, setFocusedId] = useState<string | null>(null);

  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return items.filter((i) => {
      if (filter === "active" && !i.enabled) return false;
      if (filter === "disabled" && i.enabled) return false;
      if (!q) return true;
      return (
        i.name.toLowerCase().includes(q) ||
        i.id.toLowerCase().includes(q) ||
        (i.program ?? "").toLowerCase().includes(q)
      );
    });
  }, [items, query, filter]);

  const focused = items.find((i) => i.id === focusedId) ?? null;

  return (
    <>
      <div className="list-col">
        <div className="list-head" data-tauri-drag-region>
          <h1 className="list-title">Startup Programs</h1>
        </div>
        <div className="list-tools">
          <div className="search">
            <IconSearch />
            <input
              placeholder="Search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </div>
        </div>

        <div className="segmented">
          {(["all", "active", "disabled"] as Filter[]).map((f) => (
            <button
              key={f}
              className="segment"
              aria-current={filter === f}
              onClick={() => setFilter(f)}
            >
              {f === "all" ? "All programs" : f === "active" ? "Active" : "Disabled"}
            </button>
          ))}
        </div>

        {banner}
        <div className="count-line">
          {loading
            ? "Reading startup items…"
            : `${shown.length} item${shown.length === 1 ? "" : "s"}`}
        </div>

        <div className="list-scroll">
          {shown.map((item) => (
            <div
              key={item.id}
              className="card"
              aria-selected={item.id === focusedId}
              onClick={() => setFocusedId(item.id)}
            >
              <div className="card-main">
                <div className="card-name">{item.name}</div>
                <div className="card-sub">
                  {STARTUP_KIND_LABEL[item.kind]}
                  {item.requires_admin ? " · needs admin" : ""}
                </div>
              </div>
              {item.can_toggle ? (
                <label className="toggle" onClick={(e) => e.stopPropagation()}>
                  <input
                    type="checkbox"
                    checked={item.enabled}
                    disabled={busyId === item.id}
                    onChange={(e) => onToggle(item, e.target.checked)}
                  />
                </label>
              ) : (
                <span className="pill pill-low" title={item.locked_reason ?? ""}>
                  managed
                </span>
              )}
            </div>
          ))}
        </div>
      </div>

      <div className="detail">
        {!focused ? (
          <div className="empty">
            <ArtEmpty />
            <h2>Startup Programs</h2>
            <p>
              Programs and processes that run automatically when you log in. Switching
              one off does not remove it — you can switch it back on at any time.
            </p>
          </div>
        ) : (
          <>
            <div className="detail-top" data-tauri-drag-region />
            <div className="detail-body">
              <div className="hero">
                <div className="hero-main">
                  <div className="hero-name-row">
                    <div className="hero-name">{focused.name}</div>
                    <div className="hero-size">{focused.enabled ? "On" : "Off"}</div>
                  </div>
                  <dl className="hero-rows">
                    <div className="hero-row">
                      <dt>Type</dt>
                      <dd>{STARTUP_KIND_LABEL[focused.kind]}</dd>
                    </div>
                    <div className="hero-row">
                      <dt>Identifier</dt>
                      <dd title={focused.id}>{focused.id}</dd>
                    </div>
                    <div className="hero-row">
                      <dt>Runs as</dt>
                      <dd>
                        {focused.kind === "launch_daemon"
                          ? "root, at startup"
                          : "you, at login"}
                      </dd>
                    </div>
                  </dl>
                </div>
              </div>

              <h2 className="section-title">Details</h2>
              <dl className="table">
                <div className="table-row">
                  <dt>Program</dt>
                  <dd title={focused.program ?? ""}>{focused.program ?? "—"}</dd>
                </div>
                <div className="table-row">
                  <dt>Defined in</dt>
                  <dd title={focused.path ?? ""}>{focused.path ?? "—"}</dd>
                </div>
              </dl>

              {!focused.can_toggle && focused.locked_reason && (
                <div className="banner" style={{ margin: "16px 0 0" }}>
                  <IconWarn />
                  <div>{focused.locked_reason}</div>
                </div>
              )}
              {error && (
                <div className="banner" style={{ margin: "16px 0 0", color: "var(--danger)" }}>
                  <IconWarn />
                  <div>{error}</div>
                </div>
              )}
            </div>
          </>
        )}
      </div>
    </>
  );
}
