import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Link } from "react-router-dom";
import apiClient from "../../core/api/client";
import type { Book, Playlist, Progress } from "../../core/types";
import { useAuthStore } from "../../core/stores/authStore";
import { usePlayerStore } from "../../core/stores/playerStore";
import {
  ChevronRight,
  BarChart3,
  BellRing,
  Heart,
  History,
  Info,
  Key,
  Save,
  Settings,
  User,
} from "lucide-react";

const MyPage: React.FC = () => {
  const { t } = useTranslation();
  const user = useAuthStore((state) => state.user);
  const setUser = useAuthStore((state) => state.setUser);
  const currentChapter = usePlayerStore((state) => state.currentChapter);
  const [recentPlays, setRecentPlays] = useState<Progress[]>([]);
  const [favorites, setFavorites] = useState<Book[]>([]);
  const [playlistCount, setPlaylistCount] = useState(0);
  const [accountData, setAccountData] = useState({
    username: user?.username || "",
    password: "",
  });
  const [accountSaved, setAccountSaved] = useState(false);

  useEffect(() => {
    const fetchData = async () => {
      const [recentRes, favoritesRes, playlistsRes] =
        await Promise.allSettled([
          apiClient.get("/api/progress/recent"),
          apiClient.get("/api/favorites"),
          apiClient.get("/api/playlists"),
        ]);

      if (recentRes.status === "fulfilled") {
        setRecentPlays(recentRes.value.data || []);
      }
      if (favoritesRes.status === "fulfilled") {
        setFavorites(favoritesRes.value.data || []);
      }
      if (playlistsRes.status === "fulfilled") {
        setPlaylistCount(
          ((playlistsRes.value.data as Playlist[]) || []).length,
        );
      }
    };

    fetchData();
    window.addEventListener("focus", fetchData);
    return () => window.removeEventListener("focus", fetchData);
  }, []);

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setAccountData((current) => ({
      ...current,
      username: user?.username || "",
    }));
  }, [user?.username]);

  const listenedMinutes = Math.round(
    recentPlays.reduce(
      (total, progress) => total + Math.max(0, progress.position || 0),
      0,
    ) / 60,
  );
  const recentBookCount = new Set(
    recentPlays.map((progress) => progress.book_id).filter(Boolean),
  ).size;
  const userInitial = user?.username?.charAt(0).toUpperCase() || "U";

  const handleAccountUpdate = async (event: React.FormEvent) => {
    event.preventDefault();
    const nextUsername = accountData.username.trim();
    const nextPassword = accountData.password.trim();

    if (!nextUsername) {
      alert(t("mine.usernameRequired"));
      return;
    }

    try {
      const updateData: Record<string, string> = {};
      if (nextUsername !== user?.username) {
        updateData.username = nextUsername;
      }
      if (nextPassword) {
        updateData.password = nextPassword;
      }

      if (Object.keys(updateData).length > 0) {
        await apiClient.patch("/api/me", updateData);
        if (updateData.username && user) {
          setUser({ ...user, username: updateData.username });
        }
      }

      setAccountData({ username: nextUsername, password: "" });
      setAccountSaved(true);
      setTimeout(() => setAccountSaved(false), 1800);
    } catch (err: unknown) {
      const message =
        err instanceof Error ? err.message : t("mine.updateFailed");
      alert(message);
    }
  };

  return (
    <div className="flex-1 min-h-full flex flex-col p-4 sm:p-6 md:p-8 animate-in fade-in duration-500">
      <div className="flex-1 space-y-6 max-w-5xl w-full mx-auto">
        <section className="bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 rounded-3xl p-5 md:p-6 shadow-sm">
          <div className="flex items-center gap-4">
            <div className="w-16 h-16 rounded-2xl bg-primary-100 dark:bg-primary-900/30 text-primary-600 flex items-center justify-center text-2xl font-bold shrink-0">
              {userInitial}
            </div>
            <div className="min-w-0">
              <p className="text-sm text-slate-500 dark:text-slate-400">
                {t("nav.mine")}
              </p>
              <h1 className="text-2xl md:text-3xl font-bold text-slate-900 dark:text-white truncate">
                {user?.username || t("mine.defaultUser")}
              </h1>
              <p className="text-sm text-slate-500 mt-1">{t("mine.intro")}</p>
            </div>
          </div>

          <form
            onSubmit={handleAccountUpdate}
            className="mt-5 grid grid-cols-1 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto] gap-3 items-end"
          >
            <label className="block min-w-0">
              <span className="text-xs font-bold text-slate-500 dark:text-slate-400">
                {t("auth.username")}
              </span>
              <div className="relative mt-1.5">
                <User
                  className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400"
                  size={17}
                />
                <input
                  type="text"
                  value={accountData.username}
                  onChange={(event) =>
                    setAccountData({
                      ...accountData,
                      username: event.target.value,
                    })
                  }
                  className="w-full pl-10 pr-4 py-2.5 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white text-sm"
                />
              </div>
            </label>
            <label className="block min-w-0">
              <span className="text-xs font-bold text-slate-500 dark:text-slate-400">
                {t("mine.changePassword")}
              </span>
              <div className="relative mt-1.5">
                <Key
                  className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400"
                  size={17}
                />
                <input
                  type="password"
                  value={accountData.password}
                  onChange={(event) =>
                    setAccountData({
                      ...accountData,
                      password: event.target.value,
                    })
                  }
                  placeholder={t("mine.passwordUnchangedHint")}
                  className="w-full pl-10 pr-4 py-2.5 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white text-sm"
                />
              </div>
            </label>
            <div className="flex items-center gap-3">
              {accountSaved && (
                <span className="text-sm text-green-600 font-bold whitespace-nowrap">
                  {t("mine.accountUpdated")}
                </span>
              )}
              <button
                type="submit"
                className="inline-flex items-center justify-center gap-2 px-4 py-2.5 bg-primary-600 hover:bg-primary-700 text-white font-bold rounded-xl shadow-lg shadow-primary-500/20 transition-colors text-sm whitespace-nowrap"
              >
                <Save size={16} />
                {t("mine.save")}
              </button>
            </div>
          </form>

          <div className="grid grid-cols-3 gap-3 mt-6">
            <SummaryCard
              label={t("mine.recent")}
              value={recentBookCount}
              unit={t("mine.bookUnit")}
            />
            <SummaryCard
              label={t("mine.favorites")}
              value={favorites.length}
              unit={t("mine.bookUnit")}
            />
            <SummaryCard
              label={t("mine.playlists")}
              value={playlistCount}
              unit={t("mine.playlistUnit")}
            />
          </div>
        </section>

        <section className="space-y-3">
          <SectionHeader title={t("mine.myContent")} />
          <div className="bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 rounded-3xl shadow-sm overflow-hidden">
            <EntryItem
              to="/history"
              icon={<History size={22} />}
              title={t("mine.historyTitle")}
              description={
                recentPlays.length > 0
                  ? t("mine.historyDescription", {
                      books: recentBookCount,
                      chapters: recentPlays.length,
                      minutes: listenedMinutes || 0,
                    })
                  : t("mine.historyEmptyDescription")
              }
              tone="text-primary-600 bg-primary-50 dark:bg-primary-900/20"
            />
            <EntryItem
              to="/favorites"
              icon={<Heart size={22} />}
              title={t("mine.favoritesTitle")}
              description={t("mine.favoritesDescription", {
                count: favorites.length,
              })}
              tone="text-red-500 bg-red-50 dark:bg-red-900/20"
            />
          </div>
        </section>

        <section className="space-y-3">
          <SectionHeader title={t("mine.settingsManagement")} />
          <div className="bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 rounded-3xl shadow-sm overflow-hidden">
            <EntryItem
              to="/personalization"
              icon={<Settings size={22} />}
              title={t("settings.title")}
              description={t("mine.personalizationDescription")}
              tone="text-blue-600 bg-blue-50 dark:bg-blue-900/20"
            />
            {user?.role === "admin" && (
              <EntryItem
                to="/notifications"
                icon={<BellRing size={22} />}
                title={t("mine.notificationTitle")}
                description={t("mine.notificationDescription")}
                tone="text-emerald-600 bg-emerald-50 dark:bg-emerald-900/20"
              />
            )}
            {user?.role === "admin" && (
              <EntryItem
                to="/statistics"
                icon={<BarChart3 size={22} />}
                title={t("mine.statisticsTitle")}
                description={t("mine.statisticsDescription")}
                tone="text-violet-600 bg-violet-50 dark:bg-violet-900/20"
              />
            )}
          </div>
        </section>

        <div className="text-center text-slate-400 text-sm pt-2 pb-4">
          <Link
            to="/about"
            className="inline-flex items-center gap-2 text-slate-400 hover:text-primary-600 transition-colors text-sm font-bold underline decoration-slate-300 dark:decoration-slate-700 underline-offset-4"
          >
            <Info size={16} />
            {t("mine.aboutTitle")}
          </Link>
          <p className="mt-4 text-xs opacity-60">{t("auth.copyright")}</p>
        </div>
      </div>

      <div
        className="shrink-0 transition-all duration-300"
        style={{
          height: currentChapter
            ? "var(--safe-bottom-with-player)"
            : "var(--safe-bottom-base)",
        }}
      />
    </div>
  );
};

