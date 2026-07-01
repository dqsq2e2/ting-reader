import React, { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import apiClient from "../../core/api/client";
import {
  normalizeTheme,
  useTheme,
  type Theme,
} from "../../core/hooks/useTheme";
import { usePlayerStore } from "../../core/stores/playerStore";
import { useAuthStore } from "../../core/stores/authStore";
import {
  languageLabels,
  normalizeLanguage,
  supportedLanguages,
  type SupportedLanguage,
} from "../../core/i18n/locales";
import { useAppLanguage } from "../../core/i18n/useAppLanguage";
import {
  DEFAULT_HOME_LAYOUT,
  normalizeHomeLayout,
  serializeHomeLayout,
  type HomeLayoutSettings,
} from "../../core/utils/homeLayout";
import BackButton from "../../shared/widgets/BackButton";
import {
  CheckCircle2,
  ChevronDown,
  Code,
  Copy,
  ExternalLink,
  FastForward,
  Home,
  Info,
  Key,
  Languages,
  Monitor,
  Moon,
  Settings,
  Sun,
  User,
} from "lucide-react";
import LoadingSpinner from "../../shared/ui/LoadingSpinner";
import PluginExtensionSlot from "../../shared/pluginExtensions/PluginExtensionSlot";

type SettingsState = {
  playback_speed: number;
  sleep_timer_default: number;
  auto_preload: boolean;
  auto_cache?: boolean;
  widget_css?: string;
  theme: Theme;
  language: SupportedLanguage;
  settings_json?: Record<string, unknown>;
  home_layout?: HomeLayoutSettings;
  user_id?: string;
  updated_at?: string;
  [key: string]: unknown;
};

const defaultSettings: SettingsState = {
  playback_speed: 1.0,
  sleep_timer_default: 0,
  auto_preload: false,
  auto_cache: false,
  widget_css: "",
  theme: "system",
  language: "zh-CN",
  home_layout: DEFAULT_HOME_LAYOUT,
};

const PersonalizationPage: React.FC = () => {
  const { t } = useTranslation();
  const user = useAuthStore((state) => state.user);
  const token = useAuthStore((state) => state.token);
  const currentChapter = usePlayerStore((state) => state.currentChapter);
  const setPlaybackSpeed = usePlayerStore((state) => state.setPlaybackSpeed);
  const { applyTheme } = useTheme();
  const { language, setLanguage } = useAppLanguage();
  const [settings, setSettings] = useState<SettingsState>(defaultSettings);
  const [loading, setLoading] = useState(true);
  const [saved, setSaved] = useState(false);
  const [widgetEmbedType, setWidgetEmbedType] = useState<"private" | "public">(
    "private",
  );

  useEffect(() => {
    const fetchSettings = async () => {
      try {
        const response = await apiClient.get("/api/settings");
        const data = response.data;
        const fetchedSettings: SettingsState = {
          ...defaultSettings,
          ...data,
          playback_speed: data.playback_speed ?? defaultSettings.playback_speed,
          sleep_timer_default:
            data.sleep_timer_default ?? defaultSettings.sleep_timer_default,
          auto_preload: data.auto_preload ?? defaultSettings.auto_preload,
          auto_cache: !!data.auto_cache,
          widget_css: data.widget_css ?? "",
          theme: normalizeTheme(data.theme),
          language: normalizeLanguage(
            data.language ?? data.settings_json?.language ?? language,
          ),
          home_layout: normalizeHomeLayout(
            data.settings_json?.home_layout ?? data.home_layout,
          ),
        };
        setSettings(fetchedSettings);
        applyTheme(fetchedSettings.theme);
        void setLanguage(fetchedSettings.language, false);
      } catch (err) {
        console.error("获取设置失败", err);
      } finally {
        setLoading(false);
      }
    };

    fetchSettings();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const sanitizeSettings = (data: SettingsState) => {
    const cleanSettings: Record<string, unknown> = {
      ...data,
      home_layout: serializeHomeLayout(data.home_layout ?? DEFAULT_HOME_LAYOUT),
    };
    delete cleanSettings.settings_json;
    delete cleanSettings.user_id;
    delete cleanSettings.updated_at;
    if (user?.role !== "admin") {
      delete cleanSettings.auto_cache;
      delete cleanSettings.widget_css;
    }
    return cleanSettings;
  };

  const handleSaveSettings = async (patch: Partial<SettingsState>) => {
    const nextSettings = { ...settings, ...patch };

    try {
      await apiClient.post("/api/settings", sanitizeSettings(nextSettings));
      setSettings(nextSettings);

      if (typeof patch.playback_speed === "number") {
        setPlaybackSpeed(patch.playback_speed);
      }
      if (patch.theme) {
        applyTheme(patch.theme);
      }
      if (patch.language) {
        void setLanguage(normalizeLanguage(patch.language), false);
      }

      setSaved(true);
      setTimeout(() => setSaved(false), 1800);
    } catch {
      alert(t("common.saveFailed"));
    }
  };

  const handleSaveHomeLayout = async (patch: Partial<HomeLayoutSettings>) => {
    const currentLayout = settings.home_layout ?? DEFAULT_HOME_LAYOUT;
    const nextHomeLayout = { ...currentLayout, ...patch };
    await handleSaveSettings({ home_layout: nextHomeLayout });
  };

  const widgetToken =
    widgetEmbedType === "private" && token ? `?token=${token}` : "";
  const widgetUrl = `${window.location.origin}/widget${widgetToken}`;
  const embedCode = `<iframe src="${widgetUrl}" width="100%" height="150" frameborder="0" allow="autoplay; fullscreen"></iframe>`;
  const homeLayout = settings.home_layout ?? DEFAULT_HOME_LAYOUT;
  const layoutCodes = useMemo(
    () => ({
      fixedBottom: `<div style="position: fixed; bottom: 0; left: 0; width: 100%; z-index: 9999;">
  <iframe src="${widgetUrl}" width="100%" height="150" frameborder="0" allow="autoplay; fullscreen"></iframe>
</div>`,
      floatingRight: `<div style="position: fixed; bottom: 20px; right: 20px; width: 350px; height: 150px; z-index: 9999; border-radius: 16px; overflow: hidden; box-shadow: 0 4px 20px rgba(0,0,0,0.15);">
  <iframe src="${widgetUrl}" width="100%" height="100%" frameborder="0" allow="autoplay; fullscreen"></iframe>
</div>`,
    }),
    [widgetUrl],
  );

  const handleCopy = async (text: string) => {
    try {
      if (navigator.clipboard && navigator.clipboard.writeText) {
        await navigator.clipboard.writeText(text);
      } else {
        const textArea = document.createElement("textarea");
        textArea.value = text;
        textArea.style.position = "fixed";
        textArea.style.left = "-9999px";
        textArea.style.top = "0";
        document.body.appendChild(textArea);
        textArea.focus();
        textArea.select();
        document.execCommand("copy");
        document.body.removeChild(textArea);
      }
      alert(t("common.copied"));
    } catch (err) {
      console.error("复制失败:", err);
      alert(t("common.copyFailed"));
    }
  };

  if (loading) {
    return <LoadingSpinner />;
  }

  return (
    <div className="flex-1 min-h-full flex flex-col p-4 sm:p-6 md:p-8 animate-in fade-in duration-500">
      <div className="flex-1 space-y-6 max-w-5xl w-full mx-auto">
        <BackButton fallback="/mine" />

        <div className="flex items-center justify-between gap-4">
          <div>
            <h1 className="text-2xl md:text-3xl font-bold text-slate-900 dark:text-white flex items-center gap-3">
              <Settings className="text-primary-600" />
              {t("settings.title")}
            </h1>
            <p className="text-sm md:text-base text-slate-500 dark:text-slate-400 mt-1">
              {t("settings.subtitle")}
            </p>
          </div>
          {saved && (
            <span className="text-sm text-green-600 font-bold flex items-center gap-1">
              <CheckCircle2 size={14} />
              {t("common.saved")}
            </span>
          )}
        </div>

        <PluginExtensionSlot
          slot="settings.section"
          className="flex justify-end gap-2"
          context={{
            page: "settings",
            user_id: user?.id,
            role: user?.role,
            language: settings.language,
            theme: settings.theme,
          }}
        />

        <section className="bg-white dark:bg-slate-900 rounded-3xl p-5 md:p-6 border border-slate-100 dark:border-slate-800 shadow-sm">
          <h2 className="text-xl font-bold dark:text-white mb-6 flex items-center gap-2">
            <Monitor size={20} className="text-blue-500" />
            {t("settings.appearance")}
          </h2>
          <div className="grid grid-cols-3 gap-2 md:gap-4">
            {[
              {
                id: "light" as Theme,
                icon: <Sun size={20} />,
                label: t("settings.light"),
              },
              {
                id: "dark" as Theme,
                icon: <Moon size={20} />,
                label: t("settings.dark"),
              },
              {
                id: "system" as Theme,
                icon: <Monitor size={20} />,
                label: t("settings.system"),
              },
            ].map((theme) => (
              <button
                key={theme.id}
                onClick={() => handleSaveSettings({ theme: theme.id })}
                className={`flex flex-col items-center gap-2 md:gap-3 p-3 md:p-4 rounded-2xl border-2 transition-all ${
                  settings.theme === theme.id
                    ? "border-primary-600 bg-primary-50 dark:bg-primary-900/20 text-primary-600"
                    : "border-slate-100 dark:border-slate-800 text-slate-500 hover:bg-slate-50 dark:hover:bg-slate-800"
                }`}
              >
                {theme.icon}
                <span className="text-xs md:text-sm font-bold text-center leading-tight">
                  {theme.label}
                </span>
              </button>
            ))}
          </div>
        </section>

        <section className="bg-white dark:bg-slate-900 rounded-3xl p-5 md:p-6 border border-slate-100 dark:border-slate-800 shadow-sm">
          <div className="flex flex-col md:flex-row md:items-center md:justify-between gap-4">
            <div className="min-w-0">
              <h2 className="text-xl font-bold dark:text-white mb-2 flex items-center gap-2">
                <Languages size={20} className="text-cyan-500" />
                {t("settings.language")}
              </h2>
              <p className="text-sm text-slate-500 dark:text-slate-400">
                {t("settings.languageDescription")}
              </p>
            </div>
            <label className="relative block w-full md:w-64 shrink-0">
              <span className="sr-only">{t("settings.language")}</span>
              <Languages
                size={18}
                className="pointer-events-none absolute left-4 top-1/2 -translate-y-1/2 text-primary-600"
              />
              <select
                value={settings.language}
                onChange={(event) =>
                  handleSaveSettings({
                    language: normalizeLanguage(event.target.value),
                  })
                }
                className="w-full appearance-none rounded-2xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-950 py-3 pl-11 pr-10 text-sm font-bold text-slate-700 dark:text-slate-200 outline-none transition focus:border-primary-500 focus:ring-2 focus:ring-primary-500/20"
              >
                {supportedLanguages.map((option) => (
                  <option key={option} value={option}>
                    {languageLabels[option]}
                  </option>
                ))}
              </select>
              <ChevronDown
                size={16}
                className="pointer-events-none absolute right-4 top-1/2 -translate-y-1/2 text-slate-400"
              />
            </label>
          </div>
        </section>

        <section className="bg-white dark:bg-slate-900 rounded-3xl p-5 md:p-6 border border-slate-100 dark:border-slate-800 shadow-sm">
          <h2 className="text-xl font-bold dark:text-white mb-6 flex items-center gap-2">
            <Home size={20} className="text-emerald-500" />
            {t("settings.homeLayout")}
          </h2>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            <HomeLayoutToggle
              title={t("settings.homeHero")}
              description={t("settings.homeHeroDescription")}
              checked={homeLayout.showHero}
              onChange={() =>
                handleSaveHomeLayout({ showHero: !homeLayout.showHero })
              }
            />
            <HomeLayoutToggle
              title={t("settings.homeStats")}
              description={t("settings.homeStatsDescription")}
              checked={homeLayout.showStats}
              onChange={() =>
                handleSaveHomeLayout({ showStats: !homeLayout.showStats })
              }
            />
            <HomeLayoutToggle
              title={t("settings.homeRecommended")}
              description={t("settings.homeRecommendedDescription")}
              checked={homeLayout.showRecommended}
              onChange={() =>
                handleSaveHomeLayout({
                  showRecommended: !homeLayout.showRecommended,
                })
              }
            />
            <HomeLayoutToggle
              title={t("settings.homeRecent")}
              description={t("settings.homeRecentDescription")}
              checked={homeLayout.showRecent}
              onChange={() =>
                handleSaveHomeLayout({ showRecent: !homeLayout.showRecent })
              }
            />
            <HomeLayoutToggle
              title={t("settings.homeRecentlyAdded")}
              description={t("settings.homeRecentlyAddedDescription")}
              checked={homeLayout.showRecentlyAdded}
              onChange={() =>
                handleSaveHomeLayout({
                  showRecentlyAdded: !homeLayout.showRecentlyAdded,
                })
              }
            />
            <HomeLayoutToggle
              title={t("settings.homeCollections")}
              description={t("settings.homeCollectionsDescription")}
              checked={homeLayout.showCollections}
              onChange={() =>
                handleSaveHomeLayout({
                  showCollections: !homeLayout.showCollections,
                })
              }
            />
          </div>
        </section>

        <section className="bg-white dark:bg-slate-900 rounded-3xl p-5 md:p-6 border border-slate-100 dark:border-slate-800 shadow-sm">
          <h2 className="text-xl font-bold dark:text-white mb-6 flex items-center gap-2">
            <FastForward size={20} className="text-orange-500" />
            {t("settings.playback")}
          </h2>
          <div className="space-y-6">
            <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
              <div>
                <p className="font-bold dark:text-white">
                  {t("settings.playbackSpeed")}
                </p>
                <p className="text-xs md:text-sm text-slate-500">
                  {t("settings.playbackSpeedDescription")}
                </p>
              </div>
              <div className="flex bg-slate-100 dark:bg-slate-800 p-1 rounded-xl self-start sm:self-auto w-full sm:w-auto">
                {[1.0, 1.25, 1.5, 2.0].map((speed) => (
                  <button
                    key={speed}
                    onClick={() =>
                      handleSaveSettings({ playback_speed: speed })
                    }
                    className={`flex-1 sm:flex-none px-2 md:px-4 py-2 text-sm font-bold rounded-lg transition-all ${
                      settings.playback_speed === speed
                        ? "bg-white dark:bg-slate-700 shadow-sm text-primary-600"
                        : "text-slate-500"
                    }`}
                  >
                    {speed}x
                  </button>
                ))}
              </div>
            </div>

            <ToggleRow
              title={t("settings.autoPreload")}
              description={t("settings.autoPreloadDescription")}
              checked={settings.auto_preload}
              onChange={() =>
                handleSaveSettings({ auto_preload: !settings.auto_preload })
              }
            />
            {user?.role === "admin" && (
              <ToggleRow
                title={t("settings.autoCache")}
                description={t("settings.autoCacheDescription")}
                checked={!!settings.auto_cache}
                onChange={() =>
                  handleSaveSettings({ auto_cache: !settings.auto_cache })
                }
              />
            )}
          </div>
        </section>

        {user?.role === "admin" && (
          <section className="bg-white dark:bg-slate-900 rounded-3xl p-5 md:p-6 border border-slate-100 dark:border-slate-800 shadow-sm">
            <div className="flex flex-col lg:flex-row lg:items-center lg:justify-between gap-4 mb-6">
              <h2 className="text-xl font-bold dark:text-white flex items-center gap-2">
                <Code size={20} className="text-violet-500" />
                {t("settings.widget")}
              </h2>
              <a
                href="/widget"
                target="_blank"
                rel="noopener noreferrer"
                className="inline-flex items-center justify-center gap-2 px-3 py-2 rounded-xl bg-slate-100 dark:bg-slate-800 text-sm font-bold text-slate-600 dark:text-slate-300 hover:text-primary-600 transition-colors"
              >
                <ExternalLink size={16} />
                {t("settings.openWidget")}
              </a>
            </div>

            <div className="space-y-5">
              <div>
                <div className="flex items-center justify-between gap-3 mb-2">
                  <p className="font-bold dark:text-white">
                    {t("settings.customCss")}
                  </p>
                  <span className="text-[10px] text-slate-400 uppercase font-bold">
                    {t("settings.widgetOnly")}
                  </span>
                </div>
                <textarea
                  value={settings.widget_css || ""}
                  onChange={(event) =>
                    setSettings((prev) => ({
                      ...prev,
                      widget_css: event.target.value,
                    }))
                  }
                  onBlur={() =>
                    handleSaveSettings({
                      widget_css: settings.widget_css || "",
                    })
                  }
                  placeholder=".widget-mode { background: transparent !important; }"
                  className="w-full h-32 px-4 py-3 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-2xl outline-none focus:ring-2 focus:ring-primary-500 dark:text-white font-mono text-sm resize-none"
                />
              </div>

              <div className="rounded-2xl bg-slate-50 dark:bg-slate-800/70 border border-slate-100 dark:border-slate-800 p-4 space-y-4">
                <div className="flex items-center justify-between gap-3">
                  <p className="font-bold dark:text-white">
                    {t("settings.embedCode")}
                  </p>
                  <div className="flex bg-white dark:bg-slate-900 rounded-xl p-1 border border-slate-100 dark:border-slate-700">
                    <button
                      onClick={() => setWidgetEmbedType("private")}
                      className={`flex items-center gap-1.5 px-3 py-1.5 text-xs font-bold rounded-lg transition-all ${
                        widgetEmbedType === "private"
                          ? "bg-primary-50 dark:bg-primary-900/20 text-primary-600"
                          : "text-slate-500"
                      }`}
                    >
                      <Key size={13} />
                      {t("settings.privateEmbed")}
                    </button>
                    <button
                      onClick={() => setWidgetEmbedType("public")}
                      className={`flex items-center gap-1.5 px-3 py-1.5 text-xs font-bold rounded-lg transition-all ${
                        widgetEmbedType === "public"
                          ? "bg-primary-50 dark:bg-primary-900/20 text-primary-600"
                          : "text-slate-500"
                      }`}
                    >
                      <User size={13} />
                      {t("settings.publicEmbed")}
                    </button>
                  </div>
                </div>

                <CopyBlock code={embedCode} onCopy={handleCopy} />

                <div className="grid grid-cols-1 lg:grid-cols-2 gap-3">
                  <EmbedSnippetCard
                    title={t("settings.fixedBottom")}
                    code={layoutCodes.fixedBottom}
                    onCopy={handleCopy}
                  />
                  <EmbedSnippetCard
                    title={t("settings.floatingRight")}
                    code={layoutCodes.floatingRight}
                    onCopy={handleCopy}
                  />
                </div>

                <div className="flex gap-2 text-[11px] text-slate-500 dark:text-slate-400">
                  <Info size={14} className="shrink-0 mt-0.5" />
                  <span>
                    {widgetEmbedType === "private"
                      ? t("settings.privateEmbedHint")
                      : t("settings.publicEmbedHint")}
                  </span>
                </div>
              </div>
            </div>
          </section>
        )}
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

const ToggleRow = ({
  title,
  description,
  checked,
  onChange,
}: {
  title: string;
  description: string;
  checked: boolean;
  onChange: () => void;
}) => (
  <div className="flex items-center justify-between gap-4 pt-4 border-t border-slate-100 dark:border-slate-800">
    <div className="flex-1 min-w-0">
      <p className="font-bold dark:text-white truncate">{title}</p>
      <p className="text-xs md:text-sm text-slate-500 line-clamp-2">
        {description}
      </p>
    </div>
    <button
      onClick={onChange}
      className={`flex-shrink-0 w-12 md:w-14 h-7 md:h-8 rounded-full transition-all relative ${
        checked ? "bg-primary-600" : "bg-slate-200 dark:bg-slate-700"
      }`}
    >
      <div
        className={`absolute top-1 w-5 md:w-6 h-5 md:h-6 bg-white rounded-full transition-all ${
          checked ? "left-6 md:left-7" : "left-1"
        }`}
      />
    </button>
  </div>
);

const HomeLayoutToggle = ({
  title,
  description,
  checked,
  onChange,
}: {
  title: string;
  description: string;
  checked: boolean;
  onChange: () => void;
}) => (
  <button
    type="button"
    onClick={onChange}
    className={`text-left rounded-2xl border p-4 transition-all ${
      checked
        ? "border-primary-200 dark:border-primary-900/50 bg-primary-50/80 dark:bg-primary-900/20"
        : "border-slate-100 dark:border-slate-800 bg-slate-50 dark:bg-slate-800/50 hover:bg-slate-100 dark:hover:bg-slate-800"
    }`}
  >
    <div className="flex items-start justify-between gap-4">
      <div className="min-w-0">
        <p className="font-bold text-slate-900 dark:text-white">{title}</p>
        <p className="text-xs text-slate-500 line-clamp-2 mt-1">
          {description}
        </p>
      </div>
      <span
        className={`shrink-0 w-11 h-6 rounded-full transition-all relative ${
          checked ? "bg-primary-600" : "bg-slate-200 dark:bg-slate-700"
        }`}
      >
        <span
          className={`absolute top-1 w-4 h-4 bg-white rounded-full transition-all ${
            checked ? "left-6" : "left-1"
          }`}
        />
      </span>
    </div>
  </button>
);

const CopyBlock = ({
  code,
  onCopy,
}: {
  code: string;
  onCopy: (code: string) => void;
}) => {
  const { t } = useTranslation();
  return (
    <div className="relative">
      <code className="text-[10px] md:text-xs text-slate-600 dark:text-slate-400 break-all bg-white dark:bg-slate-950 p-3 pr-11 rounded-xl block border border-slate-100 dark:border-slate-900 font-mono leading-relaxed">
        {code}
      </code>
      <button
        onClick={() => onCopy(code)}
        className="absolute top-2 right-2 p-1.5 bg-slate-100 dark:bg-slate-800 hover:bg-primary-50 dark:hover:bg-primary-900/30 text-slate-500 hover:text-primary-600 rounded-lg transition-colors"
        title={t("common.copy")}
      >
        <Copy size={14} />
      </button>
    </div>
  );
};

const EmbedSnippetCard = ({
  title,
  code,
  onCopy,
}: {
  title: string;
  code: string;
  onCopy: (code: string) => void;
}) => {
  const { t } = useTranslation();
  return (
    <div className="relative bg-white dark:bg-slate-900 rounded-xl border border-slate-100 dark:border-slate-700 p-3">
      <p className="text-xs font-bold text-slate-500 mb-2">{title}</p>
      <code className="text-[10px] text-slate-600 dark:text-slate-400 font-mono block whitespace-pre overflow-x-auto pr-9">
        {code}
      </code>
      <button
        onClick={() => onCopy(code)}
        className="absolute top-3 right-3 p-1.5 bg-slate-100 dark:bg-slate-800 hover:bg-primary-50 dark:hover:bg-primary-900/30 text-slate-500 hover:text-primary-600 rounded-lg transition-colors"
        title={t("common.copy")}
      >
        <Copy size={14} />
      </button>
    </div>
  );
};

export default PersonalizationPage;
