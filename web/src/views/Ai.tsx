import { createResource, Show } from 'solid-js';
import { aiStatus } from '../api';

/// Live enrichment and embedding providers plus their backlogs. The endpoint
/// is public and always 200, so this view renders the same shape whether or
/// not any model is configured.
export function AiView() {
  const [status, { refetch }] = createResource(aiStatus);
  return (
    <div class="split">
      <section class="list manage">
        <div class="manage-head">
          <p class="side-label">ai status</p>
          <button type="button" class="opt" onClick={() => void refetch()}>
            refresh
          </button>
        </div>
        <Show
          when={status()}
          fallback={<p class="note">loading provider status…</p>}
        >
          {(value) => (
            <>
              <dl class="status-grid">
                <dt>enrich provider</dt>
                <dd>{value().enrich.provider}</dd>
                <dt>enrich model</dt>
                <dd>{value().enrich.model ?? '—'}</dd>
                <dt>enrich backlog</dt>
                <dd>{value().enrich.pending} items</dd>
                <dt>embedding provider</dt>
                <dd>{value().embeddings.provider}</dd>
                <dt>embedding dimensions</dt>
                <dd>{value().embeddings.dimensions ?? '—'}</dd>
                <dt>embedding backlog</dt>
                <dd>{value().embeddings.pending} items</dd>
              </dl>
              <p class="note">
                Ingestion and delivery keep working with providers set to
                disabled; enrichment is a replaceable stage. Configure a chat
                model or a MiniLM <code>.onnx</code> file — see the{' '}
                <a href="/docs">provider docs</a>.
              </p>
            </>
          )}
        </Show>
      </section>
    </div>
  );
}
