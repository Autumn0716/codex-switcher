import { useState, useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { motion, AnimatePresence } from "framer-motion";
import {
  CaretRight,
  Plus,
  Key,
  Globe,
  Cpu,
  Sliders,
  ArrowLeft,
  Link as LinkIcon,
  TextAlignLeft,
  Copy,
  Check,
  Code,
  ArrowClockwise,
} from "@phosphor-icons/react";

/* ─── Brand icon wrappers ─── */

function BrandIconImg({ src, alt, size }: { src: string; alt: string; size: number }) {
  return (
    <span
      className="inline-flex items-center justify-center rounded-md bg-white"
      style={{ width: size, height: size, padding: Math.max(1, Math.round(size * 0.05)) }}
    >
      <img src={src} alt={alt} width={size} height={size} className="block" />
    </span>
  );
}

function ClaudeIcon({ size = 24 }: { size?: number }) {
  return <BrandIconImg src="/claude-logo.svg" alt="Claude" size={size} />;
}

function CodexIcon({ size = 24 }: { size?: number }) {
  return <BrandIconImg src="/codex-logo.svg" alt="Codex" size={size} />;
}

function GeminiIcon({ size = 24 }: { size?: number }) {
  return <BrandIconImg src="/gemini-logo.svg" alt="Gemini" size={size} />;
}

function DeepseekIcon({ size = 24 }: { size?: number }) {
  return <BrandIconImg src="/deepseek-logo.svg" alt="Deepseek" size={size} />;
}

function QwenIcon({ size = 24 }: { size?: number }) {
  return <BrandIconImg src="/qwen-logo.svg" alt="Qwen" size={size} />;
}

function MimoIcon({ size = 24 }: { size?: number }) {
  return <BrandIconImg src="/mimo-logo.svg" alt="MIMO" size={size} />;
}

function ZaiIcon({ size = 24 }: { size?: number }) {
  return <BrandIconImg src="/zai-logo.svg" alt="Zai" size={size} />;
}

function OpenaiIcon({ size = 24 }: { size?: number }) {
  return <BrandIconImg src="/openai-logo.svg" alt="OpenAI" size={size} />;
}

/* ─── Types ─── */

type ProviderBrand = "claude" | "codex" | "gemini";
type ApiFormat = "claude" | "openai(chat)" | "codex";

interface BrandMeta {
  id: ProviderBrand;
  name: string;
  tagline: string;
  accent: string;
  accentBg: string;
  accentBorder: string;
  icon: React.ComponentType<{ size?: number }>;
}

interface VendorMeta {
  id: string;
  name: string;
  icon: React.ComponentType<{ size?: number }>;
  description: string;
  baseUrls: Partial<Record<ApiFormat, string>>;
  brand: ProviderBrand;
  isCodingPlan?: boolean;
  accent?: string;
}

/* ─── Claude Code Profile ─── */

interface ClaudeProfile {
  id: string;
  vendorId?: string;
  // Provider
  name: string;
  notes: string;
  website: string;
  apiKey: string;
  baseUrl: string;
  // Advanced
  apiFormat: string;
  authField: string;
  // Model mapping
  mainModel: string;
  reasoningModel: string;
  haikuModel: string;
  sonnetModel: string;
  opusModel: string;
  // Config toggles
  hideAiSignature: boolean;
  teammatesMode: boolean;
  enableToolSearch: boolean;
  highIntensityThinking: boolean;
  disableAutoUpgrade: boolean;
  // Separate configs
  useSeparateTestConfig: boolean;
  useSeparateProxy: boolean;
  useSeparateBilling: boolean;
  hasCompletedOnboarding: boolean;
  use1MContext: boolean;
}

/* ─── Codex Profile ─── */

interface CodexProfile {
  id: string;
  vendorId?: string;
  // Provider
  name: string;
  notes: string;
  website: string;
  apiKey: string;
  baseUrl: string;
  modelName: string;
  // auth.json
  authMode: string;
  authJson: string;
  // config.toml
  contextWindow: string;
  autoCompactThreshold: string;
  reasoningEffort: string;
  approvalsReviewer: string;
  notifyPath: string;
  configToml: string;
}

interface GenericProfile {
  id: string;
  vendorId?: string;
  name: string;
  apiKey: string;
  baseUrl: string;
  model: string;
}

type ProviderProfile = ClaudeProfile | CodexProfile | GenericProfile;
type ActiveProfileIds = Record<ProviderBrand, string | null>;
type ProviderState = {
  profiles?: unknown[];
  activeId?: string | null;
};
type CodexLoginLaunch = {
  authPath: string;
  previousModifiedAt?: number | null;
  authUrl?: string | null;
};
type SaveOptions = {
  close?: boolean;
};
type CodexUsageWindow = {
  remainingPercent?: number | null;
  usedPercent?: number | null;
  resetAfterSeconds?: number | null;
  resetAt?: number | null;
};
type CodexUsageInfo = {
  accountEmail?: string | null;
  accountPlan?: string | null;
  accountIdHash?: string | null;
  fiveHour?: CodexUsageWindow | null;
  weekly?: CodexUsageWindow | null;
  error?: string | null;
};
type CodexUsageState = {
  loading: boolean;
  profileKey?: string;
  data?: CodexUsageInfo;
};
type ProfileCardInfo = {
  id: string;
  name: string;
  vendorId?: string;
  model?: string;
  usageEnabled?: boolean;
  accountLabel?: string;
  accountPlan?: string;
  usage?: CodexUsageInfo;
  usageLoading?: boolean;
};

const brands: BrandMeta[] = [
  {
    id: "claude",
    name: "Claude Code",
    tagline: "Anthropic Engine",
    accent: "#D97757",
    accentBg: "rgba(217,119,87,0.10)",
    accentBorder: "rgba(217,119,87,0.25)",
    icon: ClaudeIcon,
  },
  {
    id: "codex",
    name: "Codex",
    tagline: "OpenAI Foundation",
    accent: "#412991",
    accentBg: "rgba(65,41,145,0.10)",
    accentBorder: "rgba(65,41,145,0.25)",
    icon: CodexIcon,
  },
  {
    id: "gemini",
    name: "Gemini",
    tagline: "Google DeepMind",
    accent: "#8E75B2",
    accentBg: "rgba(142,117,178,0.10)",
    accentBorder: "rgba(142,117,178,0.25)",
    icon: GeminiIcon,
  },
];

const vendors: VendorMeta[] = [
  {
    id: "zai",
    name: "Zai",
    icon: ZaiIcon,
    description: "Anthropic & OpenAI Completions",
    baseUrls: {
      claude: "https://api.z.ai/api/anthropic",
      "openai(chat)": "https://api.z.ai/api/paas/v4",
    },
    brand: "claude",
    accent: "#6366f1",
  },
  {
    id: "zai-coding",
    name: "Zai",
    icon: ZaiIcon,
    description: "Coding Plan — Anthropic & OpenAI & Responses",
    baseUrls: {
      claude: "https://api.z.ai/api/coding/anthropic",
      "openai(chat)": "https://api.z.ai/api/coding/paas/v4",
    },
    brand: "claude",
    isCodingPlan: true,
    accent: "#6366f1",
  },
  {
    id: "qwen",
    name: "Qwen",
    icon: QwenIcon,
    description: "Anthropic & OpenAI & Responses (sk-xxxxx)",
    baseUrls: {
      claude: "https://dashscope.aliyuncs.com/apps/anthropic",
      codex: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    },
    brand: "codex",
    accent: "#f97316",
  },
  {
    id: "qwen-coding",
    name: "Qwen",
    icon: QwenIcon,
    description: "Coding Plan — Anthropic & OpenAI (sk-sp-xxx)",
    baseUrls: {
      claude: "https://coding.dashscope.aliyuncs.com/apps/anthropic",
      "openai(chat)": "https://coding.dashscope.aliyuncs.com/compatible-mode/v1",
    },
    brand: "claude",
    isCodingPlan: true,
    accent: "#f97316",
  },
  {
    id: "deepseek",
    name: "Deepseek",
    icon: DeepseekIcon,
    description: "Anthropic & OpenAI Completions",
    baseUrls: {
      claude: "https://api.deepseek.com/anthropic",
      "openai(chat)": "https://api.deepseek.com",
    },
    brand: "claude",
    accent: "#3b82f6",
  },
  {
    id: "mimo",
    name: "MIMO",
    icon: MimoIcon,
    description: "Anthropic & OpenAI Completions",
    baseUrls: {
      claude: "https://dashscope.aliyuncs.com/apps/anthropic",
      "openai(chat)": "https://dashscope.aliyuncs.com/compatible-mode/v1",
    },
    brand: "claude",
    accent: "#10b981",
  },
  {
    id: "mimo-coding",
    name: "MIMO",
    icon: MimoIcon,
    description: "Coding Plan — Anthropic & OpenAI Completions",
    baseUrls: {
      claude: "https://token-plan-cn.xiaomimimo.com/anthropic",
      "openai(chat)": "https://token-plan-cn.xiaomimimo.com/v1",
    },
    brand: "claude",
    isCodingPlan: true,
    accent: "#10b981",
  },
];

function getVendorById(vendorId: string | undefined): VendorMeta | undefined {
  if (!vendorId) return undefined;
  return vendors.find((v) => v.id === vendorId);
}

function getVendorsForBrand(_brandId: ProviderBrand): VendorMeta[] {
  return vendors;
}

const defaultClaudeProfiles: ClaudeProfile[] = [
  {
    id: "anthropic",
    name: "Anthropic",
    notes: "",
    website: "https://api.anthropic.com",
    apiKey: "",
    baseUrl: "https://api.anthropic.com",
    apiFormat: "anthropic_messages",
    authField: "ANTHROPIC_AUTH_TOKEN",
    mainModel: "claude-sonnet-4-6-20250514",
    reasoningModel: "claude-sonnet-4-6-20250514",
    haikuModel: "claude-haiku-4-5-20251001",
    sonnetModel: "claude-sonnet-4-6-20250514",
    opusModel: "claude-opus-4-7-20250416",
    hideAiSignature: false,
    teammatesMode: false,
    enableToolSearch: true,
    highIntensityThinking: false,
    disableAutoUpgrade: false,
    useSeparateTestConfig: false,
    useSeparateProxy: false,
    useSeparateBilling: false,
    hasCompletedOnboarding: true,
    use1MContext: false,
  },
];

const defaultCodexProfiles: CodexProfile[] = [
  {
    id: "openai",
    name: "OpenAI",
    notes: "",
    website: "https://api.openai.com",
    apiKey: "",
    baseUrl: "https://api.openai.com",
    modelName: "gpt-5.5",
    authMode: "chatgpt",
    authJson: `{
  "auth_mode": "chatgpt",
  "OPENAI_API_KEY": null,
  "tokens": {
    "id_token": "",
    "access_token": "",
    "refresh_token": "",
    "account_id": ""
  },
  "last_refresh": ""
}`,
    contextWindow: "1000000",
    autoCompactThreshold: "900000",
    reasoningEffort: "xhigh",
    approvalsReviewer: "user",
    notifyPath: "",
    configToml: `[codex]
model = "gpt-5.5"
model_provider = "openai"
model_context_window = 1000000
model_auto_compact_token_limit = 900000
model_reasoning_effort = "xhigh"
approvals_reviewer = "user"`,
  },
];

const defaultGeminiProfiles: GenericProfile[] = [
  {
    id: "google",
    name: "Google",
    apiKey: "",
    baseUrl: "https://generativelanguage.googleapis.com",
    model: "gemini-2.5-pro-preview-05-06",
  },
];

const defaultActiveProfileIds: ActiveProfileIds = {
  claude: defaultClaudeProfiles[0]?.id ?? null,
  codex: defaultCodexProfiles[0]?.id ?? null,
  gemini: defaultGeminiProfiles[0]?.id ?? null,
};

function hasProfileId(profile: unknown): profile is ProviderProfile {
  return (
    typeof profile === "object" &&
    profile !== null &&
    "id" in profile &&
    typeof (profile as { id: unknown }).id === "string"
  );
}

function profilesFromState<T extends ProviderProfile>(state: ProviderState): T[] | null {
  if (!Array.isArray(state.profiles) || state.profiles.length === 0 || !state.profiles.every(hasProfileId)) {
    return null;
  }

  return state.profiles as T[];
}

function activeIdFromState(profiles: ProviderProfile[], fallbackId: string | null, activeId?: string | null) {
  return activeId ?? profiles.find((profile) => (profile as ProviderProfile & { isActive?: boolean }).isActive)?.id ?? fallbackId;
}

/* ─── Small animated components ─── */

function PulseDot({ color }: { color: string }) {
  return (
    <span className="relative flex h-2 w-2">
      <motion.span
        className="absolute inset-0 rounded-full opacity-60"
        style={{ backgroundColor: color }}
        animate={{ scale: [1, 1.8, 1], opacity: [0.6, 0, 0.6] }}
        transition={{ duration: 2.5, repeat: Infinity, ease: "easeInOut" }}
      />
      <span className="relative inline-block h-2 w-2 rounded-full" style={{ backgroundColor: color }} />
    </span>
  );
}

/* ─── Sidebar ─── */

function Sidebar({
  active,
  onSelect,
}: {
  active: ProviderBrand;
  onSelect: (id: ProviderBrand) => void;
}) {
  return (
    <motion.aside
      className="flex w-[260px] flex-col border-r border-white/[0.06] bg-gradient-to-b from-zinc-900/80 via-zinc-900/60 to-zinc-950/80 backdrop-blur-sm"
      initial={false}
      animate={{ x: 0, opacity: 1 }}
      transition={{ duration: 0.4, ease: [0.16, 1, 0.3, 1] }}
    >
      <div className="px-7 pt-8 pb-4">
        <p className="text-[9px] uppercase tracking-[0.3em] text-zinc-600 font-semibold">Providers</p>
      </div>

      <nav className="mt-2 flex-1">
        <AnimatePresence>
          {brands.map((b, i) => {
            const isActive = b.id === active;
            const Icon = b.icon;
            return (
              <motion.div
                key={b.id}
                role="button"
                tabIndex={0}
                onClick={() => onSelect(b.id)}
                onKeyDown={(e) => e.key === "Enter" && onSelect(b.id)}
                className="group relative flex cursor-pointer items-center gap-4 px-7 py-5 transition-colors hover:bg-white/[0.03] focus:outline-none focus-visible:bg-white/[0.03]"
                initial={false}
                animate={{ x: 0, opacity: 1 }}
                transition={{ delay: i * 0.08, duration: 0.35, ease: [0.16, 1, 0.3, 1] }}
              >
                {isActive && (
                  <motion.div
                    layoutId="active-indicator"
                    className="absolute inset-y-0 left-0 w-[2px]"
                    style={{ backgroundColor: b.accent }}
                    transition={{ type: "spring", stiffness: 200, damping: 25 }}
                  />
                )}
                <div
                  className="flex h-12 w-12 items-center justify-center rounded-xl border transition-colors"
                  style={{
                    backgroundColor: isActive ? b.accentBg : "rgba(255,255,255,0.03)",
                    borderColor: isActive ? b.accentBorder : "rgba(255,255,255,0.06)",
                  }}
                >
                  <Icon size={36} />
                </div>
                <div className="flex-1">
                  <p className={`text-[12px] font-medium ${isActive ? "text-zinc-100" : "text-zinc-400 group-hover:text-zinc-300"}`}>
                    {b.name}
                  </p>
                  <p className="text-[9px] text-zinc-600">{b.tagline}</p>
                </div>
                {isActive && <CaretRight size={12} weight="light" className="text-zinc-600" />}
              </motion.div>
            );
          })}
        </AnimatePresence>
      </nav>

      <div className="border-t border-white/[0.04] px-7 py-4">
        <div className="flex items-center gap-2.5 text-zinc-600">
          <PulseDot color="#22c55e" />
          <span className="text-[9px] uppercase tracking-[0.2em] font-medium">System Ready</span>
        </div>
      </div>
    </motion.aside>
  );
}

/* ─── Reusable Input ─── */

function InputField({
  label,
  icon,
  value,
  onChange,
  type = "text",
  placeholder,
  hint,
  ghostValue,
}: {
  label: string;
  icon: React.ReactNode;
  value: string;
  onChange: (v: string) => void;
  type?: "text" | "password";
  placeholder?: string;
  hint?: string;
  ghostValue?: string;
}) {
  const [focused, setFocused] = useState(false);
  const showGhost = ghostValue && !value && !focused;

  return (
    <div className="space-y-1.5">
      <div className="flex items-center gap-2 text-[11px] text-zinc-500">
        {icon}
        <span className="uppercase tracking-[0.15em] font-medium">{label}</span>
      </div>
      <div className="relative">
        <input
          type={type}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onFocus={() => setFocused(true)}
          onBlur={() => setFocused(false)}
          className="w-full rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2 text-[13px] font-mono text-zinc-200 placeholder:text-zinc-700 focus:border-zinc-600 focus:outline-none transition-colors"
          placeholder={placeholder || `Enter ${label.toLowerCase()}`}
        />
        {showGhost && (
          <div className="pointer-events-none absolute inset-0 flex items-center px-3 py-2 text-[13px] font-mono text-zinc-500 truncate">
            {ghostValue}
          </div>
        )}
      </div>
      {hint && <p className="text-[10px] text-zinc-600">{hint}</p>}
    </div>
  );
}

/* ─── Toggle Row ─── */

function ToggleRow({
  label,
  checked,
  onChange,
  accent,
  description,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
  accent?: string;
  description?: string;
}) {
  return (
    <div className="group relative flex items-center justify-between py-1.5">
      <span className="text-[13px] text-zinc-300">{label}</span>
      {description && (
        <div className="pointer-events-none absolute bottom-full left-0 mb-2 w-64 rounded-lg border border-white/[0.08] bg-zinc-900/95 px-3 py-2 text-[11px] text-zinc-400 opacity-0 shadow-lg backdrop-blur-sm transition-opacity group-hover:opacity-100 z-50">
          {description}
          <div className="absolute top-full left-4 -mt-px border-4 border-transparent border-t-zinc-900/95" />
        </div>
      )}
      <button
        onClick={() => onChange(!checked)}
        className="relative h-[22px] w-10 rounded-full transition-colors"
        style={{ backgroundColor: checked && accent ? accent : checked ? "#71717a" : "#3f3f46" }}
      >
        <motion.div
          className="absolute top-[3px] h-4 w-4 rounded-full bg-white shadow-sm"
          animate={{ left: checked ? 20 : 3 }}
          transition={{ type: "spring", stiffness: 300, damping: 25 }}
        />
      </button>
    </div>
  );
}

/* ─── Select Field ─── */

function SelectField({
  label,
  icon,
  value,
  onChange,
  options,
}: {
  label: string;
  icon: React.ReactNode;
  value: string;
  onChange: (v: string) => void;
  options: { value: string; label: string }[];
}) {
  return (
    <div className="space-y-1.5">
      <div className="flex items-center gap-2 text-[11px] text-zinc-500">
        {icon}
        <span className="uppercase tracking-[0.15em] font-medium">{label}</span>
      </div>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="w-full rounded-lg border border-white/[0.06] bg-white/[0.02] px-3 py-2 text-[13px] font-mono text-zinc-200 focus:border-zinc-600 focus:outline-none transition-colors appearance-none cursor-pointer"
      >
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
    </div>
  );
}

/* ─── Vendor Selector ─── */

function VendorSelector({
  brandId,
  selectedVendorId,
  onSelect,
  accent,
}: {
  brandId: ProviderBrand;
  selectedVendorId: string | undefined;
  onSelect: (vendor: VendorMeta | null) => void;
  accent: string;
}) {
  const brandVendors = getVendorsForBrand(brandId);

  return (
    <div className="rounded-lg border border-white/[0.06] bg-white/[0.015] p-5 space-y-4">
      <div className="flex items-center gap-2 mb-2">
        <Globe size={14} weight="light" style={{ color: accent }} />
        <h3 className="text-[10px] uppercase tracking-[0.2em] font-semibold" style={{ color: accent }}>
          Vendor
        </h3>
      </div>

      <div className="grid grid-cols-2 sm:grid-cols-3 gap-2.5">
        {brandVendors.map((v) => {
          const VendorIcon = v.icon;
          const isSelected = selectedVendorId === v.id;
          return (
            <motion.button
              key={v.id}
              type="button"
              onClick={() => onSelect(isSelected ? null : v)}
              className={`group relative flex items-center gap-3 rounded-xl border px-3 py-3 text-left transition-all ${
                isSelected
                  ? "bg-white/[0.06]"
                  : "bg-white/[0.01] hover:bg-white/[0.03]"
              } ${v.isCodingPlan ? "border-amber-500/20" : "border-white/[0.06]"}`}
              style={{
                borderColor: isSelected ? `${accent}66` : undefined,
              }}
              whileHover={{ y: -1 }}
              whileTap={{ scale: 0.98 }}
            >
              <div
                className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg"
                style={{ backgroundColor: isSelected ? `${accent}18` : "rgba(255,255,255,0.04)" }}
              >
                <VendorIcon size={28} />
              </div>
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-1.5">
                  <span className={`text-[12px] font-medium truncate ${isSelected ? "text-zinc-100" : "text-zinc-300"}`}>
                    {v.name}
                  </span>
                  {v.isCodingPlan && (
                    <span className="shrink-0 rounded-full bg-amber-500/15 px-1.5 py-0.5 text-[7px] uppercase tracking-[0.1em] font-semibold text-amber-400">
                      CP
                    </span>
                  )}
                </div>
                <p className="text-[9px] text-zinc-600 truncate mt-0.5">{v.description}</p>
              </div>
              {isSelected && (
                <Check size={14} weight="bold" style={{ color: accent }} className="shrink-0" />
              )}
            </motion.button>
          );
        })}

        {/* Custom option */}
        <motion.button
          type="button"
          onClick={() => onSelect(null)}
          className={`flex items-center gap-3 rounded-xl border border-dashed px-3 py-3 text-left transition-all ${
            !selectedVendorId
              ? "bg-white/[0.06] border-white/[0.15]"
              : "bg-white/[0.01] border-white/[0.06] hover:bg-white/[0.03]"
          }`}
          whileHover={{ y: -1 }}
          whileTap={{ scale: 0.98 }}
        >
          <div
            className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg"
            style={{ backgroundColor: !selectedVendorId ? `${accent}18` : "rgba(255,255,255,0.04)" }}
          >
            <Sliders size={28} weight="light" />
          </div>
          <div className="flex-1 min-w-0">
            <span className={`text-[12px] font-medium ${!selectedVendorId ? "text-zinc-100" : "text-zinc-300"}`}>
              Custom
            </span>
            <p className="text-[9px] text-zinc-600 mt-0.5">Manual configuration</p>
          </div>
          {!selectedVendorId && (
            <Check size={14} weight="bold" style={{ color: accent }} className="shrink-0" />
          )}
        </motion.button>
      </div>
    </div>
  );
}

/* ─── Vendor Info Card ─── */

function VendorInfoCard({
  vendor,
  brandAccent,
  brandIcon: BrandIcon,
  brandName,
}: {
  vendor: VendorMeta | undefined;
  brandAccent: string;
  brandIcon: React.ComponentType<{ size?: number }>;
  brandName: string;
}) {
  const accent = vendor?.accent ?? brandAccent;
  const Icon = vendor?.icon ?? BrandIcon;
  const name = vendor?.name ?? brandName;
  const description = vendor?.description ?? "Custom configuration";

  return (
    <motion.div
      className="relative overflow-hidden rounded-xl border"
      style={{
        borderColor: `${accent}30`,
        background: `linear-gradient(135deg, ${accent}08 0%, transparent 60%)`,
      }}
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.3 }}
    >
      {/* Subtle glow */}
      <div
        className="pointer-events-none absolute -top-16 -right-16 h-32 w-32 rounded-full opacity-[0.06]"
        style={{ background: `radial-gradient(circle, ${accent} 0%, transparent 70%)` }}
      />
      <div className="relative flex items-center gap-4 px-5 py-4">
        <div
          className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl"
          style={{ backgroundColor: `${accent}15`, border: `1px solid ${accent}25` }}
        >
          <Icon size={26} />
        </div>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="text-[14px] font-semibold text-zinc-100">{name}</span>
            <span
              className="rounded-full px-2 py-0.5 text-[8px] uppercase tracking-[0.15em] font-bold"
              style={{ color: accent, backgroundColor: `${accent}18`, border: `1px solid ${accent}28` }}
            >
              Active
            </span>
          </div>
          <p className="text-[11px] text-zinc-500 mt-0.5 truncate">{description}</p>
        </div>
      </div>
    </motion.div>
  );
}

