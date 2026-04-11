import { useAppStore } from "../../stores/appStore";
import { ProjectList } from "../project/ProjectList";
import { ProjectGroupView } from "../project/ProjectGroupView";
import { SessionList } from "../session/SessionList";
import { ConversationView } from "../session/ConversationView";
import { BackupManager } from "../backup/BackupManager";
import { SearchResults } from "../search/SearchResults";
import { SettingsPage } from "../settings/SettingsPage";

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
    default:
      return <ProjectList />;
  }
}
