import { icons } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';

const iconComponents = icons as Record<string, LucideIcon | undefined>;

const toPascalCase = (value: string) => value
  .trim()
  .replace(/(^|[-_\s]+)([a-zA-Z0-9])/g, (_, __, letter: string) => letter.toUpperCase())
  .replace(/[^a-zA-Z0-9]/g, '');

// Keep the full registry behind one dynamic import instead of generating a
// chunk for every icon or loading every plugin icon with the application shell.
export const getPluginLucideIcon = (value: string): LucideIcon | undefined =>
  iconComponents[value]
  || iconComponents[toPascalCase(value)]
  || iconComponents[toPascalCase(value.replace(/^lucide:/i, ''))];
