// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

/**
 * Icon Registry — single source of truth for all UI icons.
 *
 * Consumer components MUST import `<Icon>` from `$lib/icons` and pass a
 * semantic `name`. They MUST NOT import directly from `@lucide/svelte` or
 * `simple-icons`. Adding a new icon = adding one row to `ICONS` below.
 *
 * Two-library policy:
 *  - Lucide v1 (`@lucide/svelte`) for all functional/UI icons.
 *  - Simple Icons (`simple-icons`) for brand logos only — Lucide v1 removed
 *    brand icons for trademark reasons.
 */
import {
  AlertCircle,
  AlertTriangle,
  ArrowLeft,
  ArrowRight,
  ArrowUpCircle,
  BarChart3,
  BookOpen,
  Check,
  CheckCircle2,
  ChevronDown,
  ChevronLeft,
  Cpu,
  ChevronRight,
  Clipboard,
  ClipboardCopy,
  ClipboardPaste,
  Crown,
  Download,
  Eye,
  EyeOff,
  ExternalLink,
  FileText,
  Folder,
  Globe,
  Info,
  Keyboard,
  Lightbulb,
  Loader2,
  Lock,
  LogIn,
  LogOut,
  Mail,
  Megaphone,
  Menu,
  MessageCircle,
  Mic,
  MicOff,
  Monitor,
  MousePointerClick,
  NotebookPen,
  PartyPopper,
  RefreshCw,
  Rocket,
  Save,
  ScrollText,
  Search,
  Send,
  Settings,
  ShieldAlert,
  ShieldCheck,
  SlidersHorizontal,
  Sparkles,
  Star,
  Timer,
  Upload,
  User,
  Wrench,
  X,
  XCircle,
  Zap,
} from '@lucide/svelte';
import { siApple, siBluesky, siDiscord, siFacebook, siGoogle, siLinux, siX } from 'simple-icons';
import type { LucideIcon } from '@lucide/svelte';

/**
 * Brands that Simple Icons no longer ships due to trademark removal requests
 * (notably Microsoft, which covers LinkedIn and Windows). Path data is taken
 * from the brand's own publicly published press-kit SVGs.
 */
const SI_GAPS = {
  linkedin: {
    title: 'LinkedIn',
    hex: '0A66C2',
    path: 'M20.452 20.452h-3.554v-5.569c0-1.328-.027-3.037-1.852-3.037-1.853 0-2.136 1.445-2.136 2.939v5.667H9.351V9h3.414v1.561h.046c.477-.9 1.637-1.85 3.37-1.85 3.601 0 4.268 2.37 4.268 5.455v6.286zM5.337 7.433c-1.144 0-2.063-.926-2.063-2.065 0-1.138.92-2.063 2.063-2.063 1.14 0 2.064.925 2.064 2.063 0 1.139-.925 2.065-2.064 2.065zm1.782 13.019H3.555V9h3.564v11.452zM22.225 0H1.771C.792 0 0 .774 0 1.729v20.542C0 23.227.792 24 1.771 24h20.451C23.2 24 24 23.227 24 22.271V1.729C24 .774 23.2 0 22.222 0h.003z',
  },
  windows: {
    title: 'Windows',
    hex: '0078D4',
    path: 'M0 3.449L9.75 2.1v9.451H0m10.949-9.602L24 0v11.4H10.949M0 12.6h9.75v9.451L0 20.699M10.949 12.6H24V24l-12.9-1.801',
  },
} as const;

/** A Lucide-backed icon definition. */
export interface LucideIconDef {
  kind: 'lucide';
  component: LucideIcon;
}

/** A Simple Icons brand definition. */
export interface BrandIconDef {
  kind: 'brand';
  /** Simple Icons title for accessibility. */
  title: string;
  /** SVG path data. */
  path: string;
  /** Brand hex color (without `#`) — used as default if no color override. */
  hex: string;
  /**
   * If true, the brand should default to `currentColor` rather than the
   * brand's official hex. Used for marks where the brand color is too dark
   * for our dark UI (e.g. Apple, X — both look better as white-on-dark).
   */
  monochrome?: boolean;
}

export type IconDef = LucideIconDef | BrandIconDef;

const brand = (
  icon: { title: string; path: string; hex: string },
  opts?: { monochrome?: boolean }
): BrandIconDef => ({
  kind: 'brand',
  title: icon.title,
  path: icon.path,
  hex: icon.hex,
  monochrome: opts?.monochrome,
});

const lucide = (component: LucideIcon): LucideIconDef => ({ kind: 'lucide', component });

