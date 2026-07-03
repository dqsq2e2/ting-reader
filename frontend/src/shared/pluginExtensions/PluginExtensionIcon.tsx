import { icons, PlugZap } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type {
  ClientExtensionDescriptor,
  ClientExtensionIcon,
} from "../../core/pluginExtensions";

const iconComponents = icons as Record<string, LucideIcon | undefined>;

const toPascalCase = (value: string) =>
  value
    .trim()
    .replace(/(^|[-_\s]+)([a-zA-Z0-9])/g, (_, __, letter: string) =>
      letter.toUpperCase(),
    )
    .replace(/[^a-zA-Z0-9]/g, "");

const iconText = (icon: ClientExtensionIcon | undefined) => {
  if (typeof icon === "string") return icon.trim();
  if (!icon || typeof icon !== "object") return undefined;
  return (icon.src || icon.name || icon.value || "").trim();
};

const isImageIcon = (value: string) =>
  /^(https?:\/\/|data:image\/|\/)/i.test(value);

const PluginExtensionIcon = ({
  extension,
  size = 18,
}: {
  extension: ClientExtensionDescriptor;
  size?: number;
}) => {
  const raw = extension.icon;
  const value = iconText(raw);
  const explicitType =
    raw && typeof raw === "object" && typeof raw.type === "string"
      ? raw.type
      : undefined;

  if (
    value &&
    (explicitType === "image" || explicitType === "url" || isImageIcon(value))
  ) {
    return (
      <img
        src={value}
        alt=""
        className="h-full w-full rounded-[inherit] object-cover"
        draggable={false}
      />
    );
  }

  if (value && explicitType !== "emoji") {
    const Icon =
      iconComponents[value] ||
      iconComponents[toPascalCase(value)] ||
      iconComponents[toPascalCase(value.replace(/^lucide:/i, ""))];
    if (Icon) {
      return <Icon size={size} strokeWidth={2.2} />;
    }
  }

  if (value) {
    return (
      <span
        className="text-center text-[18px] leading-none"
        aria-hidden="true"
      >
        {value}
      </span>
    );
  }

  return <PlugZap size={size} strokeWidth={2.2} />;
};

export default PluginExtensionIcon;
