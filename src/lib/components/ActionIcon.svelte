<script lang="ts">
  import { iconPaths, type IconName } from '../icons';
  let { name, size = 18, class: className = '' }: { name: IconName; size?: number; class?: string; } = $props();
</script>
<svg width={size} height={size} viewBox="0 0 24 24" class="qd-action-icon {className}" aria-hidden="true"
  fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
  {@html iconPaths[name]}
</svg>
<style>.qd-action-icon { display: inline-block; vertical-align: middle; flex-shrink: 0; }</style>
