import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  AlertTriangle,
  CheckCircle2,
  CirclePlus,
  KeyRound,
  Loader2,
  Pencil,
  RefreshCcw,
  ShieldCheck,
  Trash2,
} from "lucide-react";

type ProfileInfo = {
  name: string;
  email: string | null;
  plan: string | null;
  accountIdHash: string | null;
  isActive: boolean;
  isCurrentAuth: boolean;
  idTokenExpired: boolean;
  accessTokenExpired: boolean;
};

type RequirementInfo = {
  authPath: string;
  profilesPath: string;
  configPath: string;
  platform: string;
};

type Notice = {
  type: "success" | "error";
  message: string;
} | null;

type BusyAction = "load" | "add" | "switch" | "rename" | "remove" | null;

const commandLabels: Record<Exclude<BusyAction, null>, string> = {
  load: "Refreshing",
  add: "Adding",
  switch: "Switching",
  rename: "Renaming",
  remove: "Removing",
};

function describeError(error: unknown) {
  if (typeof error === "string") {
    return error;
  }

  if (error instanceof Error) {
    return error.message;
  }

  try {
    return JSON.stringify(error);
  } catch {
    return "Unexpected frontend error.";
  }
}

function chooseSelectedName(profiles: ProfileInfo[], preferredName?: string) {
  return (
    profiles.find((profile) => profile.name === preferredName)?.name ??
    profiles.find((profile) => profile.isCurrentAuth)?.name ??
    profiles.find((profile) => profile.isActive)?.name ??
    profiles[0]?.name ??
    ""
  );
}

function profileLabel(profile: ProfileInfo) {
  return profile.email || profile.accountIdHash || "Metadata missing";
}

function tokenSummary(profile: ProfileInfo) {
  const expiredTokens = [
    profile.idTokenExpired ? "ID" : null,
    profile.accessTokenExpired ? "Access" : null,
  ].filter(Boolean);

  if (expiredTokens.length > 0) {
    return `${expiredTokens.join(" + ")} expired`;
  }

  return "Tokens valid";
}

