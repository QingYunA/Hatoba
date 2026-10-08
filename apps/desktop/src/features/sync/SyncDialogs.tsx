import { useEffect, useId, useRef, useState } from "react";
import { Button, Checkbox, LinkButton, TextField } from "@/components/controls";
import { Dialog, toast } from "@/components/overlay";
import { errorMessage } from "@/app/errors";
import { Field } from "@/features/keys/Field";
import { copyText } from "@/features/keys/clipboard";
import { useT } from "@/i18n";
import { api } from "@/ipc/api";
import s from "./Dialogs.module.css";

/** Settings → Security's "generate a new recovery code", reachable from the sync page too. */
export function RecoveryCodeDialog({ onClose }: { onClose: () => void }) {
  const t = useT();
  const fieldId = useId();
  const formId = useId();
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [code, setCode] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

  const generate = async () => {
    if (!password || busy) return;
    setBusy(true);
    setError(null);
    try {
      setCode(await api.vault_rotate_recovery(password));
    } catch (e) {
      setError(errorMessage(t, e));
      setBusy(false);
    }
  };

  const copy = async () => {
    try {
      await copyText(code ?? "");
      toast(t("btn.copied"), "success");
    } catch {
      toast(t("err.internal"), "error");
    }
  };

  if (code) {
    // The code is shown once and the old one is already void: closing is only possible after confirming.
    return (
      <Dialog
        title={t("sync.rotate.newTitle")}
        icon="key"
        body={t("sync.rotate.newBody")}
        actions={
          <Button variant="primary" disabled={!saved} onClick={onClose}>
            {t("btn.done")}
          </Button>
        }
      >
        <div className={`${s.codeBox} selectable`}>
          {code.split("-").map((group, i) => (
            <span key={i} className={s.codeCell}>
              <span className={s.codeIndex}>{i + 1}</span>
              <span className={s.codeText}>{group}</span>
            </span>
          ))}
        </div>
        <div className={s.codeActions}>
          <LinkButton icon="copy" onClick={() => void copy()}>
            {t("btn.copy")}
          </LinkButton>
        </div>
        <div className={s.saved}>
          <Checkbox checked={saved} onChange={setSaved}>
            {t("sync.rotate.saved")}
          </Checkbox>
        </div>
      </Dialog>
    );
  }

  return (
    <Dialog
      title={t("sync.rotate.title")}
      icon="key"
      body={t("sync.rotate.body")}
      onClose={busy ? undefined : onClose}
      actions={
        <>
          <Button onClick={onClose} disabled={busy}>
            {t("btn.cancel")}
          </Button>
          <Button variant="primary" type="submit" form={formId} busy={busy} disabled={!password}>
            {t("sync.rotate.action")}
          </Button>
        </>
      }
    >
      <form
        id={formId}
        className={s.form}
        onSubmit={(e) => {
          e.preventDefault();
          void generate();
        }}
      >
        <Field label={t("sync.masterPassword")} htmlFor={fieldId} error={error}>
          <TextField
            id={fieldId}
            secret
            large
            autoFocus
            value={password}
            invalid={!!error}
            autoComplete="current-password"
            onChange={(e) => {
              setPassword(e.target.value);
              setError(null);
            }}
          />
        </Field>
      </form>
    </Dialog>
  );
}

const MIN_PASSWORD = 8;

export function ChangePasswordDialog({ onClose }: { onClose: () => void }) {
  const t = useT();
  const currentId = useId();
  const nextId = useId();
  const confirmId = useId();
  const formId = useId();
  const currentRef = useRef<HTMLInputElement>(null);
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<{ field: "current" | "next" | "confirm"; message: string } | null>(null);

  useEffect(() => {
    if (error?.field === "current") currentRef.current?.focus();
  }, [error]);

  const submit = async () => {
    if (busy) return;
    if (next.length < MIN_PASSWORD) return setError({ field: "next", message: t("sync.password.short") });
    if (next !== confirmation) return setError({ field: "confirm", message: t("sync.password.mismatch") });
    setBusy(true);
    setError(null);
    try {
      await api.vault_change_password(current, next);
      toast(t("sync.password.done"), "success");
      onClose();
    } catch (e) {
      const wrong = (e as { code?: string })?.code === "wrong_password";
      setError({ field: wrong ? "current" : "next", message: errorMessage(t, e) });
      setBusy(false);
    }
  };

  const clear = <T,>(set: (v: T) => void) => (v: T) => {
    set(v);
    setError(null);
  };

  return (
    <Dialog
      title={t("sync.password.title")}
      icon="lock-key"
      body={t("sync.password.body")}
      onClose={busy ? undefined : onClose}
      actions={
        <>
          <Button onClick={onClose} disabled={busy}>
            {t("btn.cancel")}
          </Button>
          <Button variant="primary" type="submit" form={formId} busy={busy} disabled={!current || !next || !confirmation}>
            {t("btn.save")}
          </Button>
        </>
      }
    >
      <form
        id={formId}
        className={s.form}
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        <Field label={t("sync.password.current")} htmlFor={currentId} error={error?.field === "current" ? error.message : undefined}>
          <TextField
            id={currentId}
            ref={currentRef}
            secret
            autoFocus
            value={current}
            invalid={error?.field === "current"}
            autoComplete="current-password"
            onChange={(e) => clear(setCurrent)(e.target.value)}
          />
        </Field>
        <Field label={t("sync.password.new")} htmlFor={nextId} error={error?.field === "next" ? error.message : undefined}>
          <TextField
            id={nextId}
            secret
            value={next}
            invalid={error?.field === "next"}
            autoComplete="new-password"
            onChange={(e) => clear(setNext)(e.target.value)}
          />
        </Field>
        <Field label={t("sync.password.confirm")} htmlFor={confirmId} error={error?.field === "confirm" ? error.message : undefined}>
          <TextField
            id={confirmId}
            secret
            value={confirmation}
            invalid={error?.field === "confirm"}
            autoComplete="new-password"
            onChange={(e) => clear(setConfirmation)(e.target.value)}
          />
        </Field>
      </form>
    </Dialog>
  );
}
