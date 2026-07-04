/**
 * Account pane — sign-in status (Issue #40 review #2).
 * Mock Google sign-in: toggle a local useState to simulate login/logout.
 * Displays current BYOK provider and signed-in email when logged in.
 */

import { useState } from "react";
import { LogIn } from "lucide-react";
import { Icon } from "../../ui/Icon";
import { useT } from "../../../i18n";
import { useSettingsStore } from "../../../store/settingsStore";
import { Section, Field, Value } from "../shared";

const MOCK_EMAIL = "user@example.com";

export function AccountPane() {
  const t = useT();
  const provider = useSettingsStore((s) => s.byokProvider);
  const [signedIn, setSignedIn] = useState(false);

  const toggle = () => setSignedIn((v) => !v);

  return (
    <Section title={t("settings.section.account")}>
      <Field
        label={t("settings.accountSignIn")}
        description={
          signedIn
            ? `${t("settings.accountSignedInAs")} ${MOCK_EMAIL}`
            : t("settings.accountNotSignedIn")
        }
        control={
          <button
            type="button"
            onClick={toggle}
            className="hover-area"
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: 6,
              height: 28,
              padding: signedIn ? "0 var(--space-md)" : "0 var(--space-lg)",
              borderRadius: "var(--radius-sm)",
              border: "var(--bw-thin) solid var(--border-primary)",
              color: signedIn ? "var(--text-secondary)" : "var(--text-primary)",
              background: signedIn ? "transparent" : "var(--accent-primary)",
              fontSize: "var(--fs-sm)",
              fontWeight: "var(--fw-medium)",
            }}
          >
            {!signedIn && <Icon icon={LogIn} size={14} />}
            {signedIn ? t("settings.accountSignOut") : t("settings.accountSignIn")}
          </button>
        }
      />
      <Field
        label={t("settings.byokProvider")}
        control={<Value>{provider}</Value>}
      />
    </Section>
  );
}