<script lang="ts">
  /** Marks a feature that works but is not final. One hue, set apart
   *  by weight and a soft ramp rather than a second colour. */
  let { label = "Beta" }: { label?: string } = $props();
</script>

<span class="beta-tag">{label}</span>

<style>
  .beta-tag {
    display: inline-flex;
    align-items: center;
    padding: 1px 6px;
    border: 1px solid var(--accent);
    border-radius: 999px;
    background: linear-gradient(135deg, var(--accent-tint-strong), var(--accent-tint-weak));
    color: var(--accent);
    font-size: 10px;
    font-weight: var(--weight-semi);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    line-height: 1.4;
    white-space: nowrap;
  }
</style>
