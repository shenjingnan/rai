import { QueryClientProvider } from "@tanstack/react-query";
import { Navigate, Route, Routes } from "react-router-dom";
import { AppShell } from "@/components/layout/AppShell";
import { ToastProvider } from "@/components/ui/toast";
import { queryClient } from "@/lib/queryClient";
import { ChatPage } from "@/pages/ChatPage";
import { CompanionPage } from "@/pages/CompanionPage";
import { HomePage } from "@/pages/HomePage";
import { ModelsOverviewPage } from "@/pages/ModelsOverviewPage";
import { AsrPage } from "@/pages/models/AsrPage";
import { KwsPage } from "@/pages/models/KwsPage";
import { LibraryPage } from "@/pages/models/LibraryPage";
import { LlmPage } from "@/pages/models/LlmPage";
import { TtsPage } from "@/pages/models/TtsPage";
import { SettingsPage } from "@/pages/SettingsPage";
import { AppRuntimeProvider } from "@/providers/AppRuntimeProvider";

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <AppRuntimeProvider>
        <ToastProvider>
          <Routes>
            <Route element={<AppShell />}>
              <Route index element={<Navigate to="/home" replace />} />
              <Route path="home" element={<HomePage />} />
              <Route path="chat" element={<ChatPage />} />
              <Route path="companion" element={<CompanionPage />} />
              <Route path="models" element={<ModelsOverviewPage />} />
              <Route path="models/kws" element={<KwsPage />} />
              <Route path="models/asr" element={<AsrPage />} />
              <Route path="models/llm" element={<LlmPage />} />
              <Route path="models/tts" element={<TtsPage />} />
              <Route path="models/library" element={<LibraryPage />} />
              <Route path="settings" element={<SettingsPage />} />
            </Route>
          </Routes>
        </ToastProvider>
      </AppRuntimeProvider>
    </QueryClientProvider>
  );
}
