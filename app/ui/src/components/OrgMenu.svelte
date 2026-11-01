<script lang="ts">
  import { Ellipsis } from '@lucide/svelte';
  import { ICON } from '../lib/icons';
  import { openMenu } from '../lib/contextmenu.svelte';
  import { createFolder, currentSpace, orgUi } from '../lib/org.svelte';
  import { m } from '../paraglide/messages';

  async function newFolder() {
    const f = await createFolder(null);
    orgUi.renaming = f.id;
  }

  function exportAll() {
    const w = currentSpace();
    orgUi.exporting = w ? { folders: [...w.folders], connections: [...w.connections] } : { folders: [], connections: [] };
  }

  function more(e: MouseEvent) {
    const b = (e.currentTarget as HTMLElement).getBoundingClientRect();
    openMenu({ x: b.left, y: b.bottom + 4 }, [
      { label: m.org_folder_new(), icon: ICON.folderNew, run: newFolder },
      { label: m.org_space_new_dots(), icon: ICON.spaces, run: () => (orgUi.space = { id: null, preset: null }) },
      'separator',
      { label: m.org_export_dots(), icon: ICON.export, run: exportAll },
      { label: m.org_import_dots(), icon: ICON.import, run: () => (orgUi.importing = true) },
    ]);
  }
</script>

<button class="icon-btn" type="button" aria-label={m.org_folder_new()} data-tip={m.org_folder_new()} onclick={() => void newFolder()}><ICON.folderNew size={15} strokeWidth={1.7} /></button>
<button class="icon-btn" type="button" aria-label={m.org_more()} data-tip={m.org_more()} aria-haspopup="menu" onclick={more}><Ellipsis size={15} strokeWidth={1.8} /></button>
