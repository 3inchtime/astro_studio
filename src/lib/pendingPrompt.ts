const PENDING_PROMPT_KEY = "astro-studio.pending-prompt";

export function savePendingPrompt(prompt: string): void {
  if (typeof window === "undefined") return;

  sessionStorage.setItem(PENDING_PROMPT_KEY, prompt);
}

export function consumePendingPrompt(): string | null {
  if (typeof window === "undefined") return null;

  const prompt = sessionStorage.getItem(PENDING_PROMPT_KEY);
  if (!prompt) return null;

  sessionStorage.removeItem(PENDING_PROMPT_KEY);
  return prompt;
}
