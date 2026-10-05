import { ArrowRight, Check, Command, Cpu, Keyboard, Mic, ShieldCheck, Sparkles } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState, type ReactNode } from "react";
import { api, type Hotkey, type ModelStatus, type Permissions, IS_MAC, PLATFORM, THIS_DEVICE } from "../lib/api";
import { useDictation } from "../lib/hooks";
import { cx } from "../lib/utils";
import { Button, HotkeyPicker, Keys, Logo } from "./ui";

const STEP_COUNT = 6;

export default function Onboarding({
  hotkey,
  model,
  permissions,
  onHotkeyChange,
  onFinish,
}: {
  hotkey: Hotkey;
  model: ModelStatus | null;
  permissions: Permissions | null;
  onHotkeyChange: (hotkey: Hotkey) => void;
  onFinish: () => void;
}) {
  const [step, setStep] = useState(0);
  const [tried, setTried] = useState(false);
  const { phase } = useDictation((event) => {
    if ((event.phase === "done" || event.phase === "copied") && event.text) setTried(true);
  });
  const next = () => setStep((current) => Math.min(current + 1, STEP_COUNT - 1));

  useEffect(() => {
    if (step === 3 && model?.state === "error") api.prepareModel();
  }, [step, model?.state]);

  const microphone = permissions?.microphone;
  const percent = Math.round((model?.progress ?? 0) * 100);

  return (
    <div className="relative flex h-full flex-col items-center justify-center overflow-hidden px-6">
      <div className="absolute inset-x-0 top-0 h-11" data-tauri-drag-region />
      <div className="pointer-events-none absolute -top-48 left-1/2 size-[560px] -translate-x-1/2 rounded-full bg-white/[0.045] blur-3xl" />

      <div className="relative w-full max-w-[440px]">
        <AnimatePresence mode="wait">
          <motion.div
            key={step}
            initial={{ opacity: 0, x: 24 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -24 }}
            transition={{ duration: 0.22, ease: "easeOut" }}
          >
            {step === 0 && (
              <Step
                icon={<Logo size={56} />}
                title="Talk, don't type."
                body={`Parla turns your voice into text in any app, instantly. Everything runs on ${THIS_DEVICE} — your words never leave it.`}
              >
                <Button className="h-10 w-full" onClick={next}>
                  Get started <ArrowRight className="size-4" />
                </Button>
              </Step>
            )}

            {step === 1 && (
              <Step
                icon={<IconBadge><Mic className="size-6" /></IconBadge>}
                title="Allow your microphone"
                body="Parla only listens while you hold the shortcut. Audio is transcribed locally and never stored."
              >
                {microphone === "granted" ? (
                  <Granted label="Microphone access granted" />
                ) : (
                  <Button
                    variant="secondary"
                    className="h-10 w-full"
                    onClick={() =>
                      microphone === "denied" ? api.openPrivacySettings("microphone") : api.requestMicrophone()
                    }
                  >
                    {microphone === "denied" ? (IS_MAC ? "Open System Settings" : "Open privacy settings") : "Allow microphone"}
                  </Button>
                )}
                <Button className="h-10 w-full" disabled={microphone !== "granted"} onClick={next}>
                  Continue
                </Button>
              </Step>
            )}

            {step === 2 && !IS_MAC && (
              <Step
                icon={<IconBadge><Keyboard className="size-6" /></IconBadge>}
                title="Parla types for you"
                body={
                  PLATFORM === "linux"
                    ? "Parla types your words into whatever app you're using. On Linux that needs an X11 session; under Wayland your dictations are copied, ready to paste."
                    : "Parla types your words into whatever app you're using. Nothing to switch on."
                }
              >
                <Button className="h-10 w-full" onClick={next}>
                  Continue
                </Button>
              </Step>
            )}

            {step === 2 && IS_MAC && (
              <Step
                icon={<IconBadge><Keyboard className="size-6" /></IconBadge>}
                title="Let Parla type for you"
                body="Accessibility permission lets Parla paste your words into whatever app you're using. Click Grant access, switch Parla on in System Settings, then come back."
              >
                {permissions?.accessibility ? (
                  <Granted label="Accessibility enabled" />
                ) : (
                  <Button variant="secondary" className="h-10 w-full" onClick={api.grantAccessibility}>
                    Grant access
                  </Button>
                )}
                <Button className="h-10 w-full" onClick={next} variant={permissions?.accessibility ? "primary" : "ghost"}>
                  {permissions?.accessibility ? "Continue" : "Skip for now"}
                </Button>
              </Step>
            )}

            {step === 3 && (
              <Step
                icon={<IconBadge><Cpu className="size-6" /></IconBadge>}
                title="Setting up the speech model"
                body={IS_MAC ? "A one-time download of a compact speech model that runs on your Mac's Neural Engine. After this, Parla works fully offline." : "A one-time download of the speech model (about 670 MB) and the Enhance model (about 1.1 GB). After this, Parla works fully offline."}
              >
                <div className="rounded-xl border border-line bg-surface p-4">
                  {model?.state === "ready" ? (
                    <Granted label="Model ready" bare />
                  ) : model?.state === "error" ? (
                    <p className="text-[13px] text-danger">{model.message ?? "Something went wrong."}</p>
                  ) : (
                    <>
                      <div className="flex justify-between text-[12px] text-muted">
                        <span className="truncate pr-4">{model?.message ?? "Starting…"}</span>
                        <span className="tabular-nums">{percent}%</span>
                      </div>
                      <div className="mt-2.5 h-1.5 overflow-hidden rounded-full bg-raised">
                        <motion.div className="h-full rounded-full bg-fg" animate={{ width: `${Math.max(percent, 4)}%` }} />
                      </div>
                    </>
                  )}
                </div>
                <Button className="h-10 w-full" disabled={model?.state !== "ready"} onClick={next}>
                  Continue
                </Button>
              </Step>
            )}

            {step === 4 && (
              <Step
                icon={<IconBadge><Command className="size-6" /></IconBadge>}
                title="Pick your dictation key"
                body="Hold it to talk, let go to type. Using it in shortcuts with other keys keeps working as usual."
              >
                <HotkeyPicker value={hotkey} onChange={onHotkeyChange} />
                <Button className="h-10 w-full" onClick={next}>
                  Continue
                </Button>
              </Step>
            )}

            {step === 5 && (
              <Step
                icon={<IconBadge><Sparkles className="size-6" /></IconBadge>}
                title="Give it a try"
                body="Click the box, hold your key, say a sentence, then let go."
              >
                <div className="flex justify-center py-1">
                  <Keys hotkey={hotkey} size="lg" active={phase === "recording"} />
                </div>
                <textarea
                  autoFocus
                  placeholder={phase === "recording" ? "Listening…" : "Your words will appear here"}
                  className={cx(
                    "selectable h-28 w-full resize-none rounded-xl border bg-surface p-3.5 text-[14px] outline-none transition-colors",
                    phase === "recording" ? "border-fg/50" : "border-line focus:border-fg/40",
                  )}
                />
                <Button className="h-10 w-full" onClick={onFinish}>
                  {tried ? (
                    <>
                      <Check className="size-4" /> Looks great — finish
                    </>
                  ) : (
                    "Finish setup"
                  )}
                </Button>
              </Step>
            )}
          </motion.div>
        </AnimatePresence>

        <div className="mt-8 flex justify-center gap-1.5">
          {Array.from({ length: STEP_COUNT }, (_, index) => (
            <motion.span
              key={index}
              className={cx("h-1.5 rounded-full", index <= step ? "bg-fg" : "bg-line")}
              animate={{ width: index === step ? 20 : 6 }}
            />
          ))}
        </div>
      </div>
    </div>
  );
}

function Step({ icon, title, body, children }: { icon: ReactNode; title: string; body: string; children: ReactNode }) {
  return (
    <div className="flex flex-col items-center text-center">
      {icon}
      <h1 className="mt-6 text-[26px] font-semibold tracking-tight">{title}</h1>
      <p className="mt-2 max-w-[380px] text-[14px] leading-relaxed text-muted">{body}</p>
      <div className="mt-8 flex w-full flex-col gap-2.5">{children}</div>
    </div>
  );
}

function IconBadge({ children }: { children: ReactNode }) {
  return <div className="grid size-14 place-items-center rounded-2xl bg-accent-soft text-accent">{children}</div>;
}

function Granted({ label, bare = false }: { label: string; bare?: boolean }) {
  return (
    <div
      className={cx(
        "flex items-center justify-center gap-2 text-[13px] font-medium text-ok",
        !bare && "h-10 rounded-lg border border-line bg-surface",
      )}
    >
      <ShieldCheck className="size-4" /> {label}
    </div>
  );
}