function App() {
  const [profiles, setProfiles] = useState<ProfileInfo[]>([]);
  const [requirements, setRequirements] = useState<RequirementInfo | null>(null);
  const [selectedName, setSelectedName] = useState("");
  const [addName, setAddName] = useState("");
  const [renameName, setRenameName] = useState("");
  const [notice, setNotice] = useState<Notice>(null);
  const [loading, setLoading] = useState(true);
  const [busyAction, setBusyAction] = useState<BusyAction>(null);

  const selectedProfile = useMemo(
    () => profiles.find((profile) => profile.name === selectedName) ?? null,
    [profiles, selectedName],
  );

  const statusCounts = useMemo(
    () => ({
      active: profiles.filter((profile) => profile.isActive).length,
      current: profiles.filter((profile) => profile.isCurrentAuth).length,
      expired: profiles.filter(
        (profile) => profile.idTokenExpired || profile.accessTokenExpired,
      ).length,
    }),
    [profiles],
  );

  const applyProfiles = useCallback((nextProfiles: ProfileInfo[], preferredName?: string) => {
    setProfiles(nextProfiles);
    setSelectedName((currentName) =>
      chooseSelectedName(nextProfiles, preferredName || currentName),
    );
  }, []);

  const loadProfiles = useCallback(async () => {
    setBusyAction("load");
    setNotice(null);

    try {
      const [nextProfiles, nextRequirements] = await Promise.all([
        invoke<ProfileInfo[]>("list_profiles"),
        invoke<RequirementInfo>("get_requirements"),
      ]);

      applyProfiles(nextProfiles);
      setRequirements(nextRequirements);
    } catch (error) {
      setNotice({ type: "error", message: describeError(error) });
    } finally {
      setLoading(false);
      setBusyAction(null);
    }
  }, [applyProfiles]);

  useEffect(() => {
    void loadProfiles();
  }, [loadProfiles]);

  useEffect(() => {
    setRenameName(selectedProfile?.name ?? "");
  }, [selectedProfile?.name]);

  async function runProfileCommand(
    action: Exclude<BusyAction, "load" | null>,
    command: "add_profile" | "switch_profile" | "rename_profile" | "remove_profile",
    args: Record<string, string>,
    successMessage: string,
    preferredName?: string,
  ) {
    setBusyAction(action);
    setNotice(null);

    try {
      const nextProfiles = await invoke<ProfileInfo[]>(command, args);
      applyProfiles(nextProfiles, preferredName);
      setNotice({ type: "success", message: successMessage });
      return true;
    } catch (error) {
      setNotice({ type: "error", message: describeError(error) });
      return false;
    } finally {
      setBusyAction(null);
    }
  }

  function handleAddProfile(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const name = addName.trim();

    if (!name) {
      setNotice({ type: "error", message: "Enter a profile name before adding." });
      return;
    }

    void runProfileCommand(
      "add",
      "add_profile",
      { name },
      `Saved current auth as "${name}".`,
      name,
    ).then((didAddProfile) => {
      if (didAddProfile) {
        setAddName("");
      }
    });
  }

  function handleSwitchProfile() {
    if (!selectedProfile) {
      return;
    }

    void runProfileCommand(
      "switch",
      "switch_profile",
      { name: selectedProfile.name },
      `Switched auth to "${selectedProfile.name}".`,
      selectedProfile.name,
    );
  }

  function handleRenameProfile(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    if (!selectedProfile) {
      return;
    }

    const newName = renameName.trim();

    if (!newName) {
      setNotice({ type: "error", message: "Enter a new profile name before renaming." });
      return;
    }

    void runProfileCommand(
      "rename",
      "rename_profile",
      { oldName: selectedProfile.name, newName },
      `Renamed "${selectedProfile.name}" to "${newName}".`,
      newName,
    );
  }

  function handleRemoveProfile() {
    if (!selectedProfile) {
      return;
    }

    const shouldRemove = window.confirm(
      `Remove profile "${selectedProfile.name}"? This cannot be undone.`,
    );

    if (!shouldRemove) {
      return;
    }

    void runProfileCommand(
      "remove",
      "remove_profile",
      { name: selectedProfile.name },
      `Removed "${selectedProfile.name}".`,
    );
  }

  const isBusy = busyAction !== null;
  const actionLabel = busyAction ? commandLabels[busyAction] : null;

  return (
    <main className="app-shell">
      <section className="console-heading" aria-labelledby="app-title">
        <div>
          <p className="eyebrow">Codex Switcher</p>
          <h1 id="app-title">Account Console</h1>
        </div>

        <div className="heading-actions" aria-label="Profile summary">
          <span>{profiles.length} profiles</span>
          <span>{statusCounts.current} current</span>
          <span>{statusCounts.expired} expired</span>
          <button
            className="icon-button"
            type="button"
            onClick={loadProfiles}
            disabled={isBusy}
            aria-label="Refresh profiles"
            title="Refresh profiles"
          >
            <RefreshCcw size={16} aria-hidden="true" />
          </button>
        </div>
      </section>

      {notice ? (
        <div className={`notice ${notice.type}`} role={notice.type === "error" ? "alert" : "status"}>
          {notice.type === "success" ? (
            <CheckCircle2 size={17} aria-hidden="true" />
          ) : (
            <AlertTriangle size={17} aria-hidden="true" />
          )}
          <span>{notice.message}</span>
        </div>
      ) : null}

      <section className="console-grid" aria-label="Profile manager">
        <div className="table-region">
          <div className="table-toolbar">
            <div>
              <h2>Profiles</h2>
              <p>Redacted account metadata from local profile storage.</p>
            </div>
            {actionLabel ? (
              <span className="busy-pill">
                <Loader2 size={14} aria-hidden="true" />
                {actionLabel}
              </span>
            ) : null}
          </div>

          <div className="account-table" role="table" aria-label="Saved Codex profiles">
            <div className="table-header" role="row">
              <span role="columnheader">Profile</span>
              <span role="columnheader">Plan</span>
              <span role="columnheader">Auth</span>
              <span role="columnheader">Tokens</span>
            </div>

            {loading ? (
              <div className="skeleton-stack" aria-label="Loading profiles">
                {Array.from({ length: 5 }).map((_, index) => (
                  <div className="skeleton-row" key={index}>
                    <span />
                    <span />
                    <span />
                    <span />
                  </div>
                ))}
              </div>
            ) : profiles.length === 0 ? (
              <div className="empty-state">
                <KeyRound size={22} aria-hidden="true" />
                <h3>No saved profiles</h3>
                <p>Add the currently authenticated Codex account to begin switching.</p>
              </div>
            ) : (
              <div role="rowgroup">
                {profiles.map((profile) => {
                  const isSelected = profile.name === selectedName;
                  const hasExpiredToken = profile.idTokenExpired || profile.accessTokenExpired;

                  return (
                    <button
                      className={`table-row ${isSelected ? "selected" : ""}`}
                      type="button"
                      role="row"
                      key={profile.name}
                      onClick={() => setSelectedName(profile.name)}
                      aria-selected={isSelected}
                    >
                      <span className="profile-cell" role="cell">
                        <strong>{profile.name}</strong>
                        <small>{profileLabel(profile)}</small>
                      </span>
                      <span role="cell">{profile.plan || "Unknown"}</span>
                      <span className="chip-cluster" role="cell">
                        {profile.isCurrentAuth ? <span className="chip cyan">Current</span> : null}
                        {profile.isActive ? <span className="chip green">Active</span> : null}
                        {!profile.isCurrentAuth && !profile.isActive ? (
                          <span className="chip muted">Saved</span>
                        ) : null}
                      </span>
                      <span className="chip-cluster" role="cell">
                        {!profile.accountIdHash ? <span className="chip amber">Missing</span> : null}
                        <span className={`chip ${hasExpiredToken ? "red" : "green"}`}>
                          {hasExpiredToken ? "Expired" : "Valid"}
                        </span>
                      </span>
                    </button>
                  );
                })}
              </div>
            )}
          </div>
        </div>

        <aside className="detail-panel" aria-label="Profile actions and details">
          <form className="action-block add-block" onSubmit={handleAddProfile}>
            <label htmlFor="add-profile">Add current auth</label>
            <div className="input-row">
              <input
                id="add-profile"
                value={addName}
                onChange={(event) => setAddName(event.target.value)}
                placeholder="profile-name"
                disabled={isBusy}
              />
              <button className="primary-button" type="submit" disabled={isBusy}>
                <CirclePlus size={16} aria-hidden="true" />
                Add
              </button>
            </div>
          </form>

          <div className="detail-section">
            <div className="section-title">
              <h2>Selected Profile</h2>
              {selectedProfile ? (
                <span className="hash-label">{selectedProfile.accountIdHash || "No hash"}</span>
              ) : null}
            </div>

            {loading ? (
              <div className="detail-loading">
                <span />
                <span />
                <span />
              </div>
            ) : selectedProfile ? (
              <>
                <dl className="detail-list">
                  <div>
                    <dt>Name</dt>
                    <dd>{selectedProfile.name}</dd>
                  </div>
                  <div>
                    <dt>Email</dt>
                    <dd>{selectedProfile.email || "Missing"}</dd>
                  </div>
                  <div>
                    <dt>Plan</dt>
                    <dd>{selectedProfile.plan || "Unknown"}</dd>
                  </div>
                  <div>
                    <dt>Tokens</dt>
                    <dd>{tokenSummary(selectedProfile)}</dd>
                  </div>
                </dl>

                <div className="status-strip" aria-label="Selected profile status">
                  {selectedProfile.isCurrentAuth ? <span className="chip cyan">Current</span> : null}
                  {selectedProfile.isActive ? <span className="chip green">Active</span> : null}
                  {!selectedProfile.accountIdHash ? <span className="chip amber">Metadata missing</span> : null}
                  {selectedProfile.idTokenExpired ? <span className="chip red">ID expired</span> : null}
                  {selectedProfile.accessTokenExpired ? (
                    <span className="chip red">Access expired</span>
                  ) : null}
                </div>

                <div className="button-stack">
                  <button
                    className="primary-button wide"
                    type="button"
                    onClick={handleSwitchProfile}
                    disabled={isBusy || selectedProfile.isCurrentAuth}
                  >
                    <ShieldCheck size={16} aria-hidden="true" />
                    Switch to profile
                  </button>

                  <form className="rename-form" onSubmit={handleRenameProfile}>
                    <label htmlFor="rename-profile">Rename profile</label>
                    <div className="input-row">
                      <input
                        id="rename-profile"
                        value={renameName}
                        onChange={(event) => setRenameName(event.target.value)}
                        disabled={isBusy}
                      />
                      <button className="secondary-button" type="submit" disabled={isBusy}>
                        <Pencil size={15} aria-hidden="true" />
                        Save
                      </button>
                    </div>
                  </form>

                  <button
                    className="danger-button wide"
                    type="button"
                    onClick={handleRemoveProfile}
                    disabled={isBusy}
                  >
                    <Trash2 size={16} aria-hidden="true" />
                    Remove profile
                  </button>
                </div>
              </>
            ) : (
              <div className="detail-empty">
                <p>No profile selected.</p>
              </div>
            )}
          </div>

          <div className="detail-section requirements">
            <div className="section-title">
              <h2>Local Requirements</h2>
              <span>{requirements?.platform || "Unknown platform"}</span>
            </div>
            <dl className="path-list">
              <div>
                <dt>Auth</dt>
                <dd title={requirements?.authPath}>{requirements?.authPath || "Unavailable"}</dd>
              </div>
              <div>
                <dt>Profiles</dt>
                <dd title={requirements?.profilesPath}>{requirements?.profilesPath || "Unavailable"}</dd>
              </div>
              <div>
                <dt>Config</dt>
                <dd title={requirements?.configPath}>{requirements?.configPath || "Unavailable"}</dd>
              </div>
            </dl>
          </div>
        </aside>
      </section>
    </main>
  );
}

export default App;
