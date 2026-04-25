import { useEffect, useState } from "react";
import { motion } from "framer-motion";
import { saveApiKey, getApiKey, saveBaseUrl, getBaseUrl } from "../lib/api";
import { Eye, EyeOff, Check, Key, Globe, Info, Zap } from "lucide-react";

const DEFAULT_BASE_URL = "https://api.openai.com/v1";

const cardVariants = {
  hidden: { opacity: 0, y: 10, scale: 0.98 },
  visible: (i: number) => ({
    opacity: 1,
    y: 0,
    scale: 1,
    transition: { delay: i * 0.06, duration: 0.4, ease: [0.22, 1, 0.36, 1] as [number, number, number, number] },
  }),
};

function maskKey(key: string): string {
  if (key.length <= 8) return "sk-****";
  return key.slice(0, 3) + "..." + key.slice(-4);
}

export default function SettingsPage() {
  const [apiKey, setApiKey] = useState("");
  const [showKey, setShowKey] = useState(false);
  const [keySaved, setKeySaved] = useState(false);

  const [baseUrl, setBaseUrl] = useState(DEFAULT_BASE_URL);
  const [urlSaved, setUrlSaved] = useState(false);

  useEffect(() => {
    getApiKey().then((key) => {
      if (key) setApiKey(key);
    });
    getBaseUrl().then((url) => {
      setBaseUrl(url);
    });
  }, []);

  async function handleSaveKey() {
    await saveApiKey(apiKey);
    setShowKey(false);
    setKeySaved(true);
    setTimeout(() => setKeySaved(false), 2000);
  }

  async function handleSaveUrl() {
    const url = baseUrl.trim() || DEFAULT_BASE_URL;
    await saveBaseUrl(url);
    setBaseUrl(url);
    setUrlSaved(true);
    setTimeout(() => setUrlSaved(false), 2000);
  }

  const displayKey = showKey ? apiKey : (apiKey ? maskKey(apiKey) : "");

  return (
    <div className="h-full overflow-y-auto">
      <div className="mx-auto max-w-md p-8">
        <motion.h2
          initial={{ opacity: 0, y: -4 }}
          animate={{ opacity: 1, y: 0 }}
          className="mb-6 text-[16px] font-semibold text-foreground tracking-tight"
        >
          Settings
        </motion.h2>

        <div className="space-y-3">
          <motion.div
            custom={0}
            variants={cardVariants}
            initial="hidden"
            animate="visible"
            className="rounded-[12px] border border-border-subtle bg-surface p-5 shadow-card"
          >
            <div className="mb-4 flex items-start gap-3">
              <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[10px] bg-primary/5 border border-primary/10">
                <Key size={14} className="text-primary" strokeWidth={2} />
              </div>
              <div>
                <h3 className="text-[13px] font-semibold text-foreground">
                  API Key
                </h3>
                <p className="mt-0.5 text-[11px] text-muted/60">
                  Encrypted and stored locally on your device
                </p>
              </div>
            </div>

            <div className="relative">
              <input
                type={showKey ? "text" : "password"}
                value={displayKey}
                onChange={(e) => { setApiKey(e.target.value); setKeySaved(false); }}
                onFocus={() => { if (!showKey) setShowKey(true); }}
                placeholder="sk-..."
                className="h-[36px] w-full rounded-[10px] border border-border-subtle bg-subtle/30 px-3 pr-9 text-[12px] text-foreground placeholder:text-muted/40 focus:outline-none focus:border-primary/25 focus:bg-surface focus:shadow-card transition-all duration-200"
              />
              <button
                onClick={() => setShowKey(!showKey)}
                className="absolute right-2.5 top-1/2 -translate-y-1/2 text-muted/40 hover:text-muted transition-colors"
              >
                {showKey ? <EyeOff size={13} /> : <Eye size={13} />}
              </button>
            </div>

            <motion.button
              onClick={handleSaveKey}
              disabled={!apiKey.trim()}
              whileTap={{ scale: 0.97 }}
              className="mt-3 flex h-[32px] items-center gap-1.5 rounded-[8px] border border-border-subtle px-4 text-[12px] font-medium text-muted transition-all hover:border-border hover:text-foreground disabled:opacity-30"
            >
              {keySaved ? (
                <>
                  <Check size={13} className="text-success" />
                  <span className="text-success">Saved</span>
                </>
              ) : "Save Key"}
            </motion.button>
          </motion.div>

          <motion.div
            custom={1}
            variants={cardVariants}
            initial="hidden"
            animate="visible"
            className="rounded-[12px] border border-border-subtle bg-surface p-5 shadow-card"
          >
            <div className="mb-4 flex items-start gap-3">
              <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[10px] bg-primary/5 border border-primary/10">
                <Globe size={14} className="text-primary" strokeWidth={2} />
              </div>
              <div>
                <h3 className="text-[13px] font-semibold text-foreground">
                  API Endpoint
                </h3>
                <p className="mt-0.5 text-[11px] text-muted/60">
                  OpenAI-compatible endpoint for image generation
                </p>
              </div>
            </div>

            <input
              type="text"
              value={baseUrl}
              onChange={(e) => { setBaseUrl(e.target.value); setUrlSaved(false); }}
              placeholder={DEFAULT_BASE_URL}
              className="h-[36px] w-full rounded-[10px] border border-border-subtle bg-subtle/30 px-3 text-[12px] text-foreground placeholder:text-muted/40 focus:outline-none focus:border-primary/25 focus:bg-surface focus:shadow-card transition-all duration-200"
            />

            <motion.button
              onClick={handleSaveUrl}
              whileTap={{ scale: 0.97 }}
              className="mt-3 flex h-[32px] items-center gap-1.5 rounded-[8px] border border-border-subtle px-4 text-[12px] font-medium text-muted transition-all hover:border-border hover:text-foreground"
            >
              {urlSaved ? (
                <>
                  <Check size={13} className="text-success" />
                  <span className="text-success">Saved</span>
                </>
              ) : "Save URL"}
            </motion.button>
          </motion.div>

          <motion.div
            custom={2}
            variants={cardVariants}
            initial="hidden"
            animate="visible"
            className="rounded-[12px] border border-border-subtle bg-surface p-5 shadow-card"
          >
            <div className="flex items-start gap-3">
              <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[10px] bg-primary/5 border border-primary/10">
                <Info size={14} className="text-primary" strokeWidth={2} />
              </div>
              <div className="flex-1">
                <h3 className="text-[13px] font-semibold text-foreground">
                  About
                </h3>
                <div className="mt-2 flex items-center gap-2">
                  <div className="flex h-5 w-5 items-center justify-center rounded-[6px] gradient-primary">
                    <Zap size={10} className="text-white" strokeWidth={2.5} />
                  </div>
                  <span className="text-[12px] font-medium text-foreground">
                    Astro Studio
                  </span>
                  <span className="text-[10px] text-muted/50">v0.1.0</span>
                </div>
                <p className="mt-1.5 text-[11px] text-muted/60 leading-relaxed">
                  AI Image Generation Desktop Client
                  <br />
                  Powered by GPT Image
                </p>
              </div>
            </div>
          </motion.div>
        </div>
      </div>
    </div>
  );
}
