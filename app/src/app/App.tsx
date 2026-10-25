import { useEffect } from "react";
import { BriefingCard } from "../features/briefing/BriefingCard";
import { useBriefing } from "../features/briefing/store";
import { ChatView } from "../features/chat/ChatView";
import { startChatEvents, useChat } from "../features/chat/store";
import { ConnectScreen } from "../features/connection/ConnectScreen";
import { useConnection } from "../features/connection/store";
import { JobsPage } from "../features/jobs/JobsPage";
import { NotificationsPanel } from "../features/notifications/NotificationsPanel";
import { useNotifications } from "../features/notifications/store";
import { Settings } from "../features/shell/Settings";
import { Sidebar } from "../features/shell/Sidebar";
import { AlertBanners } from "../features/system/AlertBanners";
import { SystemPage } from "../features/system/SystemPage";
import { useSystem } from "../features/system/store";
import { useShell } from "../features/shell/store";
import { useStartup } from "../features/startup/store";
import { onNavigate, onOpenConversation } from "../lib/tauri";

export default function App() {
  const connected = useConnection((state) => state.info?.status === "connected");
  const view = useShell((state) => state.view);
  const resume = useChat((state) => state.resume);
  const startSystem = useSystem((state) => state.start);
  const startStartup = useStartup((state) => state.start);
  const startNotifications = useNotifications((state) => state.start);
  const startBriefing = useBriefing((state) => state.start);
  const show = useShell((state) => state.show);

  useEffect(() => {
    // Listen before anything can be sent or resumed, so no run event is missed.
    const stop = startChatEvents();
    return () => {
      void stop.then((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    // System readings don't need Hermes (D-012): follow them from the start.
    const stop = startSystem();
    return () => {
      void stop.then((unlisten) => unlisten());
    };
  }, [startSystem]);

  useEffect(() => {
    const stops = [
      startStartup(),
      startNotifications(),
      startBriefing(),
      // The tray menu's "System" item.
      onNavigate((target) => {
        if (target === "system") show("system");
      }),
      // "Open in Kairos" from the quick box (M8).
      onOpenConversation((sessionId) => {
        show("chat");
        void useChat.getState().openConversation(sessionId);
      }),
    ];
    return () => {
      for (const stop of stops) void stop.then((unlisten) => unlisten());
    };
  }, [startStartup, startNotifications, startBriefing, show]);

  useEffect(() => {
    // M1 §9 rule 6: once connected, settle or re-follow requests from before a restart.
    if (connected) void resume();
  }, [connected, resume]);

  return (
    <div className="shell">
      <Sidebar />
      <main className="main">
        {view !== "settings" && <AlertBanners />}
        {view === "chat" && <BriefingCard />}
        {view === "settings" ? (
          <Settings />
        ) : view === "system" ? (
          <SystemPage />
        ) : view === "notifications" ? (
          <NotificationsPanel />
        ) : view === "jobs" ? (
          <JobsPage />
        ) : connected ? (
          <ChatView />
        ) : (
          <div className="connect-wrap">
            <ConnectScreen />
          </div>
        )}
      </main>
    </div>
  );
}
