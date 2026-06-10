import { useAppStore } from "../../stores/appStore";
import { ProjectList } from "../project/ProjectList";
import { ProjectGroupView } from "../project/ProjectGroupView";
import { SessionList } from "../session/SessionList";
import { ConversationView } from "../session/ConversationView";
import { BackupManager } from "../backup/BackupManager";
import { SearchResults } from "../search/SearchResults";
import { SettingsPage } from "../settings/SettingsPage";
import { LiveDashboard } from "../live/LiveDashboard";
import { LiveConversationView } from "../live/LiveConversationView";
import { UsagePage } from "../usage/UsagePage";
import { DayPlannerView } from "../dayplanner/DayPlannerView";
import { CodexProjectList } from "../codex/CodexProjectList";
import { CodexSessionList } from "../codex/CodexSessionList";
import { CodexConversationView } from "../codex/CodexConversationView";

export function MainContent() {
  const { view } = useAppStore();

  switch (view) {
    case "projects":
      return <ProjectList />;
    case "projectGroup":
      return <ProjectGroupView />;
    case "sessions":
      return <SessionList key="sessions" />;
    case "conversation":
      return <ConversationView />;
    case "favorites":
      return <SessionList key="favorites" favoritesOnly />;
    case "backups":
      return <BackupManager />;
    case "search":
      return <SearchResults />;
    case "settings":
      return <SettingsPage />;
    case "live":
      return <LiveDashboard />;
    case "liveConversation":
      return <LiveConversationView />;
    case "usage":
      return <UsagePage />;
    case "dayPlanner":
      return <DayPlannerView />;
    case "codexProjects":
      return <CodexProjectList />;
    case "codexSessions":
      return <CodexSessionList />;
    case "codexConversation":
      return <CodexConversationView />;
    default:
      return <ProjectList />;
  }
}
