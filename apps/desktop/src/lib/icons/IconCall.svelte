<script lang="ts">
  let { size = 16, color = "currentColor" }: { size?: number; color?: string } = $props();
</script>
<!-- Call option — upward arrow with C shape, Lucide-style stroke 1.75 -->
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
  <path d="M11 5 A4 4 0 1 0 11 11" />
  <polyline points="9 3 11 5 9 7" />
</svg>
