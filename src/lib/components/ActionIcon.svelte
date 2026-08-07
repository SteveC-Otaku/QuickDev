<script lang="ts">
  type IconName =
    | "play"
    | "stop"
    | "kill"
    | "open"
    | "log"
    | "refresh"
    | "edit"
    | "delete";

  interface Props {
    name: IconName;
    size?: number;
    class?: string;
  }

  let { name, size = 18, class: className = "" }: Props = $props();

  const pngIcons: Record<string, string> = {
    play: "/icon/play.png",
    stop: "/icon/stop.png",
    kill: "/icon/kill.png",
    open: "/icon/open.png",
    log: "/icon/log.png",
    edit: "/icon/edit.png",
    delete: "/icon/delete.png",
  };
</script>

{#if name === "refresh"}
  <svg width={size} height={size} viewBox="0 0 16 16" class={className} aria-hidden="true">
    <path d="M11.5 3.5A5 5 0 1 0 12.8 9" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" />
    <path d="M12.5 2.5v2.8h-2.8" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" />
  </svg>
{:else}
  <img
    src={pngIcons[name]}
    alt=""
    width={size}
    height={size}
    class="qd-action-icon {className}"
    draggable="false"
  />
{/if}

<style>
  .qd-action-icon {
    display: block;
    object-fit: contain;
    flex-shrink: 0;
  }
</style>
