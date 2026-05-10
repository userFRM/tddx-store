<script lang="ts">
  let { size = 16, color = "currentColor" }: { size?: number; color?: string } = $props();
</script>
<!-- Put option — downward arrow with P shape, Lucide-style stroke 1.75 -->
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
  <path d="M5 11 A4 4 0 1 0 5 5" />
  <polyline points="7 13 5 11 7 9" />
</svg>