const SummaryCard = ({
  label,
  value,
  unit,
}: {
  label: string;
  value: number;
  unit: string;
}) => (
  <div className="rounded-2xl bg-slate-50 dark:bg-slate-800/70 p-3 text-center min-w-0">
    <p className="text-lg md:text-xl font-black text-slate-900 dark:text-white truncate">
      {value}
      <span className="text-xs font-bold text-slate-400 ml-1">{unit}</span>
    </p>
    <p className="text-xs text-slate-500 font-bold mt-1">{label}</p>
  </div>
);

const SectionHeader = ({ title }: { title: string }) => (
  <h2 className="px-1 text-sm font-black text-slate-500 dark:text-slate-400">
    {title}
  </h2>
);

const EntryItem = ({
  to,
  icon,
  title,
  description,
  tone,
}: {
  to: string;
  icon: React.ReactNode;
  title: string;
  description: string;
  tone: string;
}) => (
  <Link
    to={to}
    className="flex items-center justify-between gap-4 px-4 md:px-5 py-4 border-b border-slate-100 dark:border-slate-800 last:border-b-0 hover:bg-slate-50 dark:hover:bg-slate-800 transition-colors"
  >
    <div className="flex items-center gap-3 min-w-0">
      <div
        className={`w-11 h-11 rounded-2xl flex items-center justify-center shrink-0 ${tone}`}
      >
        {icon}
      </div>
      <div className="min-w-0">
        <p className="font-bold text-slate-900 dark:text-white truncate">
          {title}
        </p>
        <p className="text-sm text-slate-500 truncate">{description}</p>
      </div>
    </div>
    <ChevronRight size={18} className="text-slate-300 shrink-0" />
  </Link>
);

export default MyPage;
