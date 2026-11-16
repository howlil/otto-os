import { useEffect, useRef, useState } from "react";

import { joinWaitlist, saveWaitlistProfile } from "@/data/waitlistApi";
import {
  WAITLIST_ERROR_MESSAGES,
  WAITLIST_PLATFORMS,
  WAITLIST_USAGES,
  isValidWaitlistEmail,
} from "@/domain/waitlist";
import { Button } from "@/components/ui/Button";
import { Input } from "@/components/ui/Input";
import { Select } from "@/components/ui/Select";

type Phase =
  | "form"
  | "sending"
  | "joined"
  | "saving-profile"
  | "profiled"
  | "duplicate"
  | "skipped";

const PLATFORM_OPTIONS = WAITLIST_PLATFORMS.map((value) => ({ value, label: value }));
const USAGE_OPTIONS = WAITLIST_USAGES.map((value) => ({ value, label: value }));

export default function WaitlistForm({ modal = false }: { modal?: boolean } = {}) {
  const emailInputRef = useRef<HTMLInputElement>(null);
  const [email, setEmail] = useState("");
  const [website, setWebsite] = useState("");
  const [phase, setPhase] = useState<Phase>("form");
  const [joinedEmail, setJoinedEmail] = useState("");
  const [token, setToken] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | undefined>(undefined);
  const [windowsVersion, setWindowsVersion] = useState("");
  const [usage, setUsage] = useState("");

  useEffect(() => {
    const status = new URLSearchParams(window.location.search).get("waitlist");
    if (!status) return;
    if (status === "ok") {
      setPhase("duplicate");
    } else if (status === "invalid") {
      setErrorMessage(WAITLIST_ERROR_MESSAGES.invalid_email);
    } else if (status === "rate_limited") {
      setErrorMessage(WAITLIST_ERROR_MESSAGES.rate_limited);
    } else if (status === "error") {
      setErrorMessage(WAITLIST_ERROR_MESSAGES.invalid_body);
    }
  }, []);

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

      setJoinedEmail(email.trim().toLowerCase());
      setEmail("");

      if (data.token) {
        setToken(data.token);
        setPhase("joined");
      } else {
        setToken(null);
        setPhase("duplicate");
      }
    } catch {
      setErrorMessage("Koneksi terputus saat mengirim. Periksa jaringan lalu coba lagi.");
      setPhase("form");
    }
  }

  async function saveProfile(event: { preventDefault: () => void }) {
    event.preventDefault();
    if (!token || phase !== "joined") return;

    if (!windowsVersion || !usage) {
      setErrorMessage(WAITLIST_ERROR_MESSAGES.invalid_profile);
      return;
    }

    setPhase("saving-profile");
    setErrorMessage(undefined);

    try {
      const data = await saveWaitlistProfile({
        email: joinedEmail,
        token,
        platform: windowsVersion,
        usage,
      });

      if (!data.ok) {
        setErrorMessage(
          WAITLIST_ERROR_MESSAGES[data.error ?? ""] ??
            "Profil belum tersimpan. Coba lagi atau lewati langkah ini.",
        );
        setPhase("joined");
        return;
      }

      setToken(null);
      setPhase("profiled");
    } catch {
      setErrorMessage("Koneksi terputus. Coba simpan ulang atau klik Lewati.");
      setPhase("joined");
    }
  }

  function skipProfile() {
    setToken(null);
    setErrorMessage(undefined);
    setPhase("skipped");
  }

  if (phase !== "form" && phase !== "sending") {
    const canEditProfile = phase === "joined" || phase === "saving-profile";
    const savingProfile = phase === "saving-profile";

    const heading =
      phase === "duplicate"
        ? "Email ini sudah ada di waitlist"
        : "Kamu masuk waitlist";

    const description =
      phase === "profiled"
        ? "Profil tersimpan. Undangan akan dikirim ke email ini saat gelombang beta dibuka."
        : phase === "skipped"
          ? "Pendaftaran selesai. Kami kabari lewat email saat beta dibuka."
          : phase === "duplicate"
            ? "Undangan beta akan dikirim ke alamat ini begitu giliran gelombang berikutnya dibuka."
            : "Undangan akan dikirim ke email ini saat batch beta berikutnya dibuka. Isi dua info singkat ini jika ingin membantu pemilihan tester (opsional).";

    return (
      <div className={modal ? "w-full text-left" : "w-full max-w-md rounded-2xl border border-line bg-surface p-5 text-left shadow-[0_8px_24px_rgb(15_17_21/0.06)] sm:p-6"}>
        <p role="status" className="text-base font-medium text-ink">
          {heading}
        </p>
        <p aria-live="polite" className="mt-1.5 text-sm leading-relaxed text-ink-soft">
          {description}
        </p>

        {canEditProfile ? (
          <form onSubmit={saveProfile} noValidate className="mt-5 flex flex-col gap-4">
            <Select
              label="Versi Windows kamu"
              placeholder="Pilih versi Windows"
              options={PLATFORM_OPTIONS}
              value={windowsVersion}
              disabled={savingProfile}
              onChange={(event) => {
                setWindowsVersion(event.target.value);
                if (errorMessage) setErrorMessage(undefined);
              }}
            />
            <Select
              label="Seberapa sering kamu berpindah banyak aplikasi?"
              placeholder="Pilih frekuensi"
              options={USAGE_OPTIONS}
              value={usage}
              disabled={savingProfile}
              onChange={(event) => {
                setUsage(event.target.value);
                if (errorMessage) setErrorMessage(undefined);
              }}
            />
            {errorMessage ? (
              <p role="alert" className="text-xs leading-relaxed text-danger">
                {errorMessage}
              </p>
            ) : null}
            <div className="flex flex-col-reverse gap-2 pt-1 sm:flex-row sm:justify-end">
              <Button
                type="button"
                variant="ghost"
                size="md"
                className={modal ? "rounded-full" : undefined}
                disabled={savingProfile}
                onClick={skipProfile}
              >
                Lewati
              </Button>
              <Button type="submit" variant="primary" size="md" loading={savingProfile} className={modal ? "rounded-full" : undefined}>
                Simpan profil
              </Button>
            </div>
          </form>
        ) : null}
      </div>
    );
  }

  return (
    <form
      method="post"
      action="/api/waitlist"
      onSubmit={join}
      noValidate
      className={modal ? "flex w-full flex-col gap-3" : "flex w-full max-w-md flex-col items-stretch gap-3 sm:flex-row sm:items-start"}
    >
      <div className="min-w-0 flex-1">
        <Input
          ref={emailInputRef}
          label="Email"
          hideLabel
          name="email"
          type="email"
          autoComplete="email"
          placeholder="nama@email.com"
          className={modal ? "rounded-full" : undefined}
          value={email}
          readOnly={phase === "sending"}
          onChange={(event) => {
            setEmail(event.target.value);
            if (errorMessage) setErrorMessage(undefined);
          }}
          error={errorMessage}
          required
        />
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
      <Button
        type="submit"
        variant="primary"
        size="lg"
        loading={phase === "sending"}
        className={modal ? "w-full shrink-0 rounded-full" : "w-full shrink-0 sm:w-auto"}
      >
        Daftar beta
      </Button>
    </form>
  );
}
