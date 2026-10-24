import { useEffect } from "react";
import { ChatView } from "../features/chat/ChatView";
import { startChatEvents } from "../features/chat/store";
import { ConnectScreen } from "../features/connection/ConnectScreen";
import { useConnection } from "../features/connection/store";

export default function App() {
  const connected = useConnection((state) => state.info?.status === "connected");
  const version = useConnection((state) => state.info?.version);
  const check = useConnection((state) => state.check);

  useEffect(() => {
    // Listen before anything can be sent, so no run event is missed.
    const stop = startChatEvents();
    return () => {
      void stop.then((unlisten) => unlisten());
    };
  }, []);

  return (
    <main className={connected ? "app app-chat" : "app"}>
      <header className="app-header">
        <h1>Kairos</h1>
        {connected && (
          <button className="link" onClick={() => void check()} title="Check the connection again">
            <span className="dot ok-dot" aria-hidden="true" /> Hermes {version}
          </button>
        )}
      </header>
      {connected ? <ChatView /> : <ConnectScreen />}
    </main>
  );
}
