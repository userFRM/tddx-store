<script lang="ts">
  /**
   * What an index preset can and can't promise yet. Presets resolve to
   * today's membership (indexkit `latest`), so a past date range gets
   * today's members for every day. indexkit's point-in-time history is
   * daily for the S&P 500 only and quarterly or monthly for the rest,
   * so this stays in beta until the app can use dated membership.
   */
  import BetaTag from "$lib/feedback/BetaTag.svelte";
</script>

<div class="notice" role="note">
  <BetaTag />
  <p>
    Uses <strong>today's members</strong>. For past dates, stocks that joined
    or left the index since then will be included or missing. Historical
    membership is still in development: daily for the S&P 500, quarterly or
    monthly for the other indices.
  </p>
</div>

<style>
  .notice {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-2);
    padding: 8px 10px;
    border: 1px solid var(--accent-tint-strong);
    border-radius: var(--r-md);
    background: linear-gradient(135deg, var(--accent-tint), var(--accent-tint-weak) 60%, transparent);
    font-size: var(--text-body-sm);
    line-height: 1.45;
    color: var(--fg);
  }
  .notice p { margin: 0; }
  .notice strong { font-weight: var(--weight-semi); }
  .notice :global(.beta-tag) { margin-top: 1px; }
</style>