/* ─── Model Selector ─── */

function ModelSelector({
  brand,
  baseUrl,
  apiKey,
  vendorId,
  currentValue,
  onSelect,
  accent,
}: {
  brand: ProviderBrand;
  baseUrl: string;
  apiKey: string;
  vendorId?: string;
  currentValue: string;
  onSelect: (model: string) => void;
  accent: string;
}) {
  const [models, setModels] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const [open, setOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleFetch = async () => {
    if (!baseUrl || !apiKey) return;
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<{ models: { id: string }[]; error?: string }>(
        "fetch_available_models",
        { brand, baseUrl, apiKey, vendorId: vendorId || "" }
      );
      if (result.error) {
        setError(result.error);
        setModels([]);
      } else {
        setModels(result.models.map((m) => m.id));
        setOpen(true);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-1.5">
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={handleFetch}
          disabled={loading || !baseUrl || !apiKey}
          className="flex items-center gap-1.5 rounded-md border border-white/[0.08] bg-white/[0.03] px-2.5 py-1 text-[10px] uppercase tracking-[0.12em] text-zinc-400 transition-colors hover:border-white/[0.16] hover:text-zinc-200 disabled:opacity-30 disabled:cursor-not-allowed"
        >
          {loading ? (
            <ArrowClockwise size={10} className="animate-spin" />
          ) : (
            <ArrowClockwise size={10} weight="light" />
          )}
          {loading ? "Fetching..." : "Fetch Models"}
        </button>
        {error && <span className="text-[10px] text-rose-400 truncate max-w-[200px]">{error}</span>}
      </div>
      {open && models.length > 0 && (
        <div className="max-h-48 overflow-y-auto rounded-lg border border-white/[0.06] bg-zinc-900/80 backdrop-blur-sm">
          {models.map((m) => (
            <button
              key={m}
              type="button"
              onClick={() => { onSelect(m); setOpen(false); }}
              className={`flex w-full items-center px-3 py-1.5 text-left text-[12px] font-mono transition-colors hover:bg-white/[0.04] ${
                m === currentValue ? "text-zinc-100 bg-white/[0.06]" : "text-zinc-400"
              }`}
            >
              {m}
              {m === currentValue && <Check size={10} weight="bold" className="ml-auto" style={{ color: accent }} />}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

/* ─── Generate config JSON string ─── */

function append1M(model: string, use1M: boolean): string {
  if (!use1M || !model || model.endsWith("[1m]")) return model;
  return `${model}[1m]`;
}

function generateConfigJson(p: ClaudeProfile): string {
  const env: Record<string, string> = {};

  if (p.teammatesMode) env.CLUADE_CODE_EXPERIMENTAL_AGENT_TEAMS = "1";
  if (p.enableToolSearch) env.ENABLE_TOOL_SEARCH = "true";
  if (p.opusModel) env.ANTHROPIC_DEFAULT_OPUS_MODEL = append1M(p.opusModel, p.use1MContext);
  if (p.baseUrl) env.ANTHROPIC_BASE_URL = p.baseUrl;
  if (p.apiKey) env.ANTHROPIC_AUTH_TOKEN = p.apiKey;
  if (p.mainModel) env.ANTHROPIC_MODEL = append1M(p.mainModel, p.use1MContext);
  if (p.reasoningModel) env.ANTHROPIC_REASONING_MODEL = append1M(p.reasoningModel, p.use1MContext);
  if (p.haikuModel) env.ANTHROPIC_DEFAULT_HAIKU_MODEL = append1M(p.haikuModel, p.use1MContext);
  if (p.sonnetModel) env.ANTHROPIC_DEFAULT_SONNET_MODEL = append1M(p.sonnetModel, p.use1MContext);
  if (p.hideAiSignature) env.HIDE_AI_SIGNATURE = "true";
  if (p.highIntensityThinking) env.HIGH_INTENSITY_THINKING = "true";
  if (p.disableAutoUpgrade) env.DISABLE_AUTO_UPGRADE = "true";

  return JSON.stringify({ env }, null, 2);
}

/* ─── Claude Advanced Panel ─── */

function ClaudeAdvancedPanel({ accent }: { accent: string }) {
  const [settings, setSettings] = useState<Record<string, unknown>>({});
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<string>("read_claude_settings")
      .then((json) => {
        setSettings(JSON.parse(json));
        setLoading(false);
      })
      .catch((e) => {
        setError(String(e));
        setLoading(false);
      });
  }, []);

  const env = (settings.env as Record<string, string>) || {};

  const updateSetting = (key: string, value: unknown) => {
    setSettings((prev) => ({ ...prev, [key]: value }));
  };

  const updateEnv = (key: string, value: string) => {
    setSettings((prev) => ({
      ...prev,
      env: { ...((prev.env as Record<string, string>) || {}), [key]: value },
    }));
  };

  const removeEnv = (key: string) => {
    setSettings((prev) => {
      const next = { ...((prev.env as Record<string, string>) || {}) };
      delete next[key];
      return { ...prev, env: next };
    });
  };

  const handleSave = async () => {
    setSaving(true);
    setError(null);
    try {
      await invoke("write_claude_settings", { content: JSON.stringify(settings, null, 2) });
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  if (loading) return <p className="text-[11px] text-zinc-600 py-4">Loading settings...</p>;

  return (
    <div className="space-y-5">
      {error && <p className="text-[11px] text-rose-400">{error}</p>}

      {/* Thinking */}
      <div className="rounded-lg border border-white/[0.06] bg-white/[0.015] p-5 space-y-3">
        <div className="flex items-center gap-2 mb-2">
          <Cpu size={14} weight="light" style={{ color: accent }} />
          <h3 className="text-[10px] uppercase tracking-[0.2em] font-semibold" style={{ color: accent }}>Thinking</h3>
        </div>
        <ToggleRow
          label="Always Enable Extended Thinking"
          checked={settings.alwaysThinkingEnabled === true}
          onChange={(v) => updateSetting("alwaysThinkingEnabled", v)}
          accent={accent}
          description="Enable extended thinking mode by default for all conversations"
        />
        <SelectField
          label="Effort Level"
          icon={<Sliders size={12} weight="light" />}
          value={(settings.effortLevel as string) || "high"}
          onChange={(v) => updateSetting("effortLevel", v)}
          options={[
            { value: "low", label: "Low" },
            { value: "medium", label: "Medium" },
            { value: "high", label: "High" },
            { value: "xhigh", label: "XHigh" },
            { value: "max", label: "Max" },
          ]}
        />
        <ToggleRow
          label="Show Thinking Summaries"
          checked={settings.showThinkingSummaries !== false}
          onChange={(v) => updateSetting("showThinkingSummaries", v)}
          accent={accent}
          description="Display summaries of Claude's thinking process in interactive mode"
        />
      </div>

      {/* Performance */}
      <div className="rounded-lg border border-white/[0.06] bg-white/[0.015] p-5 space-y-3">
        <div className="flex items-center gap-2 mb-2">
          <Sliders size={14} weight="light" style={{ color: accent }} />
          <h3 className="text-[10px] uppercase tracking-[0.2em] font-semibold" style={{ color: accent }}>Performance</h3>
        </div>
        <InputField
          label="Auto-compact %"
          icon={<Cpu size={12} weight="light" />}
          value={env.CLAUDE_AUTOCOMPACT_PCT_OVERRIDE || ""}
          onChange={(v) => v ? updateEnv("CLAUDE_AUTOCOMPACT_PCT_OVERRIDE", v) : removeEnv("CLAUDE_AUTOCOMPACT_PCT_OVERRIDE")}
          placeholder="95"
          hint="Context % at which compaction triggers"
        />
        <InputField
          label="Compact Window (tokens)"
          icon={<Cpu size={12} weight="light" />}
          value={env.CLAUDE_CODE_AUTO_COMPACT_WINDOW || ""}
          onChange={(v) => v ? updateEnv("CLAUDE_CODE_AUTO_COMPACT_WINDOW", v) : removeEnv("CLAUDE_CODE_AUTO_COMPACT_WINDOW")}
          placeholder="200000"
        />
        <InputField
          label="Max Output Tokens"
          icon={<Cpu size={12} weight="light" />}
          value={env.CLAUDE_CODE_MAX_OUTPUT_TOKENS || ""}
          onChange={(v) => v ? updateEnv("CLAUDE_CODE_MAX_OUTPUT_TOKENS", v) : removeEnv("CLAUDE_CODE_MAX_OUTPUT_TOKENS")}
          placeholder="64000"
        />
        <InputField
          label="API Timeout (ms)"
          icon={<Cpu size={12} weight="light" />}
          value={env.API_TIMEOUT_MS || ""}
          onChange={(v) => v ? updateEnv("API_TIMEOUT_MS", v) : removeEnv("API_TIMEOUT_MS")}
          placeholder="600000"
        />
        <ToggleRow
          label="Disable Telemetry & Auto-update"
          checked={env.CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC === "1"}
          onChange={(v) => v ? updateEnv("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1") : removeEnv("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC")}
          accent={accent}
          description="Disable telemetry, auto-updater, and error reporting traffic"
        />
      </div>

      {/* Model Overrides */}
      <div className="rounded-lg border border-white/[0.06] bg-white/[0.015] p-5 space-y-3">
        <div className="flex items-center gap-2 mb-2">
          <Code size={14} weight="light" style={{ color: accent }} />
          <h3 className="text-[10px] uppercase tracking-[0.2em] font-semibold" style={{ color: accent }}>Model Overrides</h3>
        </div>
        <InputField
          label="Default Model"
          icon={<Cpu size={12} weight="light" />}
          value={(settings.model as string) || ""}
          onChange={(v) => updateSetting("model", v)}
          placeholder="claude-sonnet-4-6"
        />
        <InputField
          label="Sonnet Override"
          icon={<Cpu size={12} weight="light" />}
          value={env.ANTHROPIC_DEFAULT_SONNET_MODEL || ""}
          onChange={(v) => v ? updateEnv("ANTHROPIC_DEFAULT_SONNET_MODEL", v) : removeEnv("ANTHROPIC_DEFAULT_SONNET_MODEL")}
        />
        <InputField
          label="Opus Override"
          icon={<Cpu size={12} weight="light" />}
          value={env.ANTHROPIC_DEFAULT_OPUS_MODEL || ""}
          onChange={(v) => v ? updateEnv("ANTHROPIC_DEFAULT_OPUS_MODEL", v) : removeEnv("ANTHROPIC_DEFAULT_OPUS_MODEL")}
        />
        <InputField
          label="Haiku Override"
          icon={<Cpu size={12} weight="light" />}
          value={env.ANTHROPIC_DEFAULT_HAIKU_MODEL || ""}
          onChange={(v) => v ? updateEnv("ANTHROPIC_DEFAULT_HAIKU_MODEL", v) : removeEnv("ANTHROPIC_DEFAULT_HAIKU_MODEL")}
        />
      </div>

      <div className="flex justify-end">
        <button
          onClick={handleSave}
          disabled={saving}
          className="rounded-full bg-zinc-100 px-5 py-1.5 text-[10px] uppercase tracking-widest font-semibold text-zinc-900 transition-transform hover:bg-white active:scale-[0.97] disabled:opacity-50"
        >
          {saving ? "Saving..." : "Save Settings"}
        </button>
      </div>
    </div>
  );
}

/* ─── Codex Advanced Panel ─── */

function CodexAdvancedPanel({ accent }: { accent: string }) {
  const [config, setConfig] = useState<Record<string, unknown>>({});
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<string>("read_codex_config")
      .then((json) => {
        setConfig(JSON.parse(json));
        setLoading(false);
      })
      .catch((e) => {
        setError(String(e));
        setLoading(false);
      });
  }, []);

  const update = (key: string, value: unknown) => {
    setConfig((prev) => ({ ...prev, [key]: value }));
  };

  const updateNested = (section: string, key: string, value: unknown) => {
    setConfig((prev) => ({
      ...prev,
      [section]: { ...((prev[section] as Record<string, unknown>) || {}), [key]: value },
    }));
  };

  const getNested = (section: string, key: string): unknown => {
    return ((config[section] as Record<string, unknown>) || {})[key];
  };

  const handleSave = async () => {
    setSaving(true);
    setError(null);
    try {
      await invoke("write_codex_config", { jsonContent: JSON.stringify(config, null, 2) });
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  if (loading) return <p className="text-[11px] text-zinc-600 py-4">Loading config...</p>;

  return (
    <div className="space-y-5">
      {error && <p className="text-[11px] text-rose-400">{error}</p>}

      {/* Reasoning */}
      <div className="rounded-lg border border-white/[0.06] bg-white/[0.015] p-5 space-y-3">
        <div className="flex items-center gap-2 mb-2">
          <Cpu size={14} weight="light" style={{ color: accent }} />
          <h3 className="text-[10px] uppercase tracking-[0.2em] font-semibold" style={{ color: accent }}>Reasoning</h3>
        </div>
        <SelectField
          label="Reasoning Effort"
          icon={<Sliders size={12} weight="light" />}
          value={(config.model_reasoning_effort as string) || "medium"}
          onChange={(v) => update("model_reasoning_effort", v)}
          options={[
            { value: "minimal", label: "Minimal" },
            { value: "low", label: "Low" },
            { value: "medium", label: "Medium" },
            { value: "high", label: "High" },
            { value: "xhigh", label: "XHigh" },
          ]}
        />
        <SelectField
          label="Reasoning Summary"
          icon={<Sliders size={12} weight="light" />}
          value={(config.model_reasoning_summary as string) || "auto"}
          onChange={(v) => update("model_reasoning_summary", v)}
          options={[
            { value: "auto", label: "Auto" },
            { value: "concise", label: "Concise" },
            { value: "detailed", label: "Detailed" },
            { value: "none", label: "None" },
          ]}
        />
        <SelectField
          label="Verbosity"
          icon={<Sliders size={12} weight="light" />}
          value={(config.model_verbosity as string) || "medium"}
          onChange={(v) => update("model_verbosity", v)}
          options={[
            { value: "low", label: "Low" },
            { value: "medium", label: "Medium" },
            { value: "high", label: "High" },
          ]}
        />
        <SelectField
          label="Service Tier"
          icon={<Sliders size={12} weight="light" />}
          value={(config.service_tier as string) || "fast"}
          onChange={(v) => update("service_tier", v)}
          options={[
            { value: "fast", label: "Fast" },
            { value: "flex", label: "Flex" },
          ]}
        />
      </div>

      {/* Context & Compact */}
      <div className="rounded-lg border border-white/[0.06] bg-white/[0.015] p-5 space-y-3">
        <div className="flex items-center gap-2 mb-2">
          <Sliders size={14} weight="light" style={{ color: accent }} />
          <h3 className="text-[10px] uppercase tracking-[0.2em] font-semibold" style={{ color: accent }}>Context & Compact</h3>
        </div>
        <InputField
          label="Context Window"
          icon={<Cpu size={12} weight="light" />}
          value={String(config.model_context_window || "")}
          onChange={(v) => update("model_context_window", v ? Number(v) : undefined)}
          placeholder="128000"
        />
        <InputField
          label="Auto-compact Threshold"
          icon={<Cpu size={12} weight="light" />}
          value={String(config.model_auto_compact_token_limit || "")}
          onChange={(v) => update("model_auto_compact_token_limit", v ? Number(v) : undefined)}
          placeholder="64000"
        />
        <InputField
          label="Tool Output Token Limit"
          icon={<Cpu size={12} weight="light" />}
          value={String(config.tool_output_token_limit || "")}
          onChange={(v) => update("tool_output_token_limit", v ? Number(v) : undefined)}
          placeholder="12000"
        />
      </div>

      {/* Sandbox & Approval */}
      <div className="rounded-lg border border-white/[0.06] bg-white/[0.015] p-5 space-y-3">
        <div className="flex items-center gap-2 mb-2">
          <Code size={14} weight="light" style={{ color: accent }} />
          <h3 className="text-[10px] uppercase tracking-[0.2em] font-semibold" style={{ color: accent }}>Sandbox & Approval</h3>
        </div>
        <SelectField
          label="Sandbox Mode"
          icon={<Sliders size={12} weight="light" />}
          value={(config.sandbox_mode as string) || "read-only"}
          onChange={(v) => update("sandbox_mode", v)}
          options={[
            { value: "read-only", label: "Read Only" },
            { value: "workspace-write", label: "Workspace Write" },
            { value: "danger-full-access", label: "Full Access (DANGER)" },
          ]}
        />
        <SelectField
          label="Approval Policy"
          icon={<Sliders size={12} weight="light" />}
          value={(config.approval_policy as string) || "on-request"}
          onChange={(v) => update("approval_policy", v)}
          options={[
            { value: "on-request", label: "On Request" },
            { value: "untrusted", label: "Untrusted" },
            { value: "never", label: "Never (YOLO)" },
          ]}
        />
        <SelectField
          label="Approvals Reviewer"
          icon={<Sliders size={12} weight="light" />}
          value={(config.approvals_reviewer as string) || "user"}
          onChange={(v) => update("approvals_reviewer", v)}
          options={[
            { value: "user", label: "User" },
            { value: "auto_review", label: "Auto Review" },
          ]}
        />
      </div>

      {/* Misc */}
      <div className="rounded-lg border border-white/[0.06] bg-white/[0.015] p-5 space-y-3">
        <div className="flex items-center gap-2 mb-2">
          <Sliders size={14} weight="light" style={{ color: accent }} />
          <h3 className="text-[10px] uppercase tracking-[0.2em] font-semibold" style={{ color: accent }}>Misc</h3>
        </div>
        <SelectField
          label="Web Search"
          icon={<Globe size={12} weight="light" />}
          value={(config.web_search as string) || "cached"}
          onChange={(v) => update("web_search", v)}
          options={[
            { value: "disabled", label: "Disabled" },
            { value: "cached", label: "Cached" },
            { value: "live", label: "Live" },
          ]}
        />
        <SelectField
          label="Personality"
          icon={<Cpu size={12} weight="light" />}
          value={(config.personality as string) || "pragmatic"}
          onChange={(v) => update("personality", v)}
          options={[
            { value: "none", label: "None" },
            { value: "friendly", label: "Friendly" },
            { value: "pragmatic", label: "Pragmatic" },
          ]}
        />
        <SelectField
          label="File Opener"
          icon={<Code size={12} weight="light" />}
          value={(config.file_opener as string) || "vscode"}
          onChange={(v) => update("file_opener", v)}
          options={[
            { value: "vscode", label: "VS Code" },
            { value: "cursor", label: "Cursor" },
            { value: "windsurf", label: "Windsurf" },
            { value: "none", label: "None" },
          ]}
        />
      </div>

      {/* Features */}
      <div className="rounded-lg border border-white/[0.06] bg-white/[0.015] p-5 space-y-3">
        <div className="flex items-center gap-2 mb-2">
          <Code size={14} weight="light" style={{ color: accent }} />
          <h3 className="text-[10px] uppercase tracking-[0.2em] font-semibold" style={{ color: accent }}>Features</h3>
        </div>
        <div className="flex items-center justify-between py-1">
          <div>
            <span className="text-[12px] text-zinc-400">Memories</span>
            <p className="text-[10px] text-zinc-600">Carry useful context from earlier threads into future work</p>
          </div>
          <button
            onClick={() => updateNested("features", "memories", !getNested("features", "memories"))}
            className={`relative h-5 w-9 rounded-full transition-colors ${getNested("features", "memories") ? "bg-zinc-500" : "bg-zinc-700"}`}
          >
            <motion.div
              className="absolute top-0.5 h-4 w-4 rounded-full bg-white"
              animate={{ left: getNested("features", "memories") ? 19 : 3 }}
              transition={{ type: "spring", stiffness: 300, damping: 25 }}
            />
          </button>
        </div>
        <div className="flex items-center justify-between py-1">
          <div>
            <span className="text-[12px] text-zinc-400">Undo</span>
            <p className="text-[10px] text-zinc-600">Enable undo via per-turn git ghost snapshots</p>
          </div>
          <button
            onClick={() => updateNested("features", "undo", !getNested("features", "undo"))}
            className={`relative h-5 w-9 rounded-full transition-colors ${getNested("features", "undo") ? "bg-zinc-500" : "bg-zinc-700"}`}
          >
            <motion.div
              className="absolute top-0.5 h-4 w-4 rounded-full bg-white"
              animate={{ left: getNested("features", "undo") ? 19 : 3 }}
              transition={{ type: "spring", stiffness: 300, damping: 25 }}
            />
          </button>
        </div>
      </div>

      {/* Analytics & Debug */}
      <div className="rounded-lg border border-white/[0.06] bg-white/[0.015] p-5 space-y-3">
        <div className="flex items-center gap-2 mb-2">
          <Sliders size={14} weight="light" style={{ color: accent }} />
          <h3 className="text-[10px] uppercase tracking-[0.2em] font-semibold" style={{ color: accent }}>Analytics & Debug</h3>
        </div>
        <div className="flex items-center justify-between py-1">
          <div>
            <span className="text-[12px] text-zinc-400">Analytics</span>
            <p className="text-[10px] text-zinc-600">Anonymous usage & health metrics sent to OpenAI</p>
          </div>
          <button
            onClick={() => updateNested("analytics", "enabled", !getNested("analytics", "enabled"))}
            className={`relative h-5 w-9 rounded-full transition-colors ${getNested("analytics", "enabled") !== false ? "bg-zinc-500" : "bg-zinc-700"}`}
          >
            <motion.div
              className="absolute top-0.5 h-4 w-4 rounded-full bg-white"
              animate={{ left: getNested("analytics", "enabled") !== false ? 19 : 3 }}
              transition={{ type: "spring", stiffness: 300, damping: 25 }}
            />
          </button>
        </div>
        <ToggleRow
          label="Hide Agent Reasoning"
          checked={config.hide_agent_reasoning === true}
          onChange={(v) => update("hide_agent_reasoning", v)}
          accent={accent}
          description="Suppress reasoning output to reduce noise in CI logs"
        />
        <ToggleRow
          label="Show Raw Agent Reasoning"
          checked={config.show_raw_agent_reasoning === true}
          onChange={(v) => update("show_raw_agent_reasoning", v)}
          accent={accent}
          description="Display raw reasoning content when the model produces output"
        />
        <ToggleRow
          label="Supports Reasoning Summaries"
          checked={config.model_supports_reasoning_summaries !== false}
          onChange={(v) => update("model_supports_reasoning_summaries", v)}
          accent={accent}
          description="Force Codex to send or not send reasoning metadata"
        />
        <p className="text-[10px] text-zinc-600 -mt-2 ml-1">Force Codex to send or not send reasoning metadata</p>
      </div>

      <div className="flex justify-end">
        <button
          onClick={handleSave}
          disabled={saving}
          className="rounded-full bg-zinc-100 px-5 py-1.5 text-[10px] uppercase tracking-widest font-semibold text-zinc-900 transition-transform hover:bg-white active:scale-[0.97] disabled:opacity-50"
        >
          {saving ? "Saving..." : "Save Config"}
        </button>
      </div>
    </div>
  );
}

/* ─── Claude Config Editor ─── */

function ClaudeConfigEditor({
  profile,
  onSave,
  onBack,
  accent,
  mode = "page",
  saveLabel = "Save",
}: {
  profile: ClaudeProfile;
  onSave: (p: ClaudeProfile, options?: SaveOptions) => void;
  onBack: () => void;
  accent: string;
  mode?: "page" | "dialog";
  saveLabel?: string;
}) {
  const [local, setLocal] = useState(profile);
  const [copied, setCopied] = useState(false);
  const [showJson, setShowJson] = useState(false);

  const update = useCallback((key: string, value: string | boolean) => {
    setLocal((prev) => ({ ...prev, [key]: value }));
  }, []);

  const handleVendorSelect = useCallback((vendor: VendorMeta | null) => {
    setLocal((prev) => ({
      ...prev,
      vendorId: vendor?.id,
      ...(vendor?.baseUrls.claude ? { baseUrl: vendor.baseUrls.claude } : {}),
    }));
  }, []);

  const configJson = generateConfigJson(local);

  const handleCopy = () => {
    navigator.clipboard.writeText(configJson);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const apiFormatOptions = [
    { value: "anthropic_messages", label: "Anthropic Messages (native)" },
    { value: "openai_compatible", label: "OpenAI Compatible" },
  ];

  const authFieldOptions = [
    { value: "ANTHROPIC_AUTH_TOKEN", label: "ANTHROPIC_AUTH_TOKEN (default)" },
    { value: "ANTHROPIC_API_KEY", label: "ANTHROPIC_API_KEY" },
  ];

  return (
    <motion.main
      className={`${mode === "dialog" ? "flex h-full min-h-0 flex-col" : "flex-1"} bg-gradient-to-br from-zinc-900/40 via-zinc-950 to-zinc-950`}
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      transition={{ duration: 0.3 }}
    >
      <header className="flex h-14 items-center justify-between border-b border-white/[0.04] px-8">
        <div className="flex items-center gap-3">
          {mode === "page" && (
            <button
              onClick={onBack}
              className="flex h-8 w-8 items-center justify-center rounded-md border border-white/[0.06] text-zinc-400 hover:text-zinc-200 hover:bg-white/[0.04] transition-colors"
            >
              <ArrowLeft size={14} weight="light" />
            </button>
          )}
          <div className="flex items-center gap-2">
            <ClaudeIcon size={16} />
            <span className="text-[13px] text-zinc-300 font-medium">Claude Code</span>
            <CaretRight size={10} weight="light" className="text-zinc-700" />
            <span className="text-[13px] text-zinc-500">{profile.name}</span>
          </div>
        </div>
        <div className="flex items-center gap-2">
          {mode === "dialog" && (
            <button
              onClick={onBack}
              className="rounded-full border border-white/[0.08] px-4 py-1.5 text-[10px] uppercase tracking-widest font-semibold text-zinc-400 transition-colors hover:border-white/[0.16] hover:text-zinc-200"
            >
              Cancel
            </button>
          )}
          <button
            onClick={() => onSave(local, { close: true })}
            className="rounded-full bg-zinc-100 px-5 py-1.5 text-[10px] uppercase tracking-widest font-semibold text-zinc-900 transition-transform hover:bg-white active:scale-[0.97]"
          >
            {saveLabel}
          </button>
        </div>
      </header>

      <section className="min-h-0 flex-1 overflow-y-auto px-6 py-5 lg:px-8">
        <div className="mx-auto max-w-5xl space-y-4">
          {/* Row 1: Vendor Selector (full width) */}
          <VendorSelector
            brandId="claude"
            selectedVendorId={local.vendorId}
            onSelect={handleVendorSelect}
            accent={accent}
          />

          {/* Vendor Info Card */}
          <VendorInfoCard
            vendor={getVendorById(local.vendorId)}
            brandAccent={accent}
            brandIcon={ClaudeIcon}
            brandName="Claude Code"
          />

          {/* Row 2: Provider (left) + Models (right) */}
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
            {/* Provider Info */}
            <div className="rounded-lg border border-white/[0.06] bg-white/[0.015] p-4">
              <div className="flex items-center gap-2 mb-3">
                <Globe size={12} weight="light" style={{ color: accent }} />
                <h3 className="text-[10px] uppercase tracking-[0.2em] font-semibold" style={{ color: accent }}>Provider</h3>
              </div>
              <div className="space-y-3">
                <InputField label="Name" icon={<Cpu size={10} weight="light" />} value={local.name} onChange={(v) => update("name", v)} />
                <InputField label="API Key" icon={<Key size={10} weight="light" />} value={local.apiKey} onChange={(v) => update("apiKey", v)} type="password" />
                <InputField label="Base URL" icon={<Globe size={10} weight="light" />} value={local.baseUrl} onChange={(v) => update("baseUrl", v)} />
                <InputField label="Website" icon={<LinkIcon size={10} weight="light" />} value={local.website} onChange={(v) => update("website", v)} placeholder="https://..." />
                <InputField label="Notes" icon={<TextAlignLeft size={10} weight="light" />} value={local.notes} onChange={(v) => update("notes", v)} placeholder="Optional remarks" />
              </div>
            </div>

            {/* Models */}
            <div className="rounded-lg border border-white/[0.06] bg-white/[0.015] p-4">
              <div className="flex items-center gap-2 mb-3">
                <Cpu size={12} weight="light" style={{ color: accent }} />
                <h3 className="text-[10px] uppercase tracking-[0.2em] font-semibold" style={{ color: accent }}>Models</h3>
              </div>
              <ModelSelector brand="claude" baseUrl={local.baseUrl} apiKey={local.apiKey} vendorId={local.vendorId} currentValue={local.mainModel} onSelect={(m) => update("mainModel", m)} accent={accent} />
              <div className="grid grid-cols-2 gap-3 mt-3">
                <InputField label="Main" icon={<Cpu size={10} weight="light" />} value={local.mainModel} onChange={(v) => update("mainModel", v)} />
                <InputField label="Reasoning" icon={<Cpu size={10} weight="light" />} value={local.reasoningModel} onChange={(v) => update("reasoningModel", v)} />
                <InputField label="Haiku" icon={<Cpu size={10} weight="light" />} value={local.haikuModel} onChange={(v) => update("haikuModel", v)} />
                <InputField label="Sonnet" icon={<Cpu size={10} weight="light" />} value={local.sonnetModel} onChange={(v) => update("sonnetModel", v)} />
                <InputField label="Opus" icon={<Cpu size={10} weight="light" />} value={local.opusModel} onChange={(v) => update("opusModel", v)} />
              </div>
            </div>
          </div>

          {/* Row 3: Options (left) + Config JSON (right) */}
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
            {/* Options */}
            <div className="rounded-lg border border-white/[0.06] bg-white/[0.015] p-4">
              <div className="flex items-center gap-2 mb-3">
                <Sliders size={12} weight="light" style={{ color: accent }} />
                <h3 className="text-[10px] uppercase tracking-[0.2em] font-semibold" style={{ color: accent }}>Options</h3>
              </div>
              <div className="grid grid-cols-2 gap-3">
                <SelectField label="API Format" icon={<Code size={10} weight="light" />} value={local.apiFormat} onChange={(v) => update("apiFormat", v)} options={apiFormatOptions} />
                <SelectField label="Auth Field" icon={<Key size={10} weight="light" />} value={local.authField} onChange={(v) => update("authField", v)} options={authFieldOptions} />
              </div>
              <div className="mt-3 space-y-1">
                <ToggleRow label="Hide AI Signature" checked={local.hideAiSignature} onChange={(v) => update("hideAiSignature", v)} accent={accent} description="Hide the 'Claude' AI signature from code output" />
                <ToggleRow label="Teammates Mode" checked={local.teammatesMode} onChange={(v) => update("teammatesMode", v)} accent={accent} description="Enable experimental agent teams feature" />
                <ToggleRow label="Enable Tool Search" checked={local.enableToolSearch} onChange={(v) => update("enableToolSearch", v)} accent={accent} description="Allow Claude to search for available tools" />
                <ToggleRow label="High Intensity Thinking" checked={local.highIntensityThinking} onChange={(v) => update("highIntensityThinking", v)} accent={accent} description="Use maximum thinking intensity for complex tasks" />
                <ToggleRow label="Disable Auto Upgrade" checked={local.disableAutoUpgrade} onChange={(v) => update("disableAutoUpgrade", v)} accent={accent} description="Prevent Claude Code from auto-updating" />
                <ToggleRow label="1M Context (append [1m] to models)" checked={local.use1MContext} onChange={(v) => update("use1MContext", v)} accent={accent} description="Append [1m] suffix to all model names for 1M token context window" />
                <ToggleRow label="Separate Test Config" checked={local.useSeparateTestConfig} onChange={(v) => update("useSeparateTestConfig", v)} accent={accent} description="Use a separate config for test environment" />
                <ToggleRow label="Separate Proxy" checked={local.useSeparateProxy} onChange={(v) => update("useSeparateProxy", v)} accent={accent} description="Use a separate proxy configuration" />
                <ToggleRow label="Separate Billing" checked={local.useSeparateBilling} onChange={(v) => update("useSeparateBilling", v)} accent={accent} description="Use a separate billing configuration" />
              </div>
            </div>

            {/* Config JSON */}
            <div className="rounded-lg border border-white/[0.06] bg-white/[0.015] p-4">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <Code size={12} weight="light" style={{ color: accent }} />
                  <h3 className="text-[10px] uppercase tracking-[0.2em] font-semibold" style={{ color: accent }}>Config JSON</h3>
                </div>
                <div className="flex items-center gap-2">
                  <button onClick={() => setShowJson(!showJson)} className="text-[10px] text-zinc-500 hover:text-zinc-300 transition-colors">
                    {showJson ? "Hide" : "Show"}
                  </button>
                  <button onClick={handleCopy} className="flex items-center gap-1 text-[10px] text-zinc-500 hover:text-zinc-300 transition-colors">
                    {copied ? <Check size={10} /> : <Copy size={10} />}
                  </button>
                </div>
              </div>
              {showJson && (
                <pre className="mt-3 rounded-lg bg-black/20 px-3 py-2 text-[10px] font-mono text-zinc-400 overflow-x-auto whitespace-pre-wrap max-h-64 overflow-y-auto">
                  {configJson}
                </pre>
              )}
              {!showJson && (
                <div className="mt-3 flex items-center justify-center h-24 rounded-lg border border-dashed border-white/[0.06]">
                  <p className="text-[10px] text-zinc-700">Click Show to view generated config</p>
                </div>
              )}
            </div>
          </div>

          {/* Bottom: Advanced settings (collapsible) */}
          <ClaudeAdvancedPanel accent={accent} />
        </div>
      </section>
    </motion.main>
  );
}

/* ─── Codex Config Editor ─── */

function CodexConfigEditor({
  profile,
  onSave,
  onBack,
  accent,
  mode = "page",
  saveLabel = "Save",
  autoSaveAuth = true,
}: {
  profile: CodexProfile;
  onSave: (p: CodexProfile, options?: SaveOptions) => void;
  onBack: () => void;
  accent: string;
  mode?: "page" | "dialog";
  saveLabel?: string;
  autoSaveAuth?: boolean;
}) {
  const [local, setLocal] = useState(profile);
  const [copied, setCopied] = useState(false);
  const [showAuthJson, setShowAuthJson] = useState(false);
  const [showToml, setShowToml] = useState(false);
  const [authStatus, setAuthStatus] = useState<string | null>(null);
  const [authBusy, setAuthBusy] = useState(false);
  const [loginBaseline, setLoginBaseline] = useState<number | null | undefined>(undefined);
  const [authUrl, setAuthUrl] = useState<string | null>(null);

  const update = useCallback((key: string, value: string) => {
    setLocal((prev) => ({ ...prev, [key]: value }));
  }, []);

  const handleVendorSelect = useCallback((vendor: VendorMeta | null) => {
    setLocal((prev) => ({
      ...prev,
      vendorId: vendor?.id,
      ...(vendor
        ? vendor.baseUrls.codex
          ? { baseUrl: vendor.baseUrls.codex }
          : vendor.baseUrls["openai(chat)"]
          ? { baseUrl: vendor.baseUrls["openai(chat)"] }
          : {}
        : { baseUrl: "" }),
    }));
  }, []);

  const importCodexAuth = useCallback(
    async (requireModifiedAfter?: number | null) => {
      setAuthBusy(true);
      try {
        const imported = await invoke<CodexProfile>("import_codex_auth", {
          profile: local,
          requireModifiedAfter,
        });
        setLocal(imported);
        if (autoSaveAuth) {
          onSave(imported, { close: false });
        }
        setLoginBaseline(undefined);
        setAuthUrl(null);
        setAuthStatus("Imported ~/.codex/auth.json");
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        setAuthStatus(message);
        throw error;
      } finally {
        setAuthBusy(false);
      }
    },
    [autoSaveAuth, local, onSave],
  );

  const handleCodexLogin = async () => {
    setAuthBusy(true);
    try {
      const launch = await invoke<CodexLoginLaunch>("start_codex_login");
      setLoginBaseline(launch.previousModifiedAt ?? null);
      if (launch.authUrl) {
        setAuthUrl(launch.authUrl);
        await invoke("open_external_url", { url: launch.authUrl });
        setAuthStatus("Codex login page opened. Waiting for auth.json...");
      } else {
        setAuthUrl(null);
        setAuthStatus("Codex login started. Waiting for auth.json...");
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setAuthStatus(message);
    } finally {
      setAuthBusy(false);
    }
  };

  const handleImportAuthJson = async () => {
    try {
      await importCodexAuth();
    } catch {
      // importCodexAuth already reports the backend error in the status line.
    }
  };

  const handleOpenAuthPage = async () => {
    if (!authUrl) {
      return;
    }

    try {
      await invoke("open_external_url", { url: authUrl });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setAuthStatus(message);
    }
  };

  useEffect(() => {
    if (loginBaseline === undefined) {
      return;
    }

    let running = false;
    const poll = async () => {
      if (running) {
        return;
      }
      running = true;
      try {
        await importCodexAuth(loginBaseline);
      } catch {
        setAuthStatus("Waiting for ~/.codex/auth.json...");
      } finally {
        running = false;
      }
    };

    void poll();
    const timer = window.setInterval(() => {
      void poll();
    }, 3000);

    return () => window.clearInterval(timer);
  }, [importCodexAuth, loginBaseline]);

  const handleCopyAuth = () => {
    navigator.clipboard.writeText(local.authJson);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleCopyToml = () => {
    navigator.clipboard.writeText(local.configToml);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const effortOptions = [
    { value: "low", label: "Low" },
    { value: "medium", label: "Medium" },
    { value: "high", label: "High" },
    { value: "xhigh", label: "XHigh" },
  ];

  const reviewerOptions = [
    { value: "user", label: "User" },
    { value: "auto", label: "Auto" },
    { value: "user+auto", label: "User + Auto" },
  ];

  return (
    <motion.main
      className={`${mode === "dialog" ? "flex h-full min-h-0 flex-col" : "flex-1"} bg-gradient-to-br from-zinc-900/40 via-zinc-950 to-zinc-950`}
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      transition={{ duration: 0.3 }}
    >
      <header className="flex h-14 items-center justify-between border-b border-white/[0.04] px-8">
        <div className="flex items-center gap-3">
          {mode === "page" && (
            <button
              onClick={onBack}
              className="flex h-8 w-8 items-center justify-center rounded-md border border-white/[0.06] text-zinc-400 hover:text-zinc-200 hover:bg-white/[0.04] transition-colors"
            >
              <ArrowLeft size={14} weight="light" />
            </button>
          )}
          <div className="flex items-center gap-2">
            <CodexIcon size={16} />
            <span className="text-[13px] text-zinc-300 font-medium">Codex</span>
            <CaretRight size={10} weight="light" className="text-zinc-700" />
            <span className="text-[13px] text-zinc-500">{profile.name}</span>
          </div>
        </div>
        <div className="flex items-center gap-2">
          {mode === "dialog" && (
            <button
              onClick={onBack}
              className="rounded-full border border-white/[0.08] px-4 py-1.5 text-[10px] uppercase tracking-widest font-semibold text-zinc-400 transition-colors hover:border-white/[0.16] hover:text-zinc-200"
            >
              Cancel
            </button>
          )}
          <button
            onClick={() => onSave(local, { close: true })}
            className="rounded-full bg-zinc-100 px-5 py-1.5 text-[10px] uppercase tracking-widest font-semibold text-zinc-900 transition-transform hover:bg-white active:scale-[0.97]"
          >
            {saveLabel}
          </button>
        </div>
      </header>

      <section className="min-h-0 flex-1 overflow-y-auto px-6 py-5 lg:px-8">
        <div className="mx-auto max-w-5xl space-y-4">
          {/* Row 1: Vendor Selector (full width) */}
          <VendorSelector brandId="codex" selectedVendorId={local.vendorId} onSelect={handleVendorSelect} accent={accent} />

          {/* Vendor Info Card */}
          <VendorInfoCard
            vendor={getVendorById(local.vendorId)}
            brandAccent={accent}
            brandIcon={CodexIcon}
            brandName="Codex"
          />

          {/* Auth Card with OpenAI logo */}
          <div className="relative overflow-hidden rounded-xl border" style={{ borderColor: `${accent}40`, background: `linear-gradient(135deg, ${accent}08 0%, transparent 60%)` }}>
            <div className="pointer-events-none absolute -top-12 -right-12 h-24 w-24 rounded-full opacity-[0.06]" style={{ background: `radial-gradient(circle, ${accent} 0%, transparent 70%)` }} />
            <div className="relative p-4">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg" style={{ backgroundColor: `${accent}15`, border: `1px solid ${accent}25` }}>
                    <OpenaiIcon size={20} />
                  </div>
                  <div>
                    <div className="flex items-center gap-2">
                      <h3 className="text-[11px] uppercase tracking-[0.15em] font-semibold" style={{ color: accent }}>Authentication</h3>
                      <span className="rounded-full border border-white/[0.08] bg-black/20 px-2 py-0.5 text-[8px] font-mono text-zinc-500">~/.codex/auth.json</span>
                    </div>
                    {authStatus && <p className="text-[10px] text-zinc-500 mt-0.5">{authStatus}</p>}
                  </div>
                </div>
                <div className="flex gap-2">
                  <button onClick={handleCodexLogin} disabled={authBusy} className="rounded-full px-3 py-1.5 text-[10px] uppercase tracking-[0.12em] font-semibold text-zinc-950 transition-transform hover:translate-y-[-1px] active:scale-[0.98] disabled:opacity-50" style={{ backgroundColor: "#f4f4f5" }}>
                    Login
                  </button>
                  <button onClick={handleImportAuthJson} disabled={authBusy} className="rounded-full border border-white/[0.12] bg-black/20 px-3 py-1.5 text-[10px] uppercase tracking-[0.12em] font-semibold text-zinc-200 transition-colors hover:border-white/[0.22] disabled:opacity-50">
                    Import
                  </button>
                </div>
              </div>
              {authUrl && (
                <div className="mt-3 rounded-lg border border-white/[0.08] bg-black/20 p-2.5 flex items-center justify-between">
                  <p className="text-[10px] text-zinc-400 truncate mr-3">{authUrl}</p>
                  <button onClick={handleOpenAuthPage} className="shrink-0 rounded-full border border-white/[0.12] px-3 py-1 text-[9px] uppercase tracking-[0.12em] text-zinc-300 hover:border-white/[0.22]">Reopen</button>
                </div>
              )}
            </div>
          </div>

          {/* Row 2: Provider (left) + Model (right) */}
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
            {/* Provider */}
            <div className="rounded-lg border border-white/[0.06] bg-white/[0.015] p-4">
              <div className="flex items-center gap-2 mb-3">
                <Globe size={12} weight="light" style={{ color: accent }} />
                <h3 className="text-[10px] uppercase tracking-[0.2em] font-semibold" style={{ color: accent }}>Provider</h3>
              </div>
              <div className="space-y-3">
                <InputField label="Name" icon={<Cpu size={10} weight="light" />} value={local.name} onChange={(v) => update("name", v)} />
                <InputField label="API Key" icon={<Key size={10} weight="light" />} value={local.apiKey} onChange={(v) => update("apiKey", v)} type="password" />
                <InputField label="Base URL" icon={<Globe size={10} weight="light" />} value={local.baseUrl} onChange={(v) => update("baseUrl", v)} />
                <InputField label="Website" icon={<LinkIcon size={10} weight="light" />} value={local.website} onChange={(v) => update("website", v)} placeholder="https://..." />
                <InputField label="Notes" icon={<TextAlignLeft size={10} weight="light" />} value={local.notes} onChange={(v) => update("notes", v)} placeholder="Optional remarks" />
              </div>
            </div>

            {/* Model */}
            <div className="rounded-lg border border-white/[0.06] bg-white/[0.015] p-4">
              <div className="flex items-center gap-2 mb-3">
                <Cpu size={12} weight="light" style={{ color: accent }} />
                <h3 className="text-[10px] uppercase tracking-[0.2em] font-semibold" style={{ color: accent }}>Model</h3>
              </div>
              <ModelSelector brand="codex" baseUrl={local.baseUrl} apiKey={local.apiKey} vendorId={local.vendorId} currentValue={local.modelName} onSelect={(m) => update("modelName", m)} accent={accent} />
              <div className="grid grid-cols-2 gap-3 mt-3">
                <InputField label="Model" icon={<Cpu size={10} weight="light" />} value={local.modelName} onChange={(v) => update("modelName", v)} placeholder="e.g. gpt-5.5" />
                <SelectField label="Reasoning" icon={<Sliders size={10} weight="light" />} value={local.reasoningEffort} onChange={(v) => update("reasoningEffort", v)} options={effortOptions} />
                <InputField label="Context Window" icon={<Cpu size={10} weight="light" />} value={local.contextWindow} onChange={(v) => update("contextWindow", v)} />
                <InputField label="Compact Threshold" icon={<Cpu size={10} weight="light" />} value={local.autoCompactThreshold} onChange={(v) => update("autoCompactThreshold", v)} />
              </div>
            </div>
          </div>

          {/* Row 3: auth.json (left) + config.toml (right) — collapsible */}
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
            {/* auth.json */}
            <div className="rounded-lg border border-white/[0.06] bg-white/[0.015] p-4">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <Key size={12} weight="light" style={{ color: accent }} />
                  <h3 className="text-[10px] uppercase tracking-[0.2em] font-semibold" style={{ color: accent }}>auth.json</h3>
                </div>
                <div className="flex items-center gap-2">
                  <button onClick={() => setShowAuthJson(!showAuthJson)} className="text-[10px] text-zinc-500 hover:text-zinc-300 transition-colors">
                    {showAuthJson ? "Hide" : "Show"}
                  </button>
                  <button onClick={handleCopyAuth} className="flex items-center gap-1 text-[10px] text-zinc-500 hover:text-zinc-300 transition-colors">
                    {copied ? <Check size={10} /> : <Copy size={10} />}
                  </button>
                </div>
              </div>
              {showAuthJson ? (
                <textarea value={local.authJson} onChange={(e) => update("authJson", e.target.value)} className="w-full mt-3 h-48 rounded-lg border border-white/[0.06] bg-black/20 px-3 py-2 text-[10px] font-mono text-zinc-300 focus:border-zinc-600 focus:outline-none resize-y" spellCheck={false} />
              ) : (
                <div className="mt-3 flex items-center justify-center h-24 rounded-lg border border-dashed border-white/[0.06]">
                  <p className="text-[10px] text-zinc-700">Click Show to edit auth.json</p>
                </div>
              )}
            </div>

            {/* config.toml */}
            <div className="rounded-lg border border-white/[0.06] bg-white/[0.015] p-4">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <Code size={12} weight="light" style={{ color: accent }} />
                  <h3 className="text-[10px] uppercase tracking-[0.2em] font-semibold" style={{ color: accent }}>config.toml</h3>
                </div>
                <div className="flex items-center gap-2">
                  <button onClick={() => setShowToml(!showToml)} className="text-[10px] text-zinc-500 hover:text-zinc-300 transition-colors">
                    {showToml ? "Hide" : "Show"}
                  </button>
                  <button onClick={handleCopyToml} className="flex items-center gap-1 text-[10px] text-zinc-500 hover:text-zinc-300 transition-colors">
                    {copied ? <Check size={10} /> : <Copy size={10} />}
                  </button>
                </div>
              </div>
              {showToml ? (
                <textarea value={local.configToml} onChange={(e) => update("configToml", e.target.value)} className="w-full mt-3 h-48 rounded-lg border border-white/[0.06] bg-black/20 px-3 py-2 text-[10px] font-mono text-zinc-300 focus:border-zinc-600 focus:outline-none resize-y" spellCheck={false} />
              ) : (
                <div className="mt-3 flex items-center justify-center h-24 rounded-lg border border-dashed border-white/[0.06]">
                  <p className="text-[10px] text-zinc-700">Click Show to edit config.toml</p>
                </div>
              )}
            </div>
          </div>

          {/* Bottom: Advanced config.toml settings */}
          <CodexAdvancedPanel accent={accent} />
        </div>
      </section>
    </motion.main>
  );
}

/* ─── Generic Config Editor (Gemini placeholder) ─── */

function GenericConfigEditor({
  brand,
  profile,
  onSave,
  onBack,
  mode = "page",
  saveLabel = "Save",
}: {
  brand: BrandMeta;
  profile: GenericProfile;
  onSave: (p: GenericProfile, options?: SaveOptions) => void;
  onBack: () => void;
  mode?: "page" | "dialog";
  saveLabel?: string;
}) {
  const Icon = brand.icon;
  const [local, setLocal] = useState(profile);
  const update = (key: string, value: string) => setLocal((prev) => ({ ...prev, [key]: value }));

  const handleVendorSelect = (vendor: VendorMeta | null) => {
    setLocal((prev) => ({
      ...prev,
      vendorId: vendor?.id,
      ...(vendor?.baseUrls.claude ? { baseUrl: vendor.baseUrls.claude } : {}),
    }));
  };

  return (
    <motion.main className={`${mode === "dialog" ? "flex h-full min-h-0 flex-col" : "flex-1"} bg-gradient-to-br from-zinc-900/40 via-zinc-950 to-zinc-950`} initial={{ opacity: 0 }} animate={{ opacity: 1 }} transition={{ duration: 0.3 }}>
      <header className="flex h-14 items-center justify-between border-b border-white/[0.04] px-8">
        <div className="flex items-center gap-3">
          {mode === "page" && (
            <button onClick={onBack} className="flex h-8 w-8 items-center justify-center rounded-md border border-white/[0.06] text-zinc-400 hover:text-zinc-200 hover:bg-white/[0.04] transition-colors">
              <ArrowLeft size={14} weight="light" />
            </button>
          )}
          <div className="flex items-center gap-2">
            <Icon size={16} />
            <span className="text-[13px] text-zinc-300 font-medium">{brand.name}</span>
            <CaretRight size={10} weight="light" className="text-zinc-700" />
            <span className="text-[13px] text-zinc-500">{profile.name}</span>
          </div>
        </div>
        <div className="flex items-center gap-2">
          {mode === "dialog" && (
            <button
              onClick={onBack}
              className="rounded-full border border-white/[0.08] px-4 py-1.5 text-[10px] uppercase tracking-widest font-semibold text-zinc-400 transition-colors hover:border-white/[0.16] hover:text-zinc-200"
            >
              Cancel
            </button>
          )}
          <button onClick={() => onSave(local, { close: true })} className="rounded-full bg-zinc-100 px-5 py-1.5 text-[10px] uppercase tracking-widest font-semibold text-zinc-900 transition-transform hover:bg-white active:scale-[0.97]">
            {saveLabel}
          </button>
        </div>
      </header>
      <section className="min-h-0 flex-1 overflow-y-auto px-6 py-5 lg:px-8">
        <div className="mx-auto max-w-3xl space-y-4">
          {/* Vendor Selector */}
          <VendorSelector brandId={brand.id} selectedVendorId={local.vendorId} onSelect={handleVendorSelect} accent={brand.accent} />

          {/* Provider + Model — single compact card */}
          <div className="rounded-lg border border-white/[0.06] bg-white/[0.015] p-4">
            <div className="flex items-center gap-2 mb-3">
              <Cpu size={12} weight="light" style={{ color: brand.accent }} />
              <h3 className="text-[10px] uppercase tracking-[0.2em] font-semibold" style={{ color: brand.accent }}>Provider</h3>
            </div>
            <ModelSelector brand={brand.id} baseUrl={local.baseUrl} apiKey={local.apiKey} vendorId={local.vendorId} currentValue={local.model} onSelect={(m) => update("model", m)} accent={brand.accent} />
            <div className="grid grid-cols-2 gap-3 mt-3">
              <InputField label="Name" icon={<Cpu size={10} weight="light" />} value={local.name} onChange={(v) => update("name", v)} />
              <InputField label="API Key" icon={<Key size={10} weight="light" />} value={local.apiKey} onChange={(v) => update("apiKey", v)} type="password" />
              <InputField label="Base URL" icon={<Globe size={10} weight="light" />} value={local.baseUrl} onChange={(v) => update("baseUrl", v)} />
              <InputField label="Model" icon={<Cpu size={10} weight="light" />} value={local.model} onChange={(v) => update("model", v)} />
            </div>
          </div>
        </div>
      </section>
    </motion.main>
  );
}

/* ─── Profile Card Grid ─── */

function formatUsagePercent(window?: CodexUsageWindow | null) {
  if (!window || typeof window.remainingPercent !== "number") {
    return "--";
  }

  return `${Math.max(0, Math.round(window.remainingPercent))}%`;
}

function usageResetDate(window?: CodexUsageWindow | null) {
  if (!window) {
    return null;
  }

  if (typeof window.resetAfterSeconds === "number" && window.resetAfterSeconds > 0) {
    return new Date(Date.now() + window.resetAfterSeconds * 1000);
  }

  if (typeof window.resetAt === "number" && window.resetAt > 0) {
    const millis = window.resetAt > 1e11 ? window.resetAt : window.resetAt * 1000;
    return new Date(millis);
  }

  return null;
}

function usageResetParts(window?: CodexUsageWindow | null) {
  const date = usageResetDate(window);
  if (!date) {
    return null;
  }

  const hours = String(date.getHours()).padStart(2, "0");
  const minutes = String(date.getMinutes()).padStart(2, "0");
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");

  return {
    date: `${month}/${day}`,
    time: `${hours}:${minutes}`,
  };
}

function usageTextColor(window?: CodexUsageWindow | null) {
  const remaining = window?.remainingPercent;
  if (typeof remaining !== "number") {
    return "text-zinc-600";
  }
  if (remaining > 50) {
    return "text-emerald-400";
  }
  if (remaining > 20) {
    return "text-amber-300";
  }
  return "text-rose-300";
}

function usageErrorLabel(error?: string | null) {
  if (!error) {
    return null;
  }
  if (error === "token_expired") {
    return "token expired";
  }
  if (error === "forbidden") {
    return "403 forbidden";
  }
  if (error === "missing_auth") {
    return "auth missing";
  }
  return "usage unavailable";
}

function formatUsageLabel(label: string) {
  return label.toUpperCase();
}

function UsageResetText({
  label,
  window,
}: {
  label: string;
  window?: CodexUsageWindow | null;
}) {
  const parts = usageResetParts(window);
  if (!parts) {
    return (
      <p className="mt-1 text-center text-[11px] font-normal tabular-nums text-white">--</p>
    );
  }

  if (label === "5h") {
    return (
      <p className="mt-1 text-center text-[11px] font-normal tabular-nums text-white">
        {parts.time}
      </p>
    );
  }

  return (
    <p className="mt-1 flex items-center justify-between gap-2 text-[11px] font-normal tabular-nums text-white">
      <span>{parts.date}</span>
      <span>{parts.time}</span>
    </p>
  );
}

function codexUsageProfileKey(profile: CodexProfile) {
  return `${profile.id}:${profile.authJson || ""}`;
}

function codexProfileHasUsageAuth(profile: CodexProfile) {
  try {
    const auth = JSON.parse(profile.authJson || "{}") as {
      tokens?: {
        access_token?: string | null;
        account_id?: string | null;
      };
    };
    return Boolean(
      auth.tokens?.access_token?.trim() &&
      auth.tokens?.account_id?.trim()
    );
  } catch {
    return false;
  }
}

function ProfileGrid({
  brand,
  profiles,
  selectedIdx,
  activeIdx,
  isSaving,
  usagePending,
  isUsageRefreshing,
  canRefreshUsage,
  onSelect,
  onEdit,
  onAdd,
  onActivate,
  onRefreshUsage,
}: {
  brand: BrandMeta;
  profiles: ProfileCardInfo[];
  selectedIdx: number | null;
  activeIdx: number | null;
  isSaving: boolean;
  usagePending: boolean;
  isUsageRefreshing: boolean;
  canRefreshUsage: boolean;
  onSelect: (idx: number) => void;
  onEdit: (idx: number) => void;
  onAdd: () => void;
  onActivate: (idx: number) => void;
  onRefreshUsage: () => void;
}) {
  const Icon = brand.icon;

  return (
    <motion.main
      className="flex-1 bg-gradient-to-br from-zinc-900/30 via-zinc-950 to-zinc-950 relative"
      initial={false}
      animate={{ opacity: 1 }}
      transition={{ duration: 0.4 }}
    >
      {/* Subtle atmospheric glow */}
      <div className="pointer-events-none absolute inset-0 overflow-hidden">
        <div className="absolute -top-32 -right-32 h-64 w-64 rounded-full opacity-[0.03]" style={{ background: `radial-gradient(circle, ${brand.accent} 0%, transparent 70%)` }} />
      </div>

      <header className="relative flex h-12 items-center justify-between border-b border-white/[0.04] px-8">
        <div className="flex items-center gap-1.5 text-[10px] text-zinc-600 font-mono">
          <span className="text-zinc-500">~</span>
          <span>/</span>
          <span className="text-zinc-300">{brand.id}</span>
        </div>
        {brand.id === "codex" && (
          <button
            onClick={onRefreshUsage}
            disabled={!canRefreshUsage || isUsageRefreshing}
            className="flex items-center gap-2 rounded-full border border-white/[0.08] bg-white/[0.02] px-3 py-1.5 text-[10px] uppercase tracking-[0.12em] text-zinc-500 transition-colors hover:border-white/[0.16] hover:text-zinc-300 disabled:cursor-not-allowed disabled:opacity-40"
          >
            <ArrowClockwise size={13} className={isUsageRefreshing ? "animate-spin" : ""} />
            Refresh Usage
          </button>
        )}
      </header>

      <section className="overflow-y-auto p-8 lg:p-12">
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-6">
          {/* Add Card */}
          <motion.div
            key={`add-${brand.id}`}
            className="flex items-center justify-center rounded-2xl border border-dashed border-white/[0.08] bg-gradient-to-br from-white/[0.01] to-transparent cursor-pointer hover:bg-gradient-to-br hover:from-white/[0.025] hover:border-white/[0.15] transition-all min-h-[180px]"
            initial={false}
            whileHover={{ scale: 1.02 }}
            whileTap={{ scale: 0.98 }}
            onClick={onAdd}
          >
            <div className="flex flex-col items-center gap-3 text-zinc-600">
              <div className="flex h-12 w-12 items-center justify-center rounded-full border border-white/[0.06]">
                <Plus size={20} weight="light" />
              </div>
              <span className="text-[11px] uppercase tracking-[0.15em]">Add Profile</span>
            </div>
          </motion.div>

          {/* Profile Cards */}
          {profiles.map((p, i) => {
            const isActive = activeIdx === i;
            const isSelected = selectedIdx === i;
            const isActiveOnly = isActive && !isSelected;
            const isIdle = !isActive && !isSelected;
            const vendor = getVendorById(p.vendorId);
            const CardIcon = vendor?.icon ?? Icon;
            const cardAccent = vendor?.accent ?? brand.accent;
            const cardAccentBg = vendor ? `${vendor.accent}18` : brand.accentBg;
            return (
              <motion.div
                key={`${brand.id}-${p.id}`}
                className={`group relative rounded-2xl border overflow-hidden transition-all cursor-pointer min-h-[180px] ${
                  isSelected
                    ? "bg-gradient-to-br from-white/[0.06] via-white/[0.035] to-white/[0.02]"
                    : isActiveOnly
                    ? "bg-gradient-to-br from-white/[0.025] via-white/[0.012] to-white/[0.006] opacity-60 hover:opacity-80"
                    : "bg-gradient-to-br from-white/[0.018] via-white/[0.008] to-white/[0.004] opacity-45 hover:opacity-70"
                }`}
                style={{
                  borderColor: isSelected ? cardAccent : isActiveOnly ? `${cardAccent}55` : "rgba(255,255,255,0.04)",
                  boxShadow: isSelected ? `0 0 34px ${cardAccent}24` : "none",
                }}
                initial={false}
                whileHover={{ y: -3, transition: { type: "spring", stiffness: 260, damping: 20 } }}
                onClick={() => onSelect(i)}
              >
                {/* Hover glow */}
                <div
                  className="absolute inset-0 rounded-2xl pointer-events-none opacity-0 group-hover:opacity-100 transition-opacity duration-300"
                  style={{ boxShadow: `inset 0 1px 1px rgba(255,255,255,0.04), 0 0 20px ${cardAccent}08` }}
                />
                {isSelected && (
                  <>
                    <div
                      className="absolute inset-0 rounded-2xl pointer-events-none"
                      style={{ border: `1.5px solid ${cardAccent}` }}
                    />
                    <div
                      className="absolute inset-[1px] rounded-[15px] pointer-events-none"
                      style={{ boxShadow: `inset 0 0 20px ${cardAccent}12, 0 0 40px ${cardAccent}08` }}
                    />
                  </>
                )}

                <div className="relative p-6 flex flex-col h-full">
                  <div className="flex items-start justify-between">
                    <div
                      className="flex h-10 w-10 items-center justify-center rounded-xl"
                      style={{ backgroundColor: cardAccentBg }}
                    >
                      <CardIcon size={22} />
                    </div>
                    {isActive && (
                      <span
                        className="rounded-full px-2.5 py-1 text-[9px] uppercase tracking-[0.15em] font-semibold backdrop-blur-md"
                        style={{ color: cardAccent, backgroundColor: `${cardAccent}14`, border: `1px solid ${cardAccent}20` }}
                      >
                        Active
                      </span>
                    )}
                  </div>

                  <div className="mt-4">
                    <h3 className={`text-[15px] font-semibold transition-colors ${
                      isSelected ? "text-zinc-100" : isActiveOnly ? "text-zinc-400" : "text-zinc-500"
                    }`}>
                      {p.name}
                    </h3>
                    {p.model && (
                      <p className={`mt-0.5 text-[11px] font-mono truncate ${
                        isSelected ? "text-zinc-500" : "text-zinc-700"
                      }`}>{p.model}</p>
                    )}
                    {brand.id === "codex" && p.usageEnabled && (
                      <div className="mt-3 space-y-2">
                        <div className="flex items-center justify-between gap-2">
                          <span className="truncate text-[10px] text-zinc-500">
                            {p.accountLabel || "No account"}
                          </span>
                          {p.accountPlan && p.accountPlan !== "?" && (
                            <span className="shrink-0 rounded-full border border-white/[0.08] px-2 py-0.5 text-[8px] uppercase tracking-[0.12em] text-zinc-500">
                              {p.accountPlan}
                            </span>
                          )}
                        </div>
                        {p.usageLoading && !p.usage ? (
                          <p className="text-[10px] uppercase tracking-[0.12em] text-zinc-700">Loading usage</p>
                        ) : usagePending && !p.usage ? (
                          <p className="text-[10px] uppercase tracking-[0.12em] text-zinc-700">Usage pending</p>
                        ) : usageErrorLabel(p.usage?.error) ? (
                          <p className="text-[10px] uppercase tracking-[0.12em] text-zinc-700">
                            {usageErrorLabel(p.usage?.error)}
                          </p>
                        ) : (
                          <div className="grid grid-cols-2 gap-2">
                            {[
                              ["5h", p.usage?.fiveHour],
                              ["7d", p.usage?.weekly],
                            ].map(([label, window]) => (
                              <div key={label as string} className="rounded-md border border-white/[0.05] bg-black/20 px-2 py-1.5">
                                <div className="flex items-start justify-between gap-2">
                                  <span className="text-[9px] uppercase tracking-[0.14em] text-zinc-600">{formatUsageLabel(label as string)}</span>
                                  <span className={`shrink-0 text-right text-[11px] font-semibold tabular-nums tracking-normal ${usageTextColor(window as CodexUsageWindow | null | undefined)}`}>
                                    {formatUsagePercent(window as CodexUsageWindow | null | undefined)}
                                  </span>
                                </div>
                                <UsageResetText label={label as string} window={window as CodexUsageWindow | null | undefined} />
                              </div>
                            ))}
                          </div>
                        )}
                      </div>
                    )}
                  </div>

                  {isIdle && (
                    <div className="mt-auto pt-4">
                      <span className="text-[10px] text-zinc-800">Click to select</span>
                    </div>
                  )}
                  {isSelected && !isActive && (
                    <div className="mt-auto pt-4">
                      <span className="text-[10px] text-zinc-600">Selected - Press Save to activate</span>
                    </div>
                  )}
                </div>
              </motion.div>
            );
          })}
        </div>
      </section>

      {/* Fixed action buttons at bottom-right */}
      <div className="fixed bottom-6 right-8 flex items-center gap-3 z-10">
        <button
          onClick={() => onEdit(selectedIdx ?? activeIdx ?? 0)}
          disabled={selectedIdx === null && activeIdx === null}
          className="flex items-center gap-2 rounded-full border border-white/10 bg-zinc-900/90 backdrop-blur-sm px-5 py-2.5 text-[11px] uppercase tracking-[0.15em] font-medium text-zinc-300 hover:text-white hover:border-white/20 transition-all active:scale-[0.97] disabled:opacity-30 disabled:cursor-not-allowed"
        >
          Edit Config
        </button>
        <button
          onClick={() => onActivate(selectedIdx ?? activeIdx ?? 0)}
          disabled={isSaving || (selectedIdx === null && activeIdx === null)}
          className="flex items-center gap-2 rounded-full bg-zinc-100 px-5 py-2.5 text-[11px] uppercase tracking-[0.15em] font-semibold text-zinc-900 hover:bg-white transition-all active:scale-[0.97] disabled:opacity-30 disabled:cursor-not-allowed"
        >
          {isSaving ? "Saving" : "Save"}
        </button>
      </div>
    </motion.main>
  );
}

/* ─── App ─── */

type ViewMode = "grid" | "config";
type AddDraft = {
  brand: ProviderBrand;
  profile: ProviderProfile;
};

function AddProfileDialog({
  draft,
  onCancel,
  onConfirm,
}: {
  draft: AddDraft;
  onCancel: () => void;
  onConfirm: (profile: ProviderProfile) => void;
}) {
  const brand = brands.find((item) => item.id === draft.brand)!;

  return (
    <div className="fixed inset-0 z-20 flex items-center justify-center bg-zinc-950/75 px-6 py-6 backdrop-blur-sm">
      <motion.div
        className="h-[min(86dvh,760px)] w-[min(1120px,calc(100vw-48px))] overflow-hidden rounded-xl border border-white/[0.08] bg-zinc-950 shadow-2xl shadow-zinc-950/70"
        initial={{ opacity: 0, y: 18, scale: 0.98 }}
        animate={{ opacity: 1, y: 0, scale: 1 }}
        exit={{ opacity: 0, y: 18, scale: 0.98 }}
        transition={{ duration: 0.25, ease: [0.16, 1, 0.3, 1] }}
      >
        {draft.brand === "claude" ? (
          <ClaudeConfigEditor
            profile={draft.profile as ClaudeProfile}
            onSave={(profile) => onConfirm(profile)}
            onBack={onCancel}
            accent={brand.accent}
            mode="dialog"
            saveLabel="Confirm"
          />
        ) : draft.brand === "codex" ? (
          <CodexConfigEditor
            profile={draft.profile as CodexProfile}
            onSave={(profile) => onConfirm(profile)}
            onBack={onCancel}
            accent={brand.accent}
            mode="dialog"
            saveLabel="Confirm"
            autoSaveAuth={false}
          />
        ) : (
          <GenericConfigEditor
            brand={brand}
            profile={draft.profile as GenericProfile}
            onSave={(profile) => onConfirm(profile)}
            onBack={onCancel}
            mode="dialog"
            saveLabel="Confirm"
          />
        )}
      </motion.div>
    </div>
  );
}

export default function App() {
  const [activeBrand, setActiveBrand] = useState<ProviderBrand>("claude");
  const [viewMode, setViewMode] = useState<ViewMode>("grid");
  const [activeProfileIdx, setActiveProfileIdx] = useState<number | null>(null);
  const [selectedProfileIdx, setSelectedProfileIdx] = useState<number | null>(null);
  const [activeProfileIds, setActiveProfileIds] = useState<ActiveProfileIds>(defaultActiveProfileIds);
  const [claudeProfiles, setClaudeProfiles] = useState<ClaudeProfile[]>(defaultClaudeProfiles);
  const [codexProfiles, setCodexProfiles] = useState<CodexProfile[]>(defaultCodexProfiles);
  const [geminiProfiles, setGeminiProfiles] = useState<GenericProfile[]>(defaultGeminiProfiles);
  const [codexUsageById, setCodexUsageById] = useState<Record<string, CodexUsageState>>({});
  const codexUsageInFlight = useRef<Set<string>>(new Set());
  const [codexUsageInitialLoadDone, setCodexUsageInitialLoadDone] = useState(false);
  const [isActivatingProfile, setIsActivatingProfile] = useState(false);
  const [addDraft, setAddDraft] = useState<AddDraft | null>(null);

  const updateBrandState = useCallback(
    <T extends ProviderBrand>(
      brand: T,
      state: ProviderState,
      fallbackActiveId: string | null = null,
    ) => {
      if (brand === "claude") {
        const profiles = profilesFromState<ClaudeProfile>(state);
        if (profiles) {
          setClaudeProfiles(profiles);
          setActiveProfileIds((prev) => ({
            ...prev,
            claude: activeIdFromState(profiles, fallbackActiveId, state.activeId),
          }));
        }
      } else if (brand === "codex") {
        const profiles = profilesFromState<CodexProfile>(state);
        if (profiles) {
          setCodexProfiles(profiles);
          setActiveProfileIds((prev) => ({
            ...prev,
            codex: activeIdFromState(profiles, fallbackActiveId, state.activeId),
          }));
        }
      } else {
        const profiles = profilesFromState<GenericProfile>(state);
        if (profiles) {
          setGeminiProfiles(profiles);
          setActiveProfileIds((prev) => ({
            ...prev,
            gemini: activeIdFromState(profiles, fallbackActiveId, state.activeId),
          }));
        }
      }
    },
    [],
  );

  useEffect(() => {
    brands.forEach(({ id }) => {
      void invoke<ProviderState>("list_provider_profiles", { brand: id })
        .then((state) => updateBrandState(id, state, defaultActiveProfileIds[id]))
        .catch(() => {
          // Browser/Vite dev fallback: keep the hardcoded defaults.
        });
    });
  }, [updateBrandState]);

  const startCodexUsageLoad = useCallback(
    (profilesToLoad: CodexProfile[]) => {
      if (profilesToLoad.length === 0) {
        return false;
      }
    if (Object.values(codexUsageById).some((state) => state.loading)) {
        return false;
    }

    setCodexUsageById((prev) => ({
      ...prev,
      ...Object.fromEntries(
        profilesToLoad.map((profile) => {
          const profileKey = codexUsageProfileKey(profile);
          codexUsageInFlight.current.add(profileKey);
          return [
            profile.id,
            {
              ...prev[profile.id],
              loading: true,
              profileKey,
            },
          ];
          }),
        ),
    }));

    for (const profile of profilesToLoad) {
      const profileKey = codexUsageProfileKey(profile);
      void invoke<CodexUsageInfo>("fetch_codex_provider_usage", { profile })
        .then((usage) => {
          setCodexUsageById((prev) => ({
            ...prev,
            [profile.id]: { loading: false, profileKey, data: usage },
          }));
        })
        .catch(() => {
          setCodexUsageById((prev) => ({
            ...prev,
            [profile.id]: {
              loading: false,
              profileKey,
              data: { error: "unavailable" },
            },
          }));
        })
        .finally(() => {
          codexUsageInFlight.current.delete(profileKey);
        });
    }

      return true;
    },
    [codexUsageById],
  );

  useEffect(() => {
    if (codexUsageInitialLoadDone) {
      return;
    }

    const profilesToLoad = codexProfiles.filter(codexProfileHasUsageAuth);
    if (startCodexUsageLoad(profilesToLoad)) {
      setCodexUsageInitialLoadDone(true);
    }
  }, [codexProfiles, codexUsageInitialLoadDone, startCodexUsageLoad]);

  const selectBrand = (id: ProviderBrand) => {
    setActiveBrand(id);
    setViewMode("grid");
    setActiveProfileIdx(null);
    setSelectedProfileIdx(0);
    setAddDraft(null);
  };

  const handleSelectProfile = (idx: number) => {
    setSelectedProfileIdx(idx);
  };

  const handleRefreshCodexUsage = () => {
    if (Object.values(codexUsageById).some((state) => state.loading)) {
      return;
    }

    if (!codexProfiles.some(codexProfileHasUsageAuth)) {
      return;
    }

    const profilesToLoad = codexProfiles.filter(codexProfileHasUsageAuth);
    if (startCodexUsageLoad(profilesToLoad)) {
      setCodexUsageInitialLoadDone(true);
    }
  };

  function getCurrentProfile(idx: number): ProviderProfile | null {
    if (activeBrand === "claude") {
      return claudeProfiles[idx] ?? null;
    }

    if (activeBrand === "codex") {
      return codexProfiles[idx] ?? null;
    }

    return geminiProfiles[idx] ?? null;
  }

  const handleActivateProfile = async (idx: number) => {
    if (isActivatingProfile) {
      return;
    }

    const profile = getCurrentProfile(idx);

    if (!profile) {
      return;
    }

    setIsActivatingProfile(true);

    try {
      const state = await invoke<ProviderState>("save_and_activate_provider_profile", {
        brand: activeBrand,
        profile,
      });
      updateBrandState(activeBrand, state, profile.id);
    } catch (error) {
      console.error("Failed to save and activate provider profile.", error);
    } finally {
      setIsActivatingProfile(false);
    }
  };

  const selectProfile = (idx: number) => {
    setActiveProfileIdx(idx);
    setViewMode("config");
  };

  const goBack = () => {
    setViewMode("grid");
    setActiveProfileIdx(null);
  };

  const handleSaveClaude = async (updated: ClaudeProfile, options: SaveOptions = { close: true }) => {
    if (activeProfileIdx !== null) {
      setClaudeProfiles((prev) =>
        prev.map((p, i) => (i === activeProfileIdx ? updated : p))
      );

      try {
        const state = await invoke<ProviderState>("save_provider_profile", { brand: "claude", profile: updated });
        updateBrandState("claude", state, activeProfileIds.claude);
      } catch {
        // Browser/Vite dev fallback: the local edit is already applied.
      }
      if (options.close !== false) {
        goBack();
      }
    }
  };

  const handleSaveCodex = async (updated: CodexProfile, options: SaveOptions = { close: true }) => {
    if (activeProfileIdx !== null) {
      setCodexProfiles((prev) =>
        prev.map((p, i) => (i === activeProfileIdx ? updated : p))
      );

      try {
        const state = await invoke<ProviderState>("save_provider_profile", { brand: "codex", profile: updated });
        updateBrandState("codex", state, activeProfileIds.codex);
      } catch {
        // Browser/Vite dev fallback: the local edit is already applied.
      }
      if (options.close !== false) {
        goBack();
      }
    }
  };

  const handleSaveGemini = async (updated: GenericProfile, options: SaveOptions = { close: true }) => {
    if (activeProfileIdx !== null) {
      setGeminiProfiles((prev) =>
        prev.map((p, i) => (i === activeProfileIdx ? updated : p))
      );

      try {
        const state = await invoke<ProviderState>("save_provider_profile", { brand: "gemini", profile: updated });
        updateBrandState("gemini", state, activeProfileIds.gemini);
      } catch {
        // Browser/Vite dev fallback: the local edit is already applied.
      }
      if (options.close !== false) {
        goBack();
      }
    }
  };

  const handleAdd = () => {
    const id = `${activeBrand}-${Date.now()}`;
    if (activeBrand === "claude") {
      const base = claudeProfiles[0];
      const newProfile: ClaudeProfile = {
        ...base,
        id,
        name: "New Provider",
        apiKey: "",
      };
      setAddDraft({ brand: activeBrand, profile: newProfile });
    } else if (activeBrand === "codex") {
      const base = codexProfiles[0];
      const newProfile: CodexProfile = {
        ...base,
        id,
        name: "New Provider",
        apiKey: "",
      };
      setAddDraft({ brand: activeBrand, profile: newProfile });
    } else {
      const base = geminiProfiles[0] ?? defaultGeminiProfiles[0];
      const newProfile: GenericProfile = {
        ...base,
        id,
        name: "New Provider",
        apiKey: "",
      };
      setAddDraft({ brand: activeBrand, profile: newProfile });
    }
  };

  const handleConfirmAdd = async (profile: ProviderProfile) => {
    if (!addDraft) {
      return;
    }

    const draftBrand = addDraft.brand;
    const nextIndex =
      draftBrand === "claude"
        ? claudeProfiles.length
        : draftBrand === "codex"
        ? codexProfiles.length
        : geminiProfiles.length;
    if (draftBrand === "claude") {
      setClaudeProfiles((prev) => [...prev, profile as ClaudeProfile]);
    } else if (draftBrand === "codex") {
      setCodexProfiles((prev) => [...prev, profile as CodexProfile]);
    } else {
      setGeminiProfiles((prev) => [...prev, profile as GenericProfile]);
    }

    setSelectedProfileIdx(nextIndex);
    setAddDraft(null);

    try {
      const state = await invoke<ProviderState>("save_provider_profile", { brand: draftBrand, profile });
      updateBrandState(draftBrand, state, activeProfileIds[draftBrand]);
    } catch {
      // Browser/Vite dev fallback: the local provider card stays in place.
    }
  };

  const brand = brands.find((b) => b.id === activeBrand)!;
  const profiles =
    activeBrand === "claude"
      ? claudeProfiles.map((p) => ({ id: p.id, name: p.name, vendorId: p.vendorId, model: p.mainModel }))
      : activeBrand === "codex"
      ? codexProfiles.map((p) => {
          const usageState = codexUsageById[p.id];
          const usage = usageState?.data;
          const usageEnabled = codexProfileHasUsageAuth(p);
          return {
            id: p.id,
            name: p.name,
            vendorId: p.vendorId,
            model: p.modelName,
            usageEnabled,
            accountLabel: usage?.accountEmail || undefined,
            accountPlan: usage?.accountPlan || undefined,
            usage: usageEnabled ? usage : undefined,
            usageLoading: usageEnabled ? usageState?.loading ?? false : false,
          };
        })
      : geminiProfiles.map((p) => ({ id: p.id, name: p.name, vendorId: p.vendorId, model: p.model }));
  const activeIdx = profiles.findIndex((profile) => profile.id === activeProfileIds[activeBrand]);
  const activeGridIdx = activeIdx >= 0 ? activeIdx : null;
  const isCodexUsageRefreshing = Object.values(codexUsageById).some((state) => state.loading);
  const hasCodexUsageProfiles = codexProfiles.some(codexProfileHasUsageAuth);
  const isCodexUsagePending =
    activeBrand === "codex" &&
    viewMode === "grid" &&
    hasCodexUsageProfiles &&
    !codexUsageInitialLoadDone &&
    !isCodexUsageRefreshing;

  return (
    <div className="flex min-h-[100dvh] w-full">
      <Sidebar active={activeBrand} onSelect={selectBrand} />
      <AnimatePresence mode="wait">
        {viewMode === "grid" ? (
          <ProfileGrid
            key="grid"
            brand={brand}
            profiles={profiles}
            selectedIdx={selectedProfileIdx}
            activeIdx={activeGridIdx}
            isSaving={isActivatingProfile}
            usagePending={isCodexUsagePending}
            isUsageRefreshing={isCodexUsageRefreshing}
            canRefreshUsage={hasCodexUsageProfiles}
            onSelect={handleSelectProfile}
            onEdit={(idx) => selectProfile(idx)}
            onAdd={handleAdd}
            onActivate={handleActivateProfile}
            onRefreshUsage={handleRefreshCodexUsage}
          />
        ) : activeProfileIdx !== null && profiles[activeProfileIdx] ? (
          activeBrand === "claude" ? (
            <ClaudeConfigEditor
              key={`config-${activeBrand}`}
              profile={claudeProfiles[activeProfileIdx]}
              onSave={handleSaveClaude}
              onBack={goBack}
              accent={brand.accent}
            />
          ) : activeBrand === "codex" ? (
            <CodexConfigEditor
              key={`config-${activeBrand}`}
              profile={codexProfiles[activeProfileIdx]}
              onSave={handleSaveCodex}
              onBack={goBack}
              accent={brand.accent}
            />
          ) : (
            <GenericConfigEditor
              key={`config-${activeBrand}`}
              brand={brand}
              profile={geminiProfiles[activeProfileIdx]}
              onSave={handleSaveGemini}
              onBack={goBack}
            />
          )
        ) : null}
      </AnimatePresence>
      <AnimatePresence>
        {addDraft && (
          <AddProfileDialog
            key={`add-${addDraft.brand}-${addDraft.profile.id}`}
            draft={addDraft}
            onCancel={() => setAddDraft(null)}
            onConfirm={handleConfirmAdd}
          />
        )}
      </AnimatePresence>
    </div>
  );
}
