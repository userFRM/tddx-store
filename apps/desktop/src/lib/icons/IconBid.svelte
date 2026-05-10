<script lang="ts">
  let { size = 16, color = "currentColor" }: { size?: number; color?: string } = $props();
</script>
<!-- Bid — buyer side, downward chevron from price line -->
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
  <line x1="3" y1="5" x2="13" y2="5" />
  <polyline points="5 8 8 11 11 8" />
</svg>
