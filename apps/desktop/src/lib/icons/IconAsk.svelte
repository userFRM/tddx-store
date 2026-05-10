<script lang="ts">
  let { size = 16, color = "currentColor" }: { size?: number; color?: string } = $props();
</script>
<!-- Ask — seller side, upward chevron to price line -->
<svg
  width={size}
  height={size}
  viewBox="0 0 16 16"
  fill="none"
  stroke={color}
  stroke-width="1.75"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  <line x1="3" y1="11" x2="13" y2="11" />
  <polyline points="5 8 8 5 11 8" />
</svg>
