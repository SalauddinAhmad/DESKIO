import React, { useState } from "react";
import { JunkGroup } from "../types";
import { humanSize } from "../api";
import { IconTrash, ArtEmpty } from "./icons";

interface DevCleanViewProps {
  junk: JunkGroup[];
  onCleanDevJunk: (selectedIds: string[]) => void;
}

export const DevCleanView: React.FC<DevCleanViewProps> = ({ junk, onCleanDevJunk }) => {
  const [selectedIds, setSelectedIds] = useState<string[]>([]);

  // Find junk groups that pertain to developer caches or system build logs
  const devGroups = junk.filter(
    (g) => g.category === "developer_junk" || g.category === "app_caches" || g.category === "logs"
  );

  const allItems = devGroups.flatMap((g) => g.items);
  const totalDevSize = allItems.reduce((acc, i) => acc + (i.size_bytes || 0), 0);

  const toggleSelect = (id: string) => {
    setSelectedIds((prev) =>
      prev.includes(id) ? prev.filter((item) => item !== id) : [...prev, id]
    );
  };

  const toggleSelectAll = () => {
    if (selectedIds.length === allItems.length) {
      setSelectedIds([]);
    } else {
      setSelectedIds(allItems.map((i) => i.id));
    }
  };

  const handleClean = () => {
    if (selectedIds.length > 0) {
      onCleanDevJunk(selectedIds);
    }
  };

  return (
    <div className="devclean-container">
      <div className="view-header">
        <div>
          <h2>Developer & Build Junk Cleaner</h2>
          <p className="view-sub">
            Sweep heavy build targets, node_modules, Xcode caches, and package manager junk.
          </p>
        </div>
        <div className="header-actions">
          {allItems.length > 0 && (
            <>
              <button className="btn-secondary" onClick={toggleSelectAll}>
                {selectedIds.length === allItems.length ? "Deselect All" : "Select All"}
              </button>
              <button
                className="btn-primary danger"
                disabled={selectedIds.length === 0}
                onClick={handleClean}
              >
                <IconTrash /> Clean Selected ({selectedIds.length})
              </button>
            </>
          )}
        </div>
      </div>

      <div className="devclean-summary-bar">
        <div className="summary-item">
          <span className="summary-label">Total Dev Junk</span>
          <span className="summary-val">{humanSize(totalDevSize)}</span>
        </div>
        <div className="summary-item">
          <span className="summary-label">Selected Items</span>
          <span className="summary-val">
            {selectedIds.length} / {allItems.length}
          </span>
        </div>
      </div>

      {allItems.length === 0 ? (
        <div className="empty-state">
          <ArtEmpty />
          <h3>No Developer Junk Found</h3>
          <p>Your dev environment build caches and targets are currently clean!</p>
        </div>
      ) : (
        <div className="dev-items-list">
          {devGroups.map((group) => (
            <div key={group.category} className="group-card">
              <div className="group-header">
                <span className="group-title">{group.label}</span>
                <span className="group-size">{humanSize(group.size_bytes)}</span>
              </div>
              <p className="group-desc">{group.description}</p>
              <div className="group-items">
                {group.items.map((item) => {
                  const isChecked = selectedIds.includes(item.id);
                  return (
                    <div
                      key={item.id}
                      className={`dev-item-row ${isChecked ? "selected" : ""}`}
                      onClick={() => toggleSelect(item.id)}
                    >
                      <input
                        type="checkbox"
                        checked={isChecked}
                        onChange={() => {}}
                        onClick={(e) => e.stopPropagation()}
                      />
                      <div className="item-info">
                        <span className="item-name">{item.name}</span>
                        <span className="item-path">{item.path}</span>
                      </div>
                      <span className="item-size">{humanSize(item.size_bytes)}</span>
                    </div>
                  );
                })}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};
