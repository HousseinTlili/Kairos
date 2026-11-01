import { create } from "zustand";
import {
  skillsAudit,
  skillsCheck,
  skillsInspect,
  skillsInstall,
  skillsInstalled,
  skillsRemove,
  skillsScan,
  skillsSearch,
  skillsSetEnabled,
  skillsUpdate,
  type AuditedSkill,
  type CommandError,
  type FoundSkill,
  type InspectedSkill,
  type InstalledSkill,
  type ScanResult,
} from "../../lib/tauri";
import { canInstall, verdictName } from "./model";

interface SkillsStore {
  installed: InstalledSkill[] | null;
  /** Name → Hermes's status, once updates were checked. */
  updates: Record<string, string> | null;
  audited: AuditedSkill[] | null;
  results: FoundSkill[] | null;
  /** The open skill's page. */
  page: InspectedSkill | null;
  /** The scan of the open skill, never another's. */
  scan: ScanResult | null;
  /** What's running, in words; one thing at a time, as each is a Hermes command. */
  busy: string | null;
  error: string | null;
  notice: string | null;

  load: () => Promise<void>;
  toggle: (name: string, enabled: boolean) => Promise<void>;
  remove: (name: string) => Promise<void>;
  update: (name: string) => Promise<void>;
  check: () => Promise<void>;
  rescan: () => Promise<void>;
  search: (query: string, source: string) => Promise<void>;
  open: (identifier: string) => Promise<void>;
  close: () => void;
  scanOpen: () => Promise<void>;
  installOpen: () => Promise<void>;
}

const messageOf = (err: unknown) => (err as CommandError)?.message ?? String(err);

export const useSkills = create<SkillsStore>((set, get) => {
  /** Runs one Hermes command, saying what it's doing and what went wrong. */
  async function act(busy: string, work: () => Promise<void>): Promise<void> {
    if (get().busy) return;
    set({ busy, error: null, notice: null });
    try {
      await work();
    } catch (err) {
      set({ error: messageOf(err) });
    } finally {
      set({ busy: null });
    }
  }

  const reload = async () => set({ installed: await skillsInstalled() });

  return {
    installed: null,
    updates: null,
    audited: null,
    results: null,
    page: null,
    scan: null,
    busy: null,
    error: null,
    notice: null,

    load: () => act("Reading the skills…", reload),

    toggle: (name, enabled) =>
      act(enabled ? `Turning ${name} on…` : `Turning ${name} off…`, async () => {
        await skillsSetEnabled(name, enabled);
        await reload();
        set({
          notice: enabled
            ? `${name} is on for new conversations.`
            : `${name} is off for new conversations. One that's already open may still mention it.`,
        });
      }),

    remove: (name) =>
      act(`Removing ${name}…`, async () => {
        await skillsRemove(name);
        await reload();
        set({ notice: `${name} is removed.` });
      }),

    update: (name) =>
      act(`Updating ${name}…`, async () => {
        try {
          const outcome = await skillsUpdate(name);
          set({
            notice: outcome.installed
              ? `${name} is updated, through the same scan.`
              : `${name} was removed, and the new version wasn't installed: the scan refused it (${verdictName(outcome.verdict)}).`,
            updates: null,
          });
        } finally {
          await reload();
        }
      }),

    check: () =>
      act("Checking for updates…", async () => {
        const found = await skillsCheck();
        set({
          updates: Object.fromEntries(found.map((u) => [u.name, u.status])),
          notice:
            found.length === 0
              ? "No skills from the hub to check. Built-in ones update with Hermes."
              : found.some((u) => u.status === "update_available")
                ? null
                : "Every skill from the hub is up to date.",
        });
      }),

    rescan: () => act("Re-scanning…", async () => set({ audited: await skillsAudit() })),

    search: (query, source) =>
      act("Searching…", async () => {
        set({ results: null });
        set({ results: await skillsSearch(query, source) });
      }),

    open: (identifier) =>
      act("Reading the skill…", async () => {
        set({ page: null, scan: null });
        set({ page: await skillsInspect(identifier) });
      }),

    close: () => set({ page: null, scan: null, error: null, notice: null }),

    scanOpen: () =>
      act("Scanning. Hermes downloads it to a quarantine folder first…", async () => {
        const { page } = get();
        if (!page) return;
        set({ scan: await skillsScan(page.identifier) });
      }),

    installOpen: () =>
      act("Installing, through the scan again…", async () => {
        const { page, scan } = get();
        if (!page || !canInstall(scan)) return;
        const outcome = await skillsInstall(page.identifier);
        // Whatever happened, the old scan no longer speaks for what's on the PC.
        set({
          scan: null,
          notice: outcome.installed
            ? "Installed. New conversations know about it; one that's already open can still find it if it looks."
            : `Not installed: the scan refused it this time (${verdictName(outcome.verdict)}). The skill may have changed since you scanned it.`,
        });
        await reload();
      }),
  };
});
