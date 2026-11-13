import { useEffect, useId, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { Check, CircleHelp, Loader2, X } from "lucide-react";
import type { ImageModel, ModelProviderProfile } from "../../types";
import type {
  ProviderDiagnosticResult,
  ProviderSmokePlan,
} from "../../types/diagnostics";
import {
  diagnoseImageProvider,
  getProviderSmokePlan,
  runProviderSmokeTest,
} from "../../lib/diagnosticsApi";
import { useDialogFocus } from "../../hooks/useDialogFocus";

const buttonClass =
  "focus-ring min-h-9 rounded-lg border border-border-subtle bg-surface px-3 py-2 text-xs font-medium text-foreground transition-colors hover:bg-subtle disabled:cursor-not-allowed disabled:opacity-50";
export function ProviderDiagnostics({
  model,
  profile,
  dirty,
}: {
  model: ImageModel;
  profile: ModelProviderProfile;
  dirty: boolean;
}) {
  const { t } = useTranslation();
  const [result, setResult] = useState<ProviderDiagnosticResult | null>(null);
  const [plan, setPlan] = useState<ProviderSmokePlan | null>(null);
  const [busy, setBusy] = useState<"diagnose" | "plan" | "generate" | null>(
    null,
  );
  const [notice, setNotice] = useState<string | null>(null);
  const [conversationId, setConversationId] = useState<string | null>(null);
  const version = useRef(0);
  const active = useRef(false);
  const configuration = JSON.stringify([model, profile, dirty]);
  useEffect(() => {
    version.current++;
    setResult(null);
    setPlan(null);
    setBusy(null);
    setNotice(null);
    setConversationId(null);
    active.current = false;
    return () => {
      version.current++;
    };
  }, [configuration]);
  const titleId = useId();
  const closePlan = () => {
    if (!active.current) setPlan(null);
  };
  const dialogRef = useDialogFocus({
    open: !!plan,
    onClose: closePlan,
    busy: busy === "generate",
  });
  const text = (key: string, fallback: string) =>
    t(`diagnostics.${key}`, { defaultValue: fallback });
  async function run(kind: "diagnose" | "plan") {
    if (dirty || active.current) return;
    const current = version.current;
    active.current = true;
    setBusy(kind);
    setNotice(null);
    setConversationId(null);
    try {
      if (kind === "diagnose") {
        const next = await diagnoseImageProvider(model, profile.id);
        if (current === version.current) setResult(next);
      } else {
        const next = await getProviderSmokePlan(model, profile.id);
        if (current === version.current) setPlan(next);
      }
    } catch {
      if (current === version.current)
        setNotice(
          text(
            "requestFailed",
            "Could not check this saved provider. Check its settings and try again.",
          ),
        );
    } finally {
      if (current === version.current) {
        active.current = false;
        setBusy(null);
      }
    }
  }
  async function generate() {
    if (!plan || dirty || active.current) return;
    const current = version.current;
    active.current = true;
    setBusy("generate");
    setNotice(null);
    try {
      const generated = await runProviderSmokeTest(
        model,
        profile.id,
        plan.fingerprint,
        crypto.randomUUID(),
      );
      if (current === version.current) {
        setConversationId(generated.conversation_id);
        setNotice(
          text(
            "completed",
            "Test image saved in a separate connection-test conversation.",
          ),
        );
      }
    } catch {
      if (current === version.current)
        setNotice(
          text(
            "unknownOutcome",
            "The test did not return a completed image. Its outcome may be unknown and a charge may still apply. Check your connection-test conversation and provider usage before trying again. No automatic retry was made.",
          ),
        );
    } finally {
      if (current === version.current) {
        active.current = false;
        setBusy(null);
        setPlan(null);
      }
    }
  }
  const stages = [
    "local",
    "transport",
    "authentication",
    "model",
    "generation",
  ] as const;
  return (
    <section
      aria-label={text("title", "Connection diagnostics")}
      className="space-y-3 border-t border-border-subtle pt-4"
    >
      <div>
        <h5 className="text-xs font-semibold text-foreground">
          {text("title", "Connection diagnostics")}
        </h5>
        <p className="mt-1 text-xs leading-relaxed text-muted">
          {text(
            "intro",
            "A read-only check verifies the saved endpoint. Image generation is a separate, optional test.",
          )}
        </p>
      </div>
      <div className="flex flex-wrap gap-2">
        <button
          type="button"
          className={buttonClass}
          disabled={dirty || !!busy}
          onClick={() => void run("diagnose")}
        >
          {busy === "diagnose"
            ? text("checking", "Checking…")
            : text("diagnose", "Diagnose connection")}
        </button>
        <button
          type="button"
          className={buttonClass}
          disabled={dirty || !!busy}
          onClick={() => void run("plan")}
        >
          {busy === "plan"
            ? text("preparing", "Preparing…")
            : text("smoke", "Test one image…")}
        </button>
      </div>
      {dirty && (
        <p className="text-xs text-muted">
          {text("saveFirst", "Save your changes before testing this provider.")}
        </p>
      )}
      {result && (
        <dl className="divide-y divide-border-subtle" aria-live="polite">
          {stages.map((stage) => {
            const item = result[stage];
            const Icon =
              item.status === "passed"
                ? Check
                : item.status === "failed"
                  ? X
                  : CircleHelp;
            return (
              <div
                key={stage}
                className="grid grid-cols-[minmax(100px,1fr)_2fr] items-start gap-3 py-2 text-xs"
              >
                <dt className="text-foreground">{text(stage, stage)}</dt>
                <dd className="flex items-start gap-2 text-muted">
                  <Icon
                    aria-hidden
                    size={14}
                    className={`mt-0.5 shrink-0 ${item.status === "passed" ? "text-success" : item.status === "failed" ? "text-error" : ""}`}
                  />
                  <span>
                    {text(`codes.${item.code}`, item.code.replace(/_/g, " "))} ·{" "}
                    {text(`status.${item.status}`, item.status)}
                  </span>
                </dd>
              </div>
            );
          })}
        </dl>
      )}
      {notice && (
        <p role="status" className="text-xs leading-relaxed text-muted">
          {notice}
        </p>
      )}
      {conversationId && (
        <Link
          className="focus-ring inline-block text-xs text-primary underline underline-offset-4"
          to={`/projects/default/chat/${conversationId}`}
        >
          {text("openConversation", "Open test conversation")}
        </Link>
      )}
      {plan && (
        <div
          className="fixed inset-0 z-[60] flex items-center justify-center bg-black/45 p-4"
          onClick={closePlan}
        >
          <div
            ref={dialogRef}
            tabIndex={-1}
            role="dialog"
            aria-modal="true"
            aria-labelledby={titleId}
            aria-busy={busy === "generate"}
            onClick={(event) => event.stopPropagation()}
            className="studio-floating-panel max-h-[calc(100dvh-2rem)] w-full max-w-lg overflow-y-auto rounded-xl p-6 outline-none"
          >
            <h2
              id={titleId}
              className="text-base font-semibold text-foreground"
            >
              {text("confirmTitle", "Generate one connection-test image?")}
            </h2>
            <p className="mt-3 text-sm leading-relaxed text-muted">
              {text(
                "cost",
                "This sends one real image request and may incur a charge. Pricing is set by your provider. The result is saved in a separate connection-test conversation.",
              )}
            </p>
            <dl className="mt-4 space-y-2 break-words text-sm text-foreground">
              <div>
                <dt className="text-xs text-muted">
                  {text("provider", "Provider / model")}
                </dt>
                <dd>
                  {plan.provider_name} · {plan.endpoint_host}
                  <br />
                  {plan.model}
                </dd>
              </div>
              <div>
                <dt className="text-xs text-muted">
                  {text("parameters", "Exact test parameters")}
                </dt>
                <dd>
                  {[
                    `${plan.image_count} ${text("image", "image")}`,
                    plan.size,
                    plan.resolution,
                    plan.aspect_ratio,
                    plan.quality,
                    plan.output_format,
                  ]
                    .filter(Boolean)
                    .join(" · ")}
                </dd>
              </div>
              <div>
                <dt className="text-xs text-muted">
                  {text("prompt", "Prompt")}
                </dt>
                <dd>{plan.prompt}</dd>
              </div>
            </dl>
            <p className="mt-4 text-xs leading-relaxed text-muted">
              {text(
                "once",
                "One request only. No automatic retry. If the outcome is unknown, check provider usage before testing again.",
              )}
            </p>
            <div className="mt-5 flex flex-wrap justify-end gap-2">
              <button
                type="button"
                className={buttonClass}
                disabled={busy === "generate"}
                onClick={closePlan}
              >
                {text("cancel", "Cancel")}
              </button>
              <button
                type="button"
                className={buttonClass}
                disabled={busy === "generate"}
                onClick={() => void generate()}
              >
                {busy === "generate" ? (
                  <span className="flex items-center gap-2">
                    <Loader2
                      size={14}
                      aria-hidden
                      className="animate-spin motion-reduce:animate-none"
                    />
                    {text("generating", "Generating…")}
                  </span>
                ) : (
                  text("confirm", "Generate one test image")
                )}
              </button>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}
