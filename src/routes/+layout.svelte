<script lang="ts">
  import '$lib/styles/global.css';
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onMount, setContext } from "svelte";
  import { fade } from "svelte/transition";
  import { goto } from "$app/navigation";
  import { page } from "$app/stores";
  import { X } from "lucide-svelte";
  import UpdateManager, { type UpdateManagerAPI } from "$lib/components/UpdateManager.svelte";
  import UpdateNotification from "$lib/components/UpdateNotification.svelte";
  import { getFormattedVersion } from "$lib/utils/version";
  import { appSettings } from "$lib/stores/appStore";
  import { watchProgressStore } from "$lib/stores/watchProgressStore";
  
  const AUDIO_EXTENSIONS = new Set(['mp3','flac','wav','aac','ogg','opus','m4a','aiff','wma']);

  let { children } = $props();

  let appReady = $state(false);
  
  let showSettings = $state(false);
  let isCheckingForUpdates = $state(false);
  
  let updateManager: UpdateManagerAPI;
  let isOnGallery = $state(true);
  
  let lastAutoCheckTime = $state<number>(0);
  if (typeof localStorage !== 'undefined') {
    const stored = localStorage.getItem('lastAutoCheckTime');
    lastAutoCheckTime = stored ? parseInt(stored, 10) || 0 : 0;
  }
  
  let settings = $state($appSettings);
  
  $effect(() => {
    isOnGallery = $page.route.id === '/' || $page.route.id === null;
  });
  
  setContext('showSettings', () => showSettings = true);
  
  onMount(() => {
    let disposed = false;
    const unsubs: UnlistenFn[] = [];
    
    (async () => {
      const results = await Promise.allSettled([
        listen<string>("open-file", async (event) => {
          const encodedPath = encodeURIComponent(event.payload);
          const ext = event.payload.split('.').pop()?.toLowerCase() ?? '';
          await goto(AUDIO_EXTENSIONS.has(ext) ? `/audio/${encodedPath}` : `/player/${encodedPath}`);
          invoke("mark_file_processed").catch(console.error);
        }),
        listen<string[]>("tauri://drag-drop", async (event) => {
          if (event.payload && event.payload.length > 0) {
            const encodedPath = encodeURIComponent(event.payload[0]);
            const ext = event.payload[0].split('.').pop()?.toLowerCase() ?? '';
            await goto(AUDIO_EXTENSIONS.has(ext) ? `/audio/${encodedPath}` : `/player/${encodedPath}`);
          }
        }),
      ]);
      
      for (const r of results) {
        if (r.status === "fulfilled") {
          const un = r.value;
          if (disposed) {
            try { un(); } catch (e) { console.error("Unlisten failed", e); }
          } else {
            unsubs.push(un);
          }
        } else {
          console.error("Failed to register Tauri listener:", r.reason);
        }
      }
    })();
    
    appReady = true;
    invoke("frontend_ready").catch(console.error);
    
    return () => {
      disposed = true;
      for (const un of unsubs) {
        try { un(); } catch (e) { console.error("Unlisten failed", e); }
      }
    };
  });
  
  function handleAutoCheckStart() {
    lastAutoCheckTime = Date.now();
  }
  
  function handleAutoCheckTimeUpdate(time: number) {
    lastAutoCheckTime = time;
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('lastAutoCheckTime', time.toString());
    }
  }
</script>

<!-- Update System -->
<UpdateManager 
  bind:this={updateManager} 
  disableAutoCheck={!isOnGallery}
  onAutoCheckStart={handleAutoCheckStart}
  onAutoCheckTimeUpdate={handleAutoCheckTimeUpdate}
  lastAutoCheckTime={lastAutoCheckTime}
/>
<UpdateNotification />

