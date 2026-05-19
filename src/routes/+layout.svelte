<script>
  import { REACTIVE_WINDOW_SIZE } from "$lib/common.svelte";
  import { SKIN_LIBRARY } from "$lib/skins.svelte.js";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import "./global.css";
  /**
   * @type {{children: import("svelte").Snippet}}
   */
  let { children } = $props();
  $effect(() => {
    getCurrentWindow().setSize(
      new LogicalSize(
        REACTIVE_WINDOW_SIZE.width * REACTIVE_WINDOW_SIZE.zoom,
        REACTIVE_WINDOW_SIZE.height * REACTIVE_WINDOW_SIZE.zoom
      )
    );
  });

  onMount(() => {
    /** @type {undefined | (() => void)} */
    let unlistenSkinChanged;

    SKIN_LIBRARY.load();
    listen("skinChanged", (event) => {
      SKIN_LIBRARY.setLibrary(/** @type {any} */ (event.payload));
    }).then((unlisten) => {
      unlistenSkinChanged = unlisten;
    });

    return () => unlistenSkinChanged?.();
  });
</script>

{@render children()}