/**
 * The icon registry. Keys are stable semantic names — call sites never see
 * the underlying Lucide / Simple Icons identifiers. Add a new entry when
 * adding a new icon to the codebase. Renaming a key requires updating call
 * sites; renaming the *value* (e.g. swapping `Mic` → `MicVocal`) does not.
 */
export const ICONS = {
  // -------------------------------------------------------------------------
  // Status / feedback
  // -------------------------------------------------------------------------
  check: lucide(Check),
  'check-circle': lucide(CheckCircle2),
  close: lucide(X),
  'x-circle': lucide(XCircle),
  warning: lucide(AlertTriangle),
  error: lucide(AlertCircle),
  tip: lucide(Lightbulb),
  info: lucide(Info),
  celebrate: lucide(PartyPopper),
  spinner: lucide(Loader2),
  speed: lucide(Zap),

  // -------------------------------------------------------------------------
  // Navigation / chrome
  // -------------------------------------------------------------------------
  'menu-open': lucide(Menu),
  'menu-close': lucide(X),
  'chevron-down': lucide(ChevronDown),
  'chevron-left': lucide(ChevronLeft),
  'chevron-right': lucide(ChevronRight),
  'arrow-left': lucide(ArrowLeft),
  'arrow-right': lucide(ArrowRight),
  'external-link': lucide(ExternalLink),
  search: lucide(Search),
  globe: lucide(Globe),

  // -------------------------------------------------------------------------
  // App-specific (recording / models)
  // -------------------------------------------------------------------------
  mic: lucide(Mic),
  'mic-off': lucide(MicOff),
  keyboard: lucide(Keyboard),
  /** Rename a hotkey key to a custom display name (Settings > Hotkeys). */
  alias: lucide(NotebookPen),
  monitor: lucide(Monitor),
  cpu: lucide(Cpu),
  'mouse-click': lucide(MousePointerClick),
  tool: lucide(Wrench),

  // -------------------------------------------------------------------------
  // Output / clipboard / files
  // -------------------------------------------------------------------------
  clipboard: lucide(Clipboard),
  copy: lucide(ClipboardCopy),
  paste: lucide(ClipboardPaste),
  save: lucide(Save),
  book: lucide(BookOpen),
  folder: lucide(Folder),
  'file-text': lucide(FileText),
  send: lucide(Send),
  upload: lucide(Upload),
  download: lucide(Download),
  timer: lucide(Timer),

  // -------------------------------------------------------------------------
  // Account / settings
  // -------------------------------------------------------------------------
  user: lucide(User),
  login: lucide(LogIn),
  logout: lucide(LogOut),
  settings: lucide(Settings),
  sliders: lucide(SlidersHorizontal),
  lock: lucide(Lock),
  eye: lucide(Eye),
  'eye-off': lucide(EyeOff),
  mail: lucide(Mail),
  message: lucide(MessageCircle),

  // -------------------------------------------------------------------------
  // Marketing / accent
  // -------------------------------------------------------------------------
  sparkles: lucide(Sparkles),
  rocket: lucide(Rocket),
  star: lucide(Star),
  crown: lucide(Crown),
  megaphone: lucide(Megaphone),
  'usage-stats': lucide(BarChart3),

  // -------------------------------------------------------------------------
  // Update / release notes
  // -------------------------------------------------------------------------
  'update-available': lucide(ArrowUpCircle),
  'release-notes': lucide(ScrollText),
  refresh: lucide(RefreshCw),

  // -------------------------------------------------------------------------
  // Security / shields
  // -------------------------------------------------------------------------
  'shield-warning': lucide(ShieldAlert),
  'shield-ok': lucide(ShieldCheck),

  // -------------------------------------------------------------------------
  // Brands (Simple Icons)
  //   - `monochrome: true` → defaults to `currentColor` instead of brand hex.
  //   - Default for everything else is the official brand color.
  // -------------------------------------------------------------------------
  'brand-discord': brand(siDiscord),
  'brand-linkedin': brand(SI_GAPS.linkedin),
  'brand-bluesky': brand(siBluesky),
  'brand-x': brand(siX, { monochrome: true }),
  'brand-windows': brand(SI_GAPS.windows),
  'brand-linux': brand(siLinux),
  'brand-apple': brand(siApple, { monochrome: true }),
  'brand-google': brand(siGoogle),
  'brand-facebook': brand(siFacebook),
} as const satisfies Record<string, IconDef>;

export type IconName = keyof typeof ICONS;