{#if !appReady}
  <div class="splash-screen" out:fade={{ duration: 250 }}>
    <img src="/logo-dark.svg" alt="glucose" class="splash-logo" />
  </div>
{/if}

{@render children()}

<!-- Settings Overlay -->
{#if showSettings}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="settings-overlay" onclick={(e) => { if (e.target === e.currentTarget) showSettings = false; }}>
    <div class="settings-modal">
      <div class="settings-header">
        <div class="settings-header-content">
          <h2>Settings</h2>
          <span class="settings-version">{getFormattedVersion()}</span>
        </div>
        <button class="settings-close" onclick={() => showSettings = false} title="Close">
          <X size={20} />
        </button>
      </div>
      
      <div class="settings-content">
        <!-- App Updates Section -->
        <div class="settings-section">
          <h3>App Updates</h3>
          
          <div class="settings-group">
            <div class="settings-item">
              <div class="settings-item-label">
                <div class="settings-item-title">Check for Updates</div>
                <div class="settings-item-desc">
                  Keep Glucose up to date with the latest features and improvements
                </div>
              </div>
              <div class="settings-item-action">
                <button 
                  class="check-update-button"
                  disabled={isCheckingForUpdates}
                  onclick={async () => {
                    if (updateManager && !isCheckingForUpdates) {
                      try {
                        isCheckingForUpdates = true;
                        const checkPromise = updateManager.manualCheckForUpdates();
                        if (checkPromise) {
                          await checkPromise;
                        }
                      } finally {
                        isCheckingForUpdates = false;
                      }
                    }
                  }}
                >
                  {isCheckingForUpdates ? 'Checking...' : 'Check Now'}
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .splash-screen {
    position: fixed;
    inset: 0;
    z-index: 9999;
    background: #080a10;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .splash-logo {
    width: 120px;
    height: auto;
    opacity: 0.9;
  }

  /* Settings and Setup dialog styles - imported from original */
  .settings-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.9);
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1500;
    animation: fadeIn 0.3s ease;
  }
  
  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }
  
  .settings-modal {
    background: rgba(20, 20, 20, 0.98);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 16px;
    width: 90%;
    max-width: 700px;
    max-height: 80vh;
    overflow: hidden;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.8);
    animation: slideUp 0.3s ease;
    display: flex;
    flex-direction: column;
  }
  
  @keyframes slideUp {
    from {
      transform: translateY(20px);
      opacity: 0;
    }
    to {
      transform: translateY(0);
      opacity: 1;
    }
  }
  
  .settings-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 2rem 2.5rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
  }
  
  .settings-header-content {
    display: flex;
    align-items: baseline;
    gap: 1rem;
  }
  
  .settings-header h2 {
    font-size: 1.75rem;
    font-weight: 600;
    margin: 0;
    color: #fff;
  }
  
  .settings-version {
    font-size: 0.875rem;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.5);
    padding: 0.25rem 0.625rem;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 4px;
  }
  
  .settings-close {
    width: 32px;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    color: rgba(255, 255, 255, 0.7);
    cursor: pointer;
    transition: all 0.2s ease;
  }
  
  .settings-close:hover {
    background: rgba(255, 255, 255, 0.15);
    border-color: rgba(255, 255, 255, 0.3);
    color: #fff;
    transform: scale(1.1);
  }
  
  .settings-content {
    flex: 1;
    overflow-y: auto;
    padding: 2rem 2.5rem;
  }
  
  .settings-section {
    margin-bottom: 2rem;
  }
  
  .settings-section:last-child {
    margin-bottom: 0;
  }
  
  .settings-section h3 {
    font-size: 1.125rem;
    font-weight: 600;
    color: #fff;
    margin: 0 0 1.5rem 0;
    padding-bottom: 0.75rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }
  
  .settings-group {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    margin-bottom: 1.5rem;
  }
  
  .settings-item {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    padding: 1rem;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 8px;
    gap: 1rem;
  }
  
  .settings-item-label {
    flex: 1;
  }
  
  .settings-item-title {
    font-size: 0.9375rem;
    font-weight: 600;
    color: #fff;
    margin-bottom: 0.25rem;
  }
  
  .settings-item-desc {
    font-size: 0.8125rem;
    color: rgba(255, 255, 255, 0.6);
    line-height: 1.4;
  }
  
  .settings-item-action {
    display: flex;
    align-items: center;
  }
  
  .check-update-button {
    background: #fff;
    color: #000;
    border: none;
    padding: 0.625rem 1.25rem;
    font-size: 0.8125rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s ease;
    border-radius: 6px;
    white-space: nowrap;
  }
  
  .check-update-button:hover {
    background: rgba(255, 255, 255, 0.9);
    transform: translateY(-1px);
  }
  
  .check-update-button:active {
    transform: translateY(0);
  }
  
  .check-update-button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
    transform: none;
  }
  
  .check-update-button:disabled:hover {
    background: #fff;
    transform: none;
  }
  
</style>
