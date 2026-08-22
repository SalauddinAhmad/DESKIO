/** Extensions: browser add-ons, plugins, panes, savers, widgets, and the
 *  installers left behind in Downloads.
 *
 *  Unlike a leftover, everything in here was installed on purpose, so nothing
 *  is ever ticked for the user — they choose, then review as usual.
 */
import { useState } from "react";
import type { ExtensionGroup } from "../types";
import { humanSize } from "../api";
import { ArtEmpty, IconPuzzle } from "./icons";

interface Props {
  groups: ExtensionGroup[];
  banner: React.ReactNode;
  loading: boolean;
  onRemove: (ids: string[]) => void;
}

export function ExtensionsView({ groups, banner, loading, onRemove }: Props) {
  const [openCategory, setOpenCategory] = useState<string | null>(null);
  const [picked, setPicked] = useState<Set<string>>(new Set());

  const group = groups.find((g) => g.category === openCategory) ?? null;

  const toggle = (id: string) => {
    const next = new Set(picked);
    next.has(id) ? next.delete(id) : next.add(id);
    setPicked(next);
  };

  const pickedInGroup = group ? group.items.filter((i) => picked.has(i.id)) : [];
  const total = pickedInGroup.reduce((n, i) => n + i.size_bytes, 0);

  return (
    <>
      <div className="list-col">
        <div className="list-head" data-tauri-drag-region>
          <h1 className="list-title">Extensions</h1>
        </div>
        {banner}
        <div className="count-line">
          {loading ? "Scanning…" : `${groups.length} categories`}
        </div>
        <div className="list-scroll">
          {groups.map((g) => (
            <button
              key={g.category}
              className="card"
              aria-selected={g.category === openCategory}
              onClick={() => setOpenCategory(g.category)}
            >
              <div
                className="card-icon"
                style={{ background: "none", color: "var(--bh-500)", width: 26, flex: "0 0 26px" }}
              >
                <IconPuzzle />
              </div>
              <div className="card-main">
                <div className="card-name">{g.label}</div>
                <div className="card-sub">
                  {g.items.length === 0
                    ? "No items"
                    : `${g.items.length} item${g.items.length === 1 ? "" : "s"}`}
                </div>
              </div>
              <div className="card-size">
                {g.items.length === 0 ? "—" : humanSize(g.size_bytes)}
              </div>
            </button>
          ))}
        </div>
      </div>

      <div className="detail">
        {!group ? (
          <div className="empty">
            <ArtEmpty />
            <h2>Extensions</h2>
            <p>
              Software that adds to your browsers, apps and system rather than running
              on its own. Pick a category to see what is installed.
            </p>
          </div>
        ) : (
          <>
            <div className="detail-top" data-tauri-drag-region />
            <div className="detail-body">
              <div className="hero">
                <div className="hero-main">
                  <div className="hero-name-row">
                    <div className="hero-name">{group.label}</div>
                    <div className="hero-size">
                      {group.items.length === 0 ? "—" : humanSize(group.size_bytes)}
                    </div>
                  </div>
                  <dl className="hero-rows">
                    <div className="hero-row">
                      <dt>{group.description}</dt>
                      <dd>{group.items.length} item(s)</dd>
                    </div>
                  </dl>
                </div>
              </div>

              {group.items.length === 0 ? (
                <p style={{ color: "var(--muted)", padding: "22px 2px" }}>
                  Nothing found in this category.
                </p>
              ) : (
                <>
                  <h2 className="section-title">Installed</h2>
                  <div style={{ display: "grid", gap: 2 }}>
                    {group.items.map((i) => (
                      <label className="item" key={i.id}>
                        <input
                          type="checkbox"
                          checked={picked.has(i.id)}
                          onChange={() => toggle(i.id)}
                        />
                        <div className="item-main">
                          <div className="item-name">{i.name}</div>
                          <div className="item-path">{i.path}</div>
                          {i.detail && (
                            <div className="item-why">
                              {i.requires_admin && <span className="pill pill-admin">admin</span>}
                              {i.detail}
                            </div>
                          )}
                        </div>
                        <div className="item-size">
                          {humanSize(i.size_bytes, i.size_unknown)}
                        </div>
                      </label>
                    ))}
                  </div>
                </>
              )}
            </div>
          </>
        )}
        <div className="detail-foot">
          <div style={{ color: "var(--muted)", fontSize: 13 }}>
            {pickedInGroup.length > 0
              ? `${pickedInGroup.length} selected · ${humanSize(total)}`
              : "Nothing here is selected for you — these were all installed deliberately."}
          </div>
          <button
            className="btn btn-primary"
            disabled={pickedInGroup.length === 0}
            onClick={() => onRemove(pickedInGroup.map((i) => i.id))}
          >
            Remove
          </button>
        </div>
      </div>
    </>
  );
}
