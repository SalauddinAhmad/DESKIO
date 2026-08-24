/** Cleanup: caches, logs, crash reports and build output.
 *
 *  Everything here goes to the Trash like every other removal, which means the
 *  disk does not actually get emptier until the Trash is emptied. The footer
 *  says so, because reporting freed space that has not been freed would be the
 *  easiest lie in the app to tell by accident.
 */
import { useState } from "react";
import type { JunkGroup } from "../types";
import { humanSize } from "../api";
import { ArtEmpty, IconBroom, IconWarn } from "./icons";

interface Props {
  groups: JunkGroup[];
  banner: React.ReactNode;
  loading: boolean;
  onClean: (ids: string[]) => void;
}

export function CleanupView({ groups, banner, loading, onClean }: Props) {
  const [openCategory, setOpenCategory] = useState<string | null>(null);
  const [picked, setPicked] = useState<Set<string>>(new Set());

  const group = groups.find((g) => g.category === openCategory) ?? null;

  const toggle = (id: string) => {
    const next = new Set(picked);
    next.has(id) ? next.delete(id) : next.add(id);
    setPicked(next);
  };

  const pickedInGroup = group ? group.items.filter((i) => picked.has(i.id)) : [];
  const allPicked =
    !!group && group.items.length > 0 && pickedInGroup.length === group.items.length;
  const selectedBytes = pickedInGroup.reduce((n, i) => n + i.size_bytes, 0);

  const reclaimable = groups
    .filter((g) => g.removable)
    .reduce((n, g) => n + g.size_bytes, 0);

  return (
    <>
      <div className="list-col">
        <div className="list-head" data-tauri-drag-region>
          <h1 className="list-title">Cleanup</h1>
        </div>
        {banner}
        <div className="count-line">
          {loading ? "Scanning…" : `${humanSize(reclaimable)} reclaimable`}
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
                <IconBroom />
              </div>
              <div className="card-main">
                <div className="card-name">{g.label}</div>
                <div className="card-sub">
                  {g.items.length === 0
                    ? "Nothing found"
                    : `${g.items.length} item${g.items.length === 1 ? "" : "s"}`}
                  {!g.removable && g.items.length > 0 ? " · reported only" : ""}
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
            <h2>Cleanup</h2>
            <p>
              Files your apps rebuild on their own — caches, logs, crash reports and
              build output. Pick a category to see exactly what is there.
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
                  <div
                    style={{
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "space-between",
                      margin: "24px 0 10px",
                    }}
                  >
                    <h2 className="section-title" style={{ margin: 0 }}>
                      Contents
                    </h2>
                    {group.removable && (
                      <label className="toggle" style={{ fontSize: 12.5 }}>
                        <input
                          type="checkbox"
                          checked={allPicked}
                          onChange={() => {
                            const next = new Set(picked);
                            group.items.forEach((i) =>
                              allPicked ? next.delete(i.id) : next.add(i.id)
                            );
                            setPicked(next);
                          }}
                        />
                        Select everything in this category
                      </label>
                    )}
                  </div>

                  <div style={{ display: "grid", gap: 2 }}>
                    {group.items.map((i) => (
                      <label className="item" key={i.id}>
                        {group.removable && (
                          <input
                            type="checkbox"
                            checked={picked.has(i.id)}
                            onChange={() => toggle(i.id)}
                          />
                        )}
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

            <div className="detail-foot">
              {group.removable ? (
                <>
                  <div style={{ color: "var(--muted)", fontSize: 13, maxWidth: "62%" }}>
                    {pickedInGroup.length > 0
                      ? `${pickedInGroup.length} selected · ${humanSize(selectedBytes)}. ` +
                        "This goes to the Trash — your disk gets the space back when you empty it."
                      : "Everything here is rebuilt automatically when an app needs it again."}
                  </div>
                  <button
                    className="btn btn-primary"
                    disabled={pickedInGroup.length === 0}
                    onClick={() => onClean(pickedInGroup.map((i) => i.id))}
                  >
                    Clean up
                  </button>
                </>
              ) : (
                <div
                  className="banner"
                  style={{ margin: 0, flex: 1, background: "transparent", border: 0 }}
                >
                  <IconWarn />
                  <div>
                    DESKIO will not empty your Trash. It is the one action with no
                    undo, so it stays with you and Finder.
                  </div>
                </div>
              )}
            </div>
          </>
        )}
      </div>
    </>
  );
}
