import { useEffect, useId, useRef, useState } from "react";

import { joinWaitlist } from "@/data/waitlistApi";
import {
  WAITLIST_ERROR_MESSAGES,
  MAX_EMAIL_LENGTH,
  isValidWaitlistEmail,
} from "@/domain/waitlist";
import { Button } from "@/components/ui/Button";

type Phase = "form" | "sending" | "joined" | "duplicate";

export default function WaitlistForm() {
  const emailId = useId();
  const errorId = `${emailId}-error`;
  const emailInputRef = useRef<HTMLInputElement>(null);
  const statusRef = useRef<HTMLParagraphElement>(null);
  const [email, setEmail] = useState("");
  const [website, setWebsite] = useState("");
  const [phase, setPhase] = useState<Phase>("form");
  const [errorMessage, setErrorMessage] = useState<string | undefined>(undefined);

  useEffect(() => {
    const status = new URLSearchParams(window.location.search).get("waitlist");
    if (!status) return;
    if (status === "ok") {
      setPhase("joined");
    } else if (status === "invalid") {
      setErrorMessage(WAITLIST_ERROR_MESSAGES.invalid_email);
    } else if (status === "rate_limited") {
      setErrorMessage(WAITLIST_ERROR_MESSAGES.rate_limited);
    } else if (status === "error") {
      setErrorMessage(WAITLIST_ERROR_MESSAGES.invalid_body);
    }
  }, []);

  useEffect(() => {
    if (phase === "joined" || phase === "duplicate") statusRef.current?.focus();
  }, [phase]);

  async function join(event: { preventDefault: () => void }) {
    event.preventDefault();
    if (phase !== "form") return;

    if (!isValidWaitlistEmail(email)) {
      setErrorMessage(WAITLIST_ERROR_MESSAGES.invalid_email);
      emailInputRef.current?.focus();
      return;
    }

    setPhase("sending");
    setErrorMessage(undefined);

    try {
      const data = await joinWaitlist({ email, website, source: "hero" });
      if (!data.ok) {
        setErrorMessage(
          WAITLIST_ERROR_MESSAGES[data.error ?? ""] ??
            "Gagal menyimpan pendaftaran. Coba lagi sebentar.",
        );
        setPhase("form");
        return;
      }

      setEmail("");
      setPhase(data.token ? "joined" : "duplicate");
    } catch {
      setErrorMessage("Koneksi terputus saat mengirim. Periksa jaringan lalu coba lagi.");
      setPhase("form");
    }
  }

  if (phase === "joined" || phase === "duplicate") {
    return (
      <div className="w-full text-left">
        <p ref={statusRef} tabIndex={-1} role="status" className="text-sm font-medium text-ink outline-none">
          {phase === "duplicate" ? "Email ini sudah ada di waitlist" : "Kamu masuk waitlist"}
        </p>
        <p className="mt-2 text-sm leading-relaxed text-ink-soft">
          Kami kabari saat beta siap. Pendaftaran belum langsung memberi akses.
        </p>
      </div>
    );
  }

  return (
    <form
      method="post"
      action="/api/waitlist"
      onSubmit={join}
      noValidate
      className="w-full"
    >
      <label htmlFor={emailId} className="sr-only">Email</label>
      <div className={`flex min-w-0 items-center gap-1 rounded-full border bg-surface-subtle p-1.5 transition-colors focus-within:border-accent focus-within:ring-2 focus-within:ring-accent/20 ${errorMessage ? "border-danger" : "border-line-strong"}`}>
        <input
          ref={emailInputRef}
          id={emailId}
          name="email"
          type="email"
          autoComplete="email"
          maxLength={MAX_EMAIL_LENGTH}
          placeholder="nama@email.com"
          className="h-11 min-w-0 flex-1 rounded-full bg-transparent px-3 text-base text-ink outline-none placeholder:text-muted"
          aria-invalid={Boolean(errorMessage)}
          aria-describedby={errorMessage ? errorId : undefined}
          value={email}
          readOnly={phase === "sending"}
          onChange={(event) => {
            setEmail(event.target.value);
            if (errorMessage) setErrorMessage(undefined);
          }}
          required
        />
        <Button
          type="submit"
          variant="primary"
          size="md"
          loading={phase === "sending"}
          disabled={phase === "sending"}
          aria-label="Join Beta"
          className="w-28 shrink-0 rounded-full!"
        >
          Join Beta
        </Button>
      </div>
      <input
        type="text"
        name="website"
        tabIndex={-1}
        autoComplete="off"
        value={website}
        onChange={(event) => setWebsite(event.target.value)}
        aria-hidden="true"
        className="sr-only"
      />
      {errorMessage ? <p id={errorId} role="alert" className="mt-2 text-xs leading-relaxed text-danger">{errorMessage}</p> : null}
    </form>
  );
}
