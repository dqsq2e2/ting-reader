import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import apiClient from "../../core/api/client";
import { formatLocalizedDate } from "../../core/utils/locale";
import {
  CHANGELOG_URL,
  PRIVACY_POLICY_URL,
  TING_READER_WEBSITE_URL,
  UPDATE_GUIDE_URL,
  USER_AGREEMENT_URL,
} from "../../core/constants/links";
import {
  CheckCircle2,
  ChevronRight,
  ExternalLink,
  FileText,
  Globe2,
  History,
  RefreshCw,
  ShieldCheck,
} from "lucide-react";
import BackButton from "../../shared/widgets/BackButton";

type UpdateInfo = {
  version: string;
  download_url?: string;
  size?: string;
  date: string;
};

const formatVersion = (version: string | undefined) => {
  const value = version?.trim();
  if (!value) return "";
  return value.startsWith("v") ? value : `v${value}`;
};

const AboutPage: React.FC = () => {
  const { t } = useTranslation();
  const [backendVersion, setBackendVersion] = useState("");
  const [loadingVersion, setLoadingVersion] = useState(true);
  const [checkingBackendUpdate, setCheckingBackendUpdate] = useState(false);
  const [backendUpdateInfo, setBackendUpdateInfo] = useState<UpdateInfo | null>(
    null,
  );

  useEffect(() => {
    let cancelled = false;
    apiClient
      .get("/api/health")
      .then((response) => {
        if (!cancelled) setBackendVersion(response.data?.version || "");
      })
      .finally(() => {
        if (!cancelled) setLoadingVersion(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const handleCheckBackendUpdate = async () => {
    if (checkingBackendUpdate || !backendVersion) return;
    setCheckingBackendUpdate(true);
    try {
      const { data } = await apiClient.get("/api/system/check-update");
      const remoteVersion = data.version.replace(/^v/, "");
      const currentVersion = backendVersion.replace(/^v/, "");

      if (remoteVersion !== currentVersion) {
        setBackendUpdateInfo(data);
      } else {
        setBackendUpdateInfo(null);
        alert(t("mine.backendUpToDate"));
      }
    } catch (error) {
      console.error("检查后端更新失败", error);
      alert(t("mine.checkBackendUpdateFailed"));
    } finally {
      setCheckingBackendUpdate(false);
    }
  };

  return (
    <div className="flex-1 min-h-full flex flex-col p-4 sm:p-6 md:p-8 animate-in fade-in duration-500">
      <div className="flex-1 space-y-6 max-w-4xl w-full mx-auto">
        <BackButton fallback="/mine" />

        <section className="bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 rounded-3xl p-6 md:p-8 shadow-sm">
          <div className="flex flex-col sm:flex-row sm:items-center gap-5">
            <img
              src="/logo.png"
              alt={t("common.logoAlt")}
              className="w-20 h-20 rounded-2xl object-contain shadow-sm"
            />
            <div className="min-w-0">
              <p className="text-sm font-bold uppercase tracking-wide text-primary-600">
                Ting Reader
              </p>
              <h1 className="text-2xl md:text-3xl font-black text-slate-900 dark:text-white tracking-tight">
                {t("mine.aboutTitle")}
              </h1>
              <p className="text-sm text-slate-500 dark:text-slate-400 mt-2">
                {t("mine.aboutDescription")}
              </p>
            </div>
          </div>

          <div className="grid grid-cols-1 gap-4 mt-8">
            <VersionCard
              label={t("mine.backendVersion")}
              version={
                loadingVersion
                  ? t("common.loading")
                  : formatVersion(backendVersion) ||
                    t("adminPlugins.unknownVersion")
              }
              actionLabel={
                checkingBackendUpdate
                  ? t("mine.checking")
                  : t("mine.checkUpdate")
              }
              actionDisabled={
                loadingVersion || checkingBackendUpdate || !backendVersion
              }
              onAction={handleCheckBackendUpdate}
            />
          </div>

          {backendUpdateInfo && (
            <div className="mt-5 rounded-2xl border border-blue-100 dark:border-blue-900/40 bg-blue-50 dark:bg-blue-950/20 p-4">
              <div className="flex items-start gap-3">
                <CheckCircle2
                  size={22}
                  className="text-blue-600 shrink-0 mt-0.5"
                />
                <div className="min-w-0 flex-1">
                  <p className="font-bold text-slate-900 dark:text-white">
                    {t("mine.newBackendVersion", {
                      version: backendUpdateInfo.version,
                    })}
                  </p>
                  {backendUpdateInfo.date && (
                    <p className="text-sm text-slate-500 mt-1">
                      {t("mine.releaseDate", {
                        date: formatLocalizedDate(
                          new Date(backendUpdateInfo.date),
                          {
                            year: "numeric",
                            month: "short",
                            day: "numeric",
                          },
                        ),
                      })}
                    </p>
                  )}
                  <div className="flex flex-wrap gap-2 mt-4">
                    <a
                      href={UPDATE_GUIDE_URL}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="inline-flex items-center gap-2 px-3 py-2 rounded-xl bg-blue-600 text-white text-sm font-bold hover:bg-blue-700 transition-colors"
                    >
                      {t("mine.goUpdate")}
                      <ExternalLink size={15} />
                    </a>
                    <button
                      onClick={() => setBackendUpdateInfo(null)}
                      className="px-3 py-2 rounded-xl bg-white dark:bg-slate-900 text-slate-600 dark:text-slate-300 text-sm font-bold border border-slate-200 dark:border-slate-700"
                    >
                      {t("mine.later")}
                    </button>
                  </div>
                </div>
              </div>
            </div>
          )}
        </section>

        <section className="bg-white dark:bg-slate-900 border border-slate-100 dark:border-slate-800 rounded-3xl p-5 md:p-6 shadow-sm">
          <p className="text-sm font-black text-primary-600 mb-4">
            {t("mine.officialLinks")}
          </p>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-x-8 gap-y-5">
            <LinkCard
              icon={<Globe2 size={24} />}
              label={t("mine.officialWebsite")}
              value={TING_READER_WEBSITE_URL}
              href={TING_READER_WEBSITE_URL}
              tone="bg-blue-50 dark:bg-blue-900/20 text-blue-600"
            />
            <LinkCard
              icon={<FileText size={24} />}
              label={t("mine.userAgreement")}
              value={t("mine.userAgreementDescription")}
              href={USER_AGREEMENT_URL}
              tone="bg-indigo-50 dark:bg-indigo-900/20 text-indigo-600"
            />
            <LinkCard
              icon={<ShieldCheck size={24} />}
              label={t("mine.privacyPolicy")}
              value={t("mine.privacyPolicyDescription")}
              href={PRIVACY_POLICY_URL}
              tone="bg-emerald-50 dark:bg-emerald-900/20 text-emerald-600"
            />
            <LinkCard
              icon={<History size={24} />}
              label={t("mine.changelog")}
              value={t("mine.changelogDescription")}
              href={CHANGELOG_URL}
              tone="bg-orange-50 dark:bg-orange-900/20 text-orange-600"
            />
          </div>
        </section>
      </div>
    </div>
  );
};

const VersionCard = ({
  label,
  version,
  actionLabel,
  actionDisabled,
  onAction,
}: {
  label: string;
  version: string;
  actionLabel?: string;
  actionDisabled?: boolean;
  onAction?: () => void;
}) => (
  <div className="rounded-2xl bg-slate-50 dark:bg-slate-800/70 border border-slate-100 dark:border-slate-700 p-4">
    <p className="text-sm font-bold text-slate-500 dark:text-slate-400">
      {label}
    </p>
    <div className="mt-3 flex items-center justify-between gap-3">
      <p className="text-lg font-black text-slate-900 dark:text-white truncate">
        {version}
      </p>
      {onAction && actionLabel && (
        <button
          onClick={onAction}
          disabled={actionDisabled}
          className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-primary-50 dark:bg-primary-900/20 text-primary-600 text-xs font-bold hover:bg-primary-100 dark:hover:bg-primary-900/40 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          <RefreshCw size={13} className={actionDisabled ? "" : undefined} />
          {actionLabel}
        </button>
      )}
    </div>
  </div>
);

const LinkCard = ({
  icon,
  label,
  value,
  href,
  tone,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
  href: string;
  tone: string;
}) => (
  <a
    href={href}
    target="_blank"
    rel="noopener noreferrer"
    className="group flex items-center gap-4 rounded-2xl p-3 transition-colors hover:bg-slate-50 dark:hover:bg-slate-800/70"
  >
    <div className={`w-14 h-14 rounded-2xl flex items-center justify-center shrink-0 ${tone}`}>
      {icon}
    </div>
    <div className="min-w-0 flex-1">
      <p className="font-bold text-slate-900 dark:text-white truncate">
        {label}
      </p>
      <p className="text-sm text-slate-500 mt-1 inline-flex items-center gap-1 min-w-0">
        <span className="truncate">{value}</span>
        <ExternalLink size={13} className="shrink-0" />
      </p>
    </div>
    <ChevronRight
      size={20}
      className="text-slate-300 shrink-0 transition-transform group-hover:translate-x-0.5"
    />
  </a>
);

export default AboutPage;
