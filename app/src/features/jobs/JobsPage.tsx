import { useEffect, useState } from "react";
import type { Job } from "../../lib/tauri";
import {
  deleteQuestion,
  formProblem,
  lastRunLabel,
  nextRunLabel,
  SCHEDULE_EXAMPLES,
  statusOf,
  toggleOf,
} from "./model";
import { useJobs } from "./store";

const REFRESH_MS = 30_000;

export function JobsPage() {
  const { jobs, loaded, error, refresh } = useJobs();

  useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => void refresh(), REFRESH_MS);
    return () => window.clearInterval(timer);
  }, [refresh]);

  return (
    <section className="notifications jobs" aria-labelledby="jobs-title">
      <div className="system-head">
        <h1 id="jobs-title">Scheduled</h1>
        <button onClick={() => void refresh()}>Refresh</button>
      </div>
      <p className="muted small">
        Jobs Hermes runs by itself on a schedule, each in a fresh conversation. Your rules apply to
        everything a job does; a step that would ask you is not run, since nobody is there to
        answer.
      </p>

      {error && (
        <div className="status-card bad" role="alert">
          <p>{error}</p>
        </div>
      )}

      <NewJobForm />

      {loaded && jobs.length === 0 && !error && <p className="muted">No scheduled jobs.</p>}
      <ul className="notif-list">
        {jobs.map((job) => (
          <JobRow key={job.id} job={job} />
        ))}
      </ul>
    </section>
  );
}

function JobRow({ job }: { job: Job }) {
  const { busyId, act, remove } = useJobs();
  const [confirmDelete, setConfirmDelete] = useState(false);
  const status = statusOf(job);
  const toggle = toggleOf(job);
  const busy = busyId === job.id;

  return (
    <li className="job-item">
      <div className="notif-meta">
        <span className="notif-title">{job.name || "Untitled job"}</span>
        <span className="muted">{job.schedule_display}</span>
      </div>
      {job.prompt && <p className="notif-body">{job.prompt}</p>}
      <div className="job-facts">
        <span className={`job-status ${status.tone}`}>{status.text}</span>
        {job.enabled && job.next_run_at && (
          <span className="muted">Next: {nextRunLabel(job.next_run_at)}</span>
        )}
        {job.last_run_at && <span className="muted">Last: {lastRunLabel(job.last_run_at)}</span>}
      </div>
      <div className="notif-toolbar">
        <button onClick={() => void act(job.id, "run")} disabled={busy}>
          Run now
        </button>
        {toggle && (
          <button onClick={() => void act(job.id, toggle)} disabled={busy}>
            {toggle === "pause" ? "Pause" : "Resume"}
          </button>
        )}
        <span className="spacer" />
        {confirmDelete ? (
          <>
            <span className="muted small">{deleteQuestion(job)}</span>
            <button
              className="deny"
              disabled={busy}
              onClick={() => {
                setConfirmDelete(false);
                void remove(job.id);
              }}
            >
              Delete
            </button>
            <button onClick={() => setConfirmDelete(false)}>Keep</button>
          </>
        ) : (
          <button onClick={() => setConfirmDelete(true)} disabled={busy}>
            Delete…
          </button>
        )}
      </div>
    </li>
  );
}

function NewJobForm() {
  const create = useJobs((s) => s.create);
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [schedule, setSchedule] = useState("");
  const [prompt, setPrompt] = useState("");
  const [sending, setSending] = useState(false);
  const problem = formProblem(name, schedule, prompt);

  if (!open) {
    return (
      <div>
        <button className="primary" onClick={() => setOpen(true)}>
          New job
        </button>
      </div>
    );
  }

  return (
    <form
      className="job-form settings-section"
      onSubmit={(e) => {
        e.preventDefault();
        if (problem || sending) return;
        setSending(true);
        void create(name, schedule, prompt).then((ok) => {
          setSending(false);
          if (ok) {
            setName("");
            setSchedule("");
            setPrompt("");
            setOpen(false);
          }
        });
      }}
    >
      <h2>New job</h2>
      <label>
        <span>Name</span>
        <input value={name} onChange={(e) => setName(e.target.value)} maxLength={200} />
      </label>
      <label>
        <span>When</span>
        <input
          value={schedule}
          onChange={(e) => setSchedule(e.target.value)}
          placeholder="every monday 9am"
          spellCheck={false}
        />
      </label>
      <p className="muted small">
        In Hermes&apos;s words or cron, for example: {SCHEDULE_EXAMPLES.join(" · ")}
      </p>
      <label>
        <span>What to do</span>
        <textarea
          className="projects-input"
          rows={4}
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
          placeholder="Self-contained instructions: the job doesn't see your chats."
        />
      </label>
      <div className="notif-toolbar">
        <button type="submit" className="primary" disabled={problem !== null || sending}>
          Schedule
        </button>
        <button type="button" onClick={() => setOpen(false)}>
          Cancel
        </button>
        {problem && (name || schedule || prompt) && <span className="muted small">{problem}</span>}
      </div>
    </form>
  );
}
