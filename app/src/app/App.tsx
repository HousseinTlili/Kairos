import { ConnectScreen } from "../features/connection/ConnectScreen";

// The Chat route arrives in Step 5; until then the app is the Connect screen.
export default function App() {
  return (
    <main className="app">
      <header className="app-header">
        <h1>Kairos</h1>
      </header>
      <ConnectScreen />
    </main>
  );
}
