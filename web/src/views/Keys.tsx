import { createSignal, For, Show } from 'solid-js';
import { adminToken, mintKey, revokeKey, setAdminToken, type MintedKey } from '../api';

/// The only place the admin token is entered. It is kept in sessionStorage
/// (see api.ts) so closing the tab forgets it; the reader views never render
/// this field and stay token-free.
export function TokenField() {
  const [token, setToken] = createSignal(adminToken());
  return (
    <label class="tokenbar">
      <span>admin token</span>
      <input
        type="password"
        value={token()}
        autocomplete="off"
        placeholder="Bearer token for management"
        onInput={(event) => {
          const next = event.currentTarget.value.trim();
          setToken(event.currentTarget.value);
          setAdminToken(next);
        }}
      />
    </label>
  );
}

const SCOPES = ['read:private', 'write:private', 'bookmarks:write'];

/// Mint scoped keys and revoke them. The plaintext is shown exactly once,
/// with a copy button, because the service stores only its hash.
export function KeysView() {
  const [name, setName] = createSignal('');
  const [scopes, setScopes] = createSignal<string[]>(['read:private']);
  const [minted, setMinted] = createSignal<MintedKey[]>([]);
  const [latest, setLatest] = createSignal<MintedKey | undefined>();
  const [notice, setNotice] = createSignal('');
  const [copied, setCopied] = createSignal(false);

  const toggleScope = (scope: string) => {
    setScopes((current) =>
      current.includes(scope) ? current.filter((value) => value !== scope) : [...current, scope],
    );
  };

  const mint = async (event: Event) => {
    event.preventDefault();
    setNotice('');
    setCopied(false);
    try {
      const key = await mintKey(name().trim(), scopes());
      setMinted((current) => [key, ...current]);
      setLatest(key);
      setName('');
      setNotice(`minted ${key.name} (${key.prefix}…).`);
    } catch (failure) {
      setNotice((failure as Error).message);
    }
  };

  const revoke = async (id: string) => {
    setNotice('');
    try {
      await revokeKey(id);
      setMinted((current) => current.filter((key) => key.id !== id));
      if (latest()?.id === id) setLatest(undefined);
      setNotice('revoked.');
    } catch (failure) {
      setNotice((failure as Error).message);
    }
  };

  const copy = async () => {
    const token = latest()?.token;
    if (!token) return;
    try {
      await navigator.clipboard.writeText(token);
      setCopied(true);
    } catch {
      setNotice('copy failed: select the token manually.');
    }
  };

  return (
    <div class="split">
      <section class="list manage">
        <TokenField />
        <form class="manage-form" onSubmit={mint}>
          <p class="side-label">mint a key</p>
          <input
            class="manage-input"
            value={name()}
            placeholder="key name, e.g. laptop-reader"
            aria-label="key name"
            onInput={(event) => setName(event.currentTarget.value)}
          />
          <div class="scope-row">
            <For each={SCOPES}>
              {(scope) => (
                <label class="check">
                  <input
                    type="checkbox"
                    checked={scopes().includes(scope)}
                    onChange={() => toggleScope(scope)}
                  />
                  <code>{scope}</code>
                </label>
              )}
            </For>
          </div>
          <button type="submit" class="manage-btn" disabled={!name().trim() || !scopes().length}>
            mint key
          </button>
        </form>
        <Show when={latest()}>
          {(key) => (
            <div class="once">
              <p class="side-label">shown once — copy it now</p>
              <p class="once-token">{key().token}</p>
              <button type="button" class="manage-btn" onClick={copy}>
                {copied() ? 'copied' : 'copy'}
              </button>
            </div>
          )}
        </Show>
        <Show when={notice()}>
          <p class="note">{notice()}</p>
        </Show>
        <Show when={minted().length}>
          <p class="side-label">minted this session</p>
          <ul class="manage-list">
            <For each={minted()}>
              {(key) => (
                <li>
                  <span>
                    {key.name} <code>{key.prefix}…</code>
                  </span>
                  <button type="button" class="opt" onClick={() => void revoke(key.id)}>
                    revoke
                  </button>
                </li>
              )}
            </For>
          </ul>
        </Show>
      </section>
    </div>
  );
}
