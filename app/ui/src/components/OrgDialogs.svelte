<script lang="ts">
  import { orgUi } from '../lib/org.svelte';
  import ExportDialog from './ExportDialog.svelte';
  import ImportDialog from './ImportDialog.svelte';
  import SpaceDialog from './SpaceDialog.svelte';
</script>

{#if orgUi.space}
  <SpaceDialog id={orgUi.space.id} preset={orgUi.space.preset} remove={orgUi.space.remove} toVars={orgUi.space.vars} onclose={() => (orgUi.space = null)} />
{/if}
{#if orgUi.exporting}
  <ExportDialog folders={orgUi.exporting.folders} connections={orgUi.exporting.connections} onclose={() => (orgUi.exporting = null)} />
{/if}
{#if orgUi.importing}
  <ImportDialog onclose={() => (orgUi.importing = false)} />
{/if}
