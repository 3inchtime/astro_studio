import type { CredentialUpdate } from "../types";

type Draft = { api_key?: string; api_key_configured?: boolean; api_key_masked?: string | null; credential_update?: CredentialUpdate };
/** Never retain a persisted secret or a previous replacement in query/form state. */
export function credentialDraft<T extends Draft>(value: T): T & { api_key: string } {
  return { ...value, api_key: "", credential_update: undefined };
}
export function credentialWrite<T extends Draft>(value: T) {
  const { api_key, api_key_configured: _configured, api_key_masked: _masked, credential_update, ...metadata } = value;
  const update: CredentialUpdate = credential_update?.action === "clear" ? credential_update
    : api_key?.trim() ? { action: "replace", value: api_key } : { action: "keep" };
  return { ...metadata, credential_update: update };
}
